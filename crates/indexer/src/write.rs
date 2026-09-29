use std::fs::File;
use std::io::{self, BufWriter, Seek, SeekFrom, Write};
use std::path::Path;

use common::Posting;

use crate::codec::{
    CODEC_BITPACKED, Chunk, DIRECTORY_ENTRY_BYTES, FORMAT_VERSION, LEXICON_ENTRY_FIXED_BYTES,
    LEXICON_HEADER_BYTES, LEXICON_MAGIC, POSTINGS_HEADER_BYTES, POSTINGS_MAGIC, SAMPLE_INTERVAL,
    WriteLe, pack_into,
};

pub(crate) struct IndexWriter {
    postings: BufWriter<File>,
    lexicon: BufWriter<File>,
    // manual byte cursors so lexicon entries can point into postings.bin
    postings_position: u64,
    lexicon_position: u64,
    chunk_capacity: usize,
    term: Option<String>,
    // current term chunk buffers
    doc_ids: Vec<u32>,
    frequencies: Vec<u32>,
    pack_scratch: Vec<u8>,
    // chunk metadata for current term
    chunks: Vec<Chunk>,
    // sparse lexicon index, every SAMPLE_INTERVAL terms
    samples: Vec<(String, u64)>,
    term_count: u32,
}

// wires into codec to write for the index builder
impl IndexWriter {
    pub(crate) fn new(output_dir: &Path, chunk_capacity: u16) -> io::Result<Self> {
        if chunk_capacity == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "chunk capacity must be greater than zero",
            ));
        }

        let mut postings = BufWriter::new(File::create(output_dir.join("postings.bin"))?);
        postings.write_all(&POSTINGS_MAGIC)?;
        postings.write_u16_le(FORMAT_VERSION)?;
        postings.write_u16_le(POSTINGS_HEADER_BYTES as u16)?;
        postings.write_u16_le(chunk_capacity)?;
        postings.write_u16_le(0)?;

        // header needs final offsets/counts, so reserve it and backfill in finish()
        let mut lexicon = BufWriter::new(File::create(output_dir.join("lexicon.bin"))?);
        lexicon.write_all(&[0; LEXICON_HEADER_BYTES as usize])?;

        let capacity = usize::from(chunk_capacity);
        Ok(Self {
            postings,
            lexicon,
            postings_position: POSTINGS_HEADER_BYTES,
            lexicon_position: LEXICON_HEADER_BYTES,
            chunk_capacity: capacity,
            term: None,
            doc_ids: Vec::with_capacity(capacity),
            frequencies: Vec::with_capacity(capacity),
            pack_scratch: Vec::new(),
            chunks: Vec::new(),
            samples: Vec::new(),
            term_count: 0,
        })
    }

    pub(crate) fn write_posting(&mut self, posting: Posting) -> io::Result<()> {
        let Posting { term, doc_id, freq } = posting;
        // input is sorted, so term changes mean previous term is complete
        if self.term.as_deref() != Some(&term) {
            self.finish_term()?;
            self.term = Some(term);
        }

        self.doc_ids.push(doc_id);
        self.frequencies.push(freq);

        if self.doc_ids.len() == self.chunk_capacity {
            self.write_chunk()?;
        }
        Ok(())
    }

    fn write_chunk(&mut self) -> io::Result<()> {
        if self.doc_ids.is_empty() {
            return Ok(());
        }

        let posting_count = self.doc_ids.len() as u16;
        let last_doc_id = *self.doc_ids.last().expect("nonempty chunk");
        // delta encode doc ids before bit-packing
        let mut previous = 0;
        for doc_id in &mut self.doc_ids {
            let current = *doc_id;
            *doc_id -= previous;
            previous = current;
        }
        self.doc_ids.resize(self.chunk_capacity, 0);
        self.frequencies.resize(self.chunk_capacity, 0);

        // write packed doc deltas, then packed frequencies
        let offset = self.postings_position;
        pack_into(&mut self.pack_scratch, &self.doc_ids);
        self.postings.write_all(&self.pack_scratch)?;
        let id_bytes = self.pack_scratch.len() as u32;
        pack_into(&mut self.pack_scratch, &self.frequencies);
        self.postings.write_all(&self.pack_scratch)?;
        let frequency_bytes = self.pack_scratch.len() as u32;
        self.postings_position += u64::from(id_bytes) + u64::from(frequency_bytes);

        self.chunks.push(Chunk {
            last_doc_id,
            offset,
            posting_count,
            id_bytes,
            frequency_bytes,
        });
        self.doc_ids.clear();
        self.frequencies.clear();
        Ok(())
    }

    fn finish_term(&mut self) -> io::Result<()> {
        let Some(term) = self.term.take() else {
            return Ok(());
        };
        self.write_chunk()?;

        let first_chunk_offset = self.chunks[0].offset;

        // chunk directory lives after the term's packed chunks
        let directory_offset = self.postings_position;
        for chunk in &self.chunks {
            self.postings.write_u32_le(chunk.last_doc_id)?;
            self.postings.write_u64_le(chunk.offset)?;
            self.postings.write_u16_le(chunk.posting_count)?;
            self.postings.write_u8(CODEC_BITPACKED)?;
            self.postings.write_u8(0)?;
            self.postings.write_u32_le(chunk.id_bytes)?;
            self.postings.write_u32_le(chunk.frequency_bytes)?;
        }
        self.postings_position += DIRECTORY_ENTRY_BYTES * self.chunks.len() as u64;

        let entry_offset = self.lexicon_position;
        // sample every nth term so reader can jump into lexicon
        if self.term_count.is_multiple_of(SAMPLE_INTERVAL) {
            self.samples.push((term.clone(), entry_offset));
        }
        let document_frequency = self
            .chunks
            .iter()
            .map(|chunk| u32::from(chunk.posting_count))
            .sum();
        write_term(&mut self.lexicon, &term)?;
        self.lexicon.write_u32_le(document_frequency)?;
        self.lexicon.write_u32_le(self.chunks.len() as u32)?;
        self.lexicon.write_u64_le(first_chunk_offset)?;
        self.lexicon.write_u64_le(directory_offset)?;
        self.lexicon_position += LEXICON_ENTRY_FIXED_BYTES + term.len() as u64;

        self.term_count += 1;
        self.chunks.clear();
        Ok(())
    }

    pub(crate) fn finish(mut self) -> io::Result<()> {
        self.finish_term()?;

        // append sample table, then seek back to fill lexicon header
        let sample_table_offset = self.lexicon_position;
        for (term, offset) in &self.samples {
            write_term(&mut self.lexicon, term)?;
            self.lexicon.write_u64_le(*offset)?;
        }

        self.lexicon.seek(SeekFrom::Start(0))?;
        self.lexicon.write_all(&LEXICON_MAGIC)?;
        self.lexicon.write_u16_le(FORMAT_VERSION)?;
        self.lexicon.write_u16_le(LEXICON_HEADER_BYTES as u16)?;
        self.lexicon.write_u16_le(self.chunk_capacity as u16)?;
        self.lexicon.write_u16_le(0)?;
        self.lexicon.write_u32_le(SAMPLE_INTERVAL)?;
        self.lexicon.write_u64_le(sample_table_offset)?;
        self.lexicon.write_u32_le(self.samples.len() as u32)?;
        self.lexicon.write_u32_le(self.term_count)?;

        self.lexicon.flush()?;
        self.postings.flush()
    }
}

fn write_term(writer: &mut impl WriteLe, term: &str) -> io::Result<()> {
    let term_len = u16::try_from(term.len()).map_err(|_| invalid_data("term is too long"))?;
    writer.write_u16_le(term_len)?;
    writer.write_all(term.as_bytes())
}

fn invalid_data(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
