//!  reader implementation for the built index
//!
//! opens the memory-mapped lexicon and postings files
//! uses the sample table to find terms in the lexicon
//! creates cursors to seek a term's posting list and read term frequencies
//!
//! see lib.rs for the public reader api

use std::cmp::Ordering;
use std::fs::File;
use std::io;
use std::ops::Range;
use std::path::Path;
use std::sync::Arc; // allows multi-threaded reading

use memmap2::Mmap;

use crate::codec::{
    Chunk, DIRECTORY_ENTRY_BYTES, FORMAT_VERSION, LEXICON_ENTRY_FIXED_BYTES, LEXICON_FILE,
    LEXICON_HEADER_BYTES, LEXICON_MAGIC, POSTINGS_FILE, POSTINGS_HEADER_BYTES, POSTINGS_MAGIC,
    ReadLe, invalid,
}; // invalid creates invalid data error

// lexicon entry bytes after term_len + term: doc_freq..directory_offset.
// fixed fields occupy 26 bytes; after the term length and bytes, 24 remain
const LEXICON_ENTRY_TAIL_BYTES: usize = LEXICON_ENTRY_FIXED_BYTES as usize - 2;

//opened index as a whole, owns shared access to lexicon and postings
//as well as shared access to chunk capacity/in-RAM sample table
//used to open cursors for diff terms
#[derive(Clone)] // point to the same reference
pub struct IndexReader {
    inner: Arc<Inner>, //arc = shared access to .bin
}

// data shared by readers and cursors alike
// shared by cursors and clones of this opened reader
struct Inner {
    postings: Mmap,              //must never be mutated while reading
    lexicon: Mmap,               // same
    chunk_capacity: usize,       // read from the .bin file headers
    sample_table_offset: u64,    //read in from lexicon header, ie where sample table begins
    samples: Vec<(String, u64)>, //loaded into RAM when index is opened for quick-nav of lexicon
}

// owns the traversal of one term's postings list. holds position for that term.
// open_list() must be called to start a new traversal / cursor
pub struct Cursor {
    inner: Arc<Inner>,             //= shared access .bin
    doc_freq: u32,                 // comes from term's lex entry
    chunk_count: usize,            // comes from term's lex entry
    directory_offset: u64,         // points to term's chunk dir in postings.bin
    frequency_range: Range<usize>, //where this chunk's packed freqs live
    next_chunk_index: usize,       // where to start the next directory search
    ids: Vec<u32>,                 //decoded vals for that chunk
    freqs: Vec<u32>,               // same as ids
    pos: Option<usize>,            // slot in current chunk's decoded IDs vector.
    exhausted: bool,               // flags end of traversal
}

impl IndexReader {
    // opens the index; its mappings stay alive while readers or cursors hold them
    //hard failure if chunk sizes listed in lex and post headers are mismatched
    pub fn open(index_dir: impl AsRef<Path>) -> io::Result<Self> {
        // open the index files
        let index_dir = index_dir.as_ref();
        let postings = map_file(&index_dir.join(POSTINGS_FILE))?;
        let lexicon = map_file(&index_dir.join(LEXICON_FILE))?;
        // validate headers, return capacity,
        // make sure there is parity in their chunk sizes
        let capacity = check_header(&postings, POSTINGS_MAGIC, POSTINGS_HEADER_BYTES)?;
        if capacity != check_header(&lexicon, LEXICON_MAGIC, LEXICON_HEADER_BYTES)? {
            return invalid("chunk capacity mismatch");
        }

        let sample_table_offset = lexicon.read_u64_le(16)?;
        //read the sample count and loads them to RAM starting at offset found above
        let samples = read_samples(&lexicon, sample_table_offset, lexicon.read_u32_le(24)?)?;

        // return a shared object for multithreaded reading
        Ok(Self {
            inner: Arc::new(Inner {
                postings,
                lexicon,
                chunk_capacity: usize::from(capacity),
                sample_table_offset,
                samples,
            }),
        })
    }
    // seeks a term and returns a cursor for it if present, None else. Hard
    // failure on reading error
    pub fn open_list(&self, term: &str) -> io::Result<Option<Cursor>> {
        // match lexicon format for comparison
        let query = term.as_bytes();
        // look at the RAM sample table, figure out what region,
        // of the in memory lexicon file to seek through.
        // from the sample table, figure out the range of the lexicon file
        // this term will be in
        let samples = &self.inner.samples;
        let Some(group) = samples
            .partition_point(|s| s.0.as_bytes() <= query)
            .checked_sub(1)
        else {
            return Ok(None);
        };
        let mut offset = samples[group].1 as usize;
        let end = samples
            .get(group + 1)
            .map_or(self.inner.sample_table_offset, |s| s.1) as usize;
        //scan in the lexicon.bin from start -> end offset found above
        // if it's not in that range of the file, it's not in the lexicon
        while offset < end {
            let term_len = usize::from(self.inner.lexicon.read_u16_le(offset)?);
            let term_bytes = self.inner.lexicon.slice(offset + 2, term_len)?;
            offset += 2 + term_len;
            // compare stored term w/ query
            match term_bytes.cmp(query) {
                //return if > query
                Ordering::Greater => return Ok(None),
                // skip metadata decode if still < query
                Ordering::Less => offset += LEXICON_ENTRY_TAIL_BYTES,
                // yay, found it, read the metadata and return a cursor
                Ordering::Equal => {
                    let fixed = self.inner.lexicon.slice(offset, LEXICON_ENTRY_TAIL_BYTES)?;
                    let chunk_count = fixed.read_u32_le(4)? as usize;
                    return Ok(Some(Cursor {
                        inner: Arc::clone(&self.inner),
                        doc_freq: fixed.read_u32_le(0)?,
                        chunk_count,
                        directory_offset: fixed.read_u64_le(16)?,
                        frequency_range: 0..0,
                        next_chunk_index: 0,
                        ids: Vec::with_capacity(self.inner.chunk_capacity),
                        freqs: Vec::with_capacity(self.inner.chunk_capacity),
                        pos: None,
                        exhausted: chunk_count == 0,
                    }));
                }
            }
        }
        Ok(None)
    }
}

