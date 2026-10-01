use std::cmp::Ordering;
use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::Arc;

use memmap2::Mmap;

use crate::codec::{
    Chunk, DIRECTORY_ENTRY_BYTES, FORMAT_VERSION, LEXICON_ENTRY_FIXED_BYTES, LEXICON_HEADER_BYTES,
    LEXICON_MAGIC, POSTINGS_HEADER_BYTES, POSTINGS_MAGIC, ReadLe, invalid,
};

// lexicon entry bytes after term_len + term: doc_freq..directory_offset.
const LEXICON_ENTRY_TAIL_BYTES: usize = LEXICON_ENTRY_FIXED_BYTES as usize - 2;

#[derive(Clone)]
pub struct IndexReader {
    inner: Arc<Inner>,
}

struct Inner {
    postings: Mmap,
    lexicon: Mmap,
    chunk_capacity: usize,
    sample_table_offset: u64,
    samples: Vec<(String, u64)>,
}

pub struct Cursor {
    inner: Arc<Inner>,
    doc_freq: u32,
    chunk_count: usize,
    directory_offset: u64,
    current_entry: Option<Chunk>,
    ids: Vec<u32>,
    freqs: Vec<u32>,
    freqs_loaded: bool,
    pos: Option<usize>,
    exhausted: bool,
}

impl IndexReader {
    pub fn open(index_dir: impl AsRef<Path>) -> io::Result<Self> {
        let index_dir = index_dir.as_ref();
        let postings = map_file(&index_dir.join("postings.bin"))?;
        let lexicon = map_file(&index_dir.join("lexicon.bin"))?;
        let capacity = check_header(&postings, POSTINGS_MAGIC, POSTINGS_HEADER_BYTES)?;
        if capacity != check_header(&lexicon, LEXICON_MAGIC, LEXICON_HEADER_BYTES)? {
            return invalid("chunk capacity mismatch");
        }
        let sample_table_offset = lexicon.read_u64_le(16)?;
        let samples = read_samples(&lexicon, sample_table_offset, lexicon.read_u32_le(24)?)?;
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

    pub fn open_list(&self, term: &str) -> io::Result<Option<Cursor>> {
        let query = term.as_bytes();
        let Some((mut offset, end)) = self.inner.group_range(query) else {
            return Ok(None);
        };
        while offset < end {
            let term_len = usize::from(self.inner.lexicon.read_u16_le(offset)?);
            let term_bytes = self.inner.lexicon.slice(offset + 2, term_len)?;
            offset += 2 + term_len;
            match term_bytes.cmp(query) {
                Ordering::Greater => return Ok(None),
                Ordering::Less => offset += LEXICON_ENTRY_TAIL_BYTES,
                Ordering::Equal => {
                    let fixed = self.inner.lexicon.slice(offset, LEXICON_ENTRY_TAIL_BYTES)?;
                    let chunk_count = fixed.read_u32_le(4)? as usize;
                    return Ok(Some(Cursor {
                        inner: Arc::clone(&self.inner),
                        doc_freq: fixed.read_u32_le(0)?,
                        chunk_count,
                        directory_offset: fixed.read_u64_le(16)?,
                        current_entry: None,
                        ids: Vec::with_capacity(self.inner.chunk_capacity),
                        freqs: Vec::with_capacity(self.inner.chunk_capacity),
                        freqs_loaded: false,
                        pos: None,
                        exhausted: chunk_count == 0,
                    }));
                }
            }
        }
        Ok(None)
    }
}

impl Cursor {
    pub fn doc_freq(&self) -> u32 {
        self.doc_freq
    }

    pub fn next_geq(&mut self, target_doc_id: u32) -> io::Result<Option<u32>> {
        if self.exhausted {
            return Ok(None);
        }
        if let (Some(entry), Some(pos)) = (self.current_entry, self.pos) {
            if target_doc_id <= self.ids[pos] {
                return Ok(Some(self.ids[pos]));
            }
            if target_doc_id <= entry.last_doc_id {
                return Ok(self.seek_loaded(target_doc_id));
            }
        }
        let Some(entry) = self.find_chunk(target_doc_id)? else {
            self.exhausted = true;
            self.pos = None;
            return Ok(None);
        };
        self.load_ids(entry)?;
        Ok(self.seek_loaded(target_doc_id))
    }

    pub fn get_score(&mut self) -> io::Result<u32> {
        let Some(pos) = self.pos else {
            return invalid("cursor is not positioned");
        };
        if !self.freqs_loaded {
            let entry = self.current_entry.unwrap();
            let offset = (entry.offset + u64::from(entry.id_bytes)) as usize;
            let src = self
                .inner
                .postings
                .slice(offset, entry.frequency_bytes as usize)?;
            decode_section(
                &mut self.freqs,
                src,
                self.inner.chunk_capacity,
                entry.posting_count.into(),
            )?;
            self.freqs_loaded = true;
        }
        Ok(self.freqs[pos])
    }

    pub fn close_list(self) {}

    fn seek_loaded(&mut self, target_doc_id: u32) -> Option<u32> {
        let start = self.pos.unwrap_or(0);
        self.pos = (start..self.ids.len()).find(|&i| self.ids[i] >= target_doc_id);
        self.pos.map(|i| self.ids[i])
    }

    fn find_chunk(&self, target_doc_id: u32) -> io::Result<Option<Chunk>> {
        let (mut lo, mut hi) = (0, self.chunk_count);
        while lo < hi {
            let mid = (lo + hi) / 2;
            if self.directory_entry(mid)?.last_doc_id < target_doc_id {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        (lo < self.chunk_count)
            .then(|| self.directory_entry(lo))
            .transpose()
    }

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
        self.current_entry = Some(entry);
        self.freqs_loaded = false;
        self.pos = None;
        Ok(())
    }

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

impl Inner {
    fn group_range(&self, query: &[u8]) -> Option<(usize, usize)> {
        let group = self
            .samples
            .partition_point(|s| s.0.as_bytes() <= query)
            .checked_sub(1)?;
        let end = self
            .samples
            .get(group + 1)
            .map_or(self.sample_table_offset, |s| s.1);
        Some((self.samples[group].1 as usize, end as usize))
    }
}

fn map_file(path: &Path) -> io::Result<Mmap> {
    let file = File::open(path)?;
    // SAFETY: the index is immutable after publication; reader code only creates
    // shared read-only mappings and never mutates or truncates the mapped files.
    unsafe { Mmap::map(&file) }
}

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

// resize to capacity, unpack every slot, then drop the padded slots past the real count.
fn decode_section(dst: &mut Vec<u32>, src: &[u8], capacity: usize, count: usize) -> io::Result<()> {
    dst.resize(capacity, 0);
    unpack_into(src, dst)?;
    dst.truncate(count);
    Ok(())
}

fn unpack_into(src: &[u8], dst: &mut [u32]) -> io::Result<()> {
    let Some((&width, data)) = src.split_first() else {
        return invalid("empty packed section");
    };
    if width == 0 {
        dst.fill(0);
        return Ok(());
    }
    let mask = if width == 32 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    };
    let (mut byte, mut bits, mut bit_count) = (0, 0u64, 0usize);
    for value in dst {
        while bit_count < usize::from(width) {
            bits |= u64::from(data[byte]) << bit_count;
            byte += 1;
            bit_count += 8;
        }
        *value = (bits & mask) as u32;
        bits >>= width;
        bit_count -= usize::from(width);
    }
    Ok(())
}
