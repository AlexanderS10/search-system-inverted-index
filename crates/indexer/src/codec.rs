//! bin format consts and the primitives for encode/decode
//! `postings.bin` and `lexicon.bin` are defined here
//! so reader/writer don't drift on def

use std::io::{self, Write};

/// header shape ==
//4 bytes  magic
//2 bytes  version
//2 bytes  header size
//2 bytes  chunk capacity
//2 bytes  reserved

//header defs
pub(crate) const POSTINGS_MAGIC: [u8; 4] = *b"POST";
pub(crate) const LEXICON_MAGIC: [u8; 4] = *b"LEXI";
pub(crate) const POSTINGS_FILE: &str = "postings.bin";
pub(crate) const LEXICON_FILE: &str = "lexicon.bin";
pub(crate) const FORMAT_VERSION: u16 = 1;

// codec id
pub(crate) const CODEC_BITPACKED: u8 = 1;

//size consts
pub(crate) const POSTINGS_HEADER_BYTES: u64 = 12;
pub(crate) const LEXICON_HEADER_BYTES: u64 = 32;
pub(crate) const DIRECTORY_ENTRY_BYTES: u64 = 24;

/// fixed part of a lexicon entry: u16 term_len + u32 doc_freq + u32 chunk_count + 2×u64 offsets.
pub(crate) const LEXICON_ENTRY_FIXED_BYTES: u64 = 26;

#[derive(Clone, Copy)]
pub(crate) struct Chunk {
    pub(crate) last_doc_id: u32,
    pub(crate) offset: u64,
    pub(crate) posting_count: u16,
    pub(crate) id_bytes: u32,
    pub(crate) frequency_bytes: u32,
}

// sample rate for in-RAM lexicon table
// the nearest sampled term is used as starting point
// for linear scan of the lexicon in-memory
pub(crate) const SAMPLE_INTERVAL: u32 = 64;

// fiexed size writer factories to standardize
pub(crate) trait WriteLe: Write {
    fn write_u8(&mut self, value: u8) -> io::Result<()> {
        self.write_all(&[value])
    }

    fn write_u16_le(&mut self, value: u16) -> io::Result<()> {
        self.write_all(&value.to_le_bytes())
    }

    fn write_u32_le(&mut self, value: u32) -> io::Result<()> {
        self.write_all(&value.to_le_bytes())
    }

    fn write_u64_le(&mut self, value: u64) -> io::Result<()> {
        self.write_all(&value.to_le_bytes())
    }
}

//blanket implementation... every Writer T gets WriteLe interface now
// (e.g. BufWriter can call .write_u64_le
impl<T: Write + ?Sized> WriteLe for T {}

//same factory pattern as above
#[allow(dead_code)]
pub(crate) trait ReadLe {
    fn slice(&self, offset: usize, len: usize) -> io::Result<&[u8]>;

    fn read_u16_le(&self, offset: usize) -> io::Result<u16> {
        Ok(u16::from_le_bytes(
            self.slice(offset, 2)?.try_into().unwrap(),
        ))
    }

    fn read_u32_le(&self, offset: usize) -> io::Result<u32> {
        Ok(u32::from_le_bytes(
            self.slice(offset, 4)?.try_into().unwrap(),
        ))
    }

    fn read_u64_le(&self, offset: usize) -> io::Result<u64> {
        Ok(u64::from_le_bytes(
            self.slice(offset, 8)?.try_into().unwrap(),
        ))
    }
}

impl ReadLe for [u8] {
    fn slice(&self, offset: usize, len: usize) -> io::Result<&[u8]> {
        self.get(offset..offset.saturating_add(len))
            .ok_or_else(|| invalid_err("truncated index"))
    }
}

#[allow(dead_code)]
pub(crate) fn invalid<T>(message: &'static str) -> io::Result<T> {
    Err(invalid_err(message))
}

#[allow(dead_code)]
pub(crate) fn invalid_err(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

// make it smol.
// packed formate ==
// 1 byte: bit width
// N bytes: bit-packed values, low bits first
pub(crate) fn pack_into(output: &mut Vec<u8>, values: &[u32], capacity: usize) {
    // write it pack it clear it call again and reuse it
    let width = values
        .iter()
        .copied()
        .max()
        .map_or(0, |value| (u32::BITS - value.leading_zeros()) as usize);
    let packed_len = 1 + (capacity * width).div_ceil(8);
    output.clear();
    output.reserve(packed_len);
    output.push(width as u8);

    // le bitbucket
    let mut bits = 0u64;
    let mut bit_count = 0;
    // whenever the bucket has at least a full byte,
    // write the lowest byte out, then shift bucket down.
    for &value in values {
        bits |= u64::from(value) << bit_count;
        bit_count += width;
        while bit_count >= 8 {
            output.push(bits as u8);
            bits >>= 8;
            bit_count -= 8;
        }
    }
    //if there are leftovers,
    // write lowest byte out and shift bucket down
    if bit_count > 0 {
        output.push(bits as u8);
    }
    //pad the packed bytes directly, leave the input buffers at their real count
    if values.len() < capacity {
        output.resize(packed_len, 0);
    }
}
