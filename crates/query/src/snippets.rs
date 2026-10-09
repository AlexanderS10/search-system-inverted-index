//! # Query-Dependent Snippet Generation
//!
//! Provides O(1) passage text retrieval from `collection.tsv` using the
//! binary byte offsets in `passage_offsets.bin`, and extracts relevant text
//! snippets around matched query terms for top-k search results

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};

/// Generator for fetching passage text and creating query-dependent snippets
pub struct SnippetGenerator {
    offsets_file: File,
    collection_reader: BufReader<File>,
}

impl SnippetGenerator {
    /// Opens the passage offsets binary and the collection text file
    ///
    /// Arguments:
    ///     offsets_path: path to data/index/passage_offsets.bin
    ///     collection_path: path to data/raw/collection.tsv
    ///
    /// Behavior:
    ///     Opens the offsets file and collection.tsv for fast random-access seeking
    ///
    /// Returns:
    ///     io::Result<SnippetGenerator>
    pub fn open(offsets_path: &str, collection_path: &str) -> io::Result<Self> {
        let offsets_file = File::open(offsets_path)?;
        let collection_file = File::open(collection_path)?;
        let collection_reader = BufReader::new(collection_file);

        return Ok(Self {
            offsets_file,
            collection_reader,
        });
    }

    /// Fetches the raw passage text for a given internal doc_id
    ///
    /// Arguments:
    ///     doc_id: internal document identifier (0, 1, 2, ...)
    ///
    /// Behavior:
    ///     1. Seeks to doc_id * 8 in passage_offsets.bin and reads the 8-byte little-endian offset.
    ///     2. Seeks collection.tsv directly to that exact byte offset in O(1) time.
    ///     3. Reads the line and extracts the passage text after the first tab.
    ///
    /// Returns:
    ///     io::Result<String> with the passage text
    pub fn get_passage_text(&mut self, doc_id: u32) -> io::Result<String> {
        let byte_pos = (doc_id as u64) * 8;

        // Seek directly to the 8-byte offset for this doc_id
        self.offsets_file.seek(SeekFrom::Start(byte_pos))?;
        let mut offset_bytes = [0u8; 8];
        self.offsets_file.read_exact(&mut offset_bytes)?;
        let file_offset = u64::from_le_bytes(offset_bytes);

        // Seek directly to the start of this passage line in collection.tsv
        self.collection_reader.seek(SeekFrom::Start(file_offset))?;

        let mut line = String::new();
        self.collection_reader.read_line(&mut line)?;

        // Format is: external_id \t passage_text
        if let Some((_ext_id, text)) = line.split_once('\t') {
            return Ok(text.trim().to_string());
        }

        return Ok(line.trim().to_string());
    }

    /// Generates a concise query-dependent snippet around matched query terms
    ///
    /// Arguments:
    ///     doc_id: document identifier
    ///     query_terms: list of lowercase query terms
    ///     max_chars: target maximum length of the snippet (e.g. 180 chars)
    ///
    /// Behavior:
    ///     1. Retrieves the passage text using the byte offsets
    ///     2. Finds the earliest occurrence of any query term
    ///     3. Extracts a window around the matched terms on word boundaries
    ///     4. Returns clean plain text with ellipsis indicators
    ///
    /// Returns:
    ///     io::Result<String> containing the plain snippet text
    pub fn generate_snippet(
        &mut self,
        doc_id: u32,
        query_terms: &[String],
        max_chars: usize,
    ) -> io::Result<String> {
        let text = self.get_passage_text(doc_id)?;
        if text.is_empty() {
            return Ok(String::new());
        }

        let lower = text.to_lowercase();

        // Find the earliest match position among query terms
        let mut first_match_idx: Option<usize> = None;
        for term in query_terms {
            if let Some(pos) = lower.find(term.as_str()) {
                first_match_idx = Some(first_match_idx.map_or(pos, |m| m.min(pos)));
            }
        }

        // Determine snippet start and end offsets
        let (start, end) = match first_match_idx {
            Some(pos) => {
                let context_before = 40;
                let raw_start = pos.saturating_sub(context_before);
                let raw_end = (raw_start + max_chars).min(text.len());

                // Adjust to nearest word boundary on start
                let clean_start = if raw_start == 0 {
                    0
                } else {
                    text[raw_start..]
                        .find(' ')
                        .map(|space| raw_start + space + 1)
                        .unwrap_or(raw_start)
                };

                // Adjust to nearest word boundary on end
                let clean_end = if raw_end >= text.len() {
                    text.len()
                } else {
                    text[..raw_end]
                        .rfind(' ')
                        .unwrap_or(raw_end)
                };

                (clean_start, clean_end.max(clean_start))
            }
            None => {
                // If terms not found directly, take beginning of passage
                let end = max_chars.min(text.len());
                let clean_end = text[..end].rfind(' ').unwrap_or(end);
                (0, clean_end)
            }
        };

        let snippet_body = &text[start..end];

        let mut result = String::new();
        if start > 0 {
            result.push_str("... ");
        }
        result.push_str(snippet_body);
        if end < text.len() {
            result.push_str(" ...");
        }

        return Ok(result);
    }
}