//handles cursor and api traversal
impl Cursor {
    pub fn doc_freq(&self) -> u32 {
        self.doc_freq
    }

    pub fn next_geq(&mut self, target_doc_id: u32) -> io::Result<Option<u32>> {
        //nothing else to see here
        if self.exhausted {
            return Ok(None);
        }
        // if the target could be in this chunk, search its decoded ids
        if self.ids.last().is_some_and(|&last| target_doc_id <= last) {
            return Ok(self.seek_loaded(target_doc_id));
        }
        // what later chunk could it be in? bin search from the current directory position
        let Some((chunk_index, entry)) = self.find_chunk(target_doc_id)? else {
            self.exhausted = true;
            self.pos = None;
            return Ok(None);
        };
        //decode chosen chunks ids
        self.load_ids(entry)?;
        let doc_id = self.seek_loaded(target_doc_id);
        //skip this chunk once positioned; retry it if no position was found
        self.next_chunk_index = chunk_index + usize::from(doc_id.is_some());
        // returns the ngeq
        Ok(doc_id)
    }
    // returns decoded term frequency if known, else decodes and returns
    pub fn get_score(&mut self) -> io::Result<u32> {
        let Some(pos) = self.pos else {
            return invalid("cursor is not positioned");
        };
        //empty freqs means this chunk hasn't had its scores decoded yet
        if self.freqs.is_empty() {
            let src = self
                .inner
                .postings
                .slice(self.frequency_range.start, self.frequency_range.len())?;
            decode_section(
                &mut self.freqs,
                src,
                self.inner.chunk_capacity,
                self.ids.len(),
            )?;
        }
        Ok(self.freqs[pos])
    }

    // consuming the cursor drops its buffers and releases its shared index reference.
    // despite empty body, calling this performs the consumption
    pub fn close_list(self) {}

    //search ids already decoded in memory
    fn seek_loaded(&mut self, target_doc_id: u32) -> Option<u32> {
        let start = self.pos.unwrap_or(0);
        self.pos = (start..self.ids.len()).find(|&i| self.ids[i] >= target_doc_id);
        self.pos.map(|i| self.ids[i])
    }
    // gallop through the directory, then binary search for the first chunk ending at or after target
    fn find_chunk(&self, target_doc_id: u32) -> io::Result<Option<(usize, Chunk)>> {
        let (mut lo, mut hi) = (self.next_chunk_index, self.chunk_count);
        let mut step = 1usize;
        let mut candidate = None;
        while lo < hi {
            //gallop until there's a candidate, then narrow the range before it
            let probe = if candidate.is_none() {
                let probe = self.next_chunk_index.saturating_add(step - 1).min(hi - 1);
                step = step.saturating_mul(2);
                probe
            } else {
                lo + (hi - lo) / 2
            };
            let entry = self.directory_entry(probe)?;
            if entry.last_doc_id < target_doc_id {
                lo = probe + 1;
            } else {
                //keep the winning entry, only search the chunks before it
                candidate = Some((probe, entry));
                hi = probe;
            }
        }
        Ok(candidate)
    }
    //decodes one chunks ids
    fn load_ids(&mut self, entry: Chunk) -> io::Result<()> {
        let src = self
            .inner
            .postings
            .slice(entry.offset as usize, entry.id_bytes as usize)?;
        decode_section(
            &mut self.ids,
            src,
            self.inner.chunk_capacity,
            entry.posting_count.into(),
        )?;
        let mut doc_id = 0;
        for id in &mut self.ids {
            doc_id += *id;
            *id = doc_id;
        }
        let start = (entry.offset + u64::from(entry.id_bytes)) as usize;
        self.frequency_range = start..start + entry.frequency_bytes as usize;
        self.freqs.clear();
        self.pos = None;
        Ok(())
    }
    // read one chunks metadata record
    fn directory_entry(&self, chunk: usize) -> io::Result<Chunk> {
        let offset = self.directory_offset as usize + chunk * DIRECTORY_ENTRY_BYTES as usize;
        let entry = self
            .inner
            .postings
            .slice(offset, DIRECTORY_ENTRY_BYTES as usize)?;
        Ok(Chunk {
            last_doc_id: entry.read_u32_le(0)?,
            offset: entry.read_u64_le(4)?,
            posting_count: entry.read_u16_le(12)?,
            id_bytes: entry.read_u32_le(16)?,
            frequency_bytes: entry.read_u32_le(20)?,
        })
    }
}

//lexicon sampling/file helpers

// maps the file for read access; the file must not be changed while mapped
fn map_file(path: &Path) -> io::Result<Mmap> {
    let file = File::open(path)?;
    // SAFETY: the index is immutable after publication. Reader code only creates
    // shared read-only mappings and never mutates or truncates the mapped files.
    unsafe { Mmap::map(&file) }
}

// validates header, returns the chunk size
fn check_header(bytes: &[u8], magic: [u8; 4], header_bytes: u64) -> io::Result<u16> {
    let capacity = bytes.read_u16_le(8)?;
    if bytes.slice(0, 4)? != magic
        || bytes.read_u16_le(4)? != FORMAT_VERSION
        || bytes.read_u16_le(6)? != header_bytes as u16
        || bytes.read_u16_le(10)? != 0
        || capacity == 0
    {
        return invalid("bad header");
    }
    Ok(capacity)
}

// allocate the in-RAM lexicon sample table
// held in the shared Inner object
fn read_samples(bytes: &[u8], offset: u64, count: u32) -> io::Result<Vec<(String, u64)>> {
    let mut samples = Vec::with_capacity(count as usize);
    let mut pos = offset as usize;
    for _ in 0..count {
        let len = usize::from(bytes.read_u16_le(pos)?);
        let term = String::from_utf8_lossy(bytes.slice(pos + 2, len)?).into_owned();
        pos += 2 + len;
        samples.push((term, bytes.read_u64_le(pos)?));
        pos += 8;
    }
    Ok(samples)
}

//resize to real count, leave the padded slots packed
//bit level decoder.
// unpacks one width-bit value per dst slot, reading packed bits least-significant first
fn decode_section(dst: &mut Vec<u32>, src: &[u8], capacity: usize, count: usize) -> io::Result<()> {
    // first byte gives the bit width; a missing width byte is an error
    let Some((&width, data)) = src.split_first() else {
        return invalid("empty packed section");
    };
    let width = usize::from(width);
    if width > 32 || count == 0 || count > capacity || data.len() != (capacity * width).div_ceil(8)
    {
        return invalid("bad packed section");
    }

    //check padding as bytes instead of unpacking values we'll throw away
    let real_bits = count * width;
    if let Some((&first, rest)) = data[real_bits / 8..].split_first()
        && (first >> (real_bits % 8) != 0 || rest.iter().any(|&byte| byte != 0))
    {
        return invalid("nonzero packed padding");
    }

    //validate before touching dst so a failed decode can be retried
    dst.resize(count, 0);
    if width == 0 {
        dst.fill(0);
        return Ok(());
    }

    let mask = (1u64 << width) - 1;
    let (mut byte, mut bits, mut bit_count) = (0, 0u64, 0usize);
    for value in dst {
        while bit_count < width {
            bits |= u64::from(data[byte]) << bit_count;
            byte += 1;
            bit_count += 8;
        }
        *value = (bits & mask) as u32;
        bits >>= width;
        bit_count -= width;
    }
    Ok(())
}
