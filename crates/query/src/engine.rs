//! Core Search Engine Abstraction
//!
//! Bundles the memory-mapped index, document table, collection statistics,
//! and snippet generator into a single unified search engine service
//! Can be consumed by the interactive terminal REPL or by the Web UI server

use crate::doc_table::{DocTable, load_stats};
use crate::search::{QueryMode, search};
use crate::snippets::SnippetGenerator;
use crate::tokenizer::tokenize_query;
use common::CollectionStats;
use indexer::IndexReader;
use serde::{Deserialize, Serialize};
use std::io;
use std::time::Instant;

/// Individual ranked passage result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResultItem {
    pub rank: usize,
    pub doc_id: u32,
    pub external_id: u32,
    pub score: f64,
    pub doc_len: u32,
    pub snippet: Option<String>,
}

/// Structured response returned by the search engine for any query
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryResponse {
    pub terms: Vec<String>,
    pub mode: QueryMode,
    pub latency_ms: f64,
    pub results: Vec<SearchResultItem>,
}

/// Coordinates the inverted index, doc metadata, and snippet extraction
pub struct SearchEngine {
    index: IndexReader,
    doc_table: DocTable,
    stats: CollectionStats,
    snippet_gen: Option<SnippetGenerator>,
}

impl SearchEngine {
    /// Initializes and loads all resources required for query execution
    ///
    /// Arguments:
    ///     index_dir: directory containing postings.bin and lexicon.bin
    ///     stats_path: path to collection_stats.json
    ///     doc_table_path: path to doc_table.tsv
    ///     offsets_path: path to passage_offsets.bin
    ///     collection_path: path to raw collection.tsv
    ///
    /// Returns:
    ///     io::Result<SearchEngine>
    pub fn open(
        index_dir: &str,
        stats_path: &str,
        doc_table_path: &str,
        offsets_path: &str,
        collection_path: &str,
    ) -> io::Result<Self> {
        // Load collection statistics
        let stats = load_stats(stats_path)?;

        // Load document table flat array into memory
        let doc_table = DocTable::load_from_tsv(doc_table_path, stats.total_docs as usize)?;

        // Memory-map the compressed inverted index
        let index = IndexReader::open(index_dir)?;

        // Open snippet generator if offset and collection files exist
        let snippet_gen = SnippetGenerator::open(offsets_path, collection_path).ok();

        return Ok(Self {
            index,
            doc_table,
            stats,
            snippet_gen,
        });
    }

    /// Returns a reference to the loaded collection statistics
    pub fn stats(&self) -> &CollectionStats {
        return &self.stats;
    }

    /// Returns the number of documents loaded in the page table
    pub fn doc_count(&self) -> usize {
        return self.doc_table.len();
    }

    /// Returns true if snippet generation is active
    pub fn has_snippets(&self) -> bool {
        return self.snippet_gen.is_some();
    }

    /// Tokenizes, executes DAAT search, and returns enriched top-k results
    ///
    /// Arguments:
    ///     query_text: raw user query string
    ///     mode: QueryMode::And or QueryMode::Or
    ///     k: number of top results to return
    ///
    /// Returns:
    ///     io::Result<QueryResponse> containing ranked matches and plain snippets
    pub fn search(
        &mut self,
        query_text: &str,
        mode: QueryMode,
        k: usize,
    ) -> io::Result<QueryResponse> {
        let terms = tokenize_query(query_text);
        if terms.is_empty() {
            return Ok(QueryResponse {
                terms: Vec::new(),
                mode,
                latency_ms: 0.0,
                results: Vec::new(),
            });
        }

        let start_time = Instant::now();
        let scored_docs = search(
            &self.index,
            &self.doc_table,
            self.stats.total_docs,
            self.stats.avg_doc_len,
            &terms,
            mode,
            k,
        )?;
        let latency_ms = start_time.elapsed().as_secs_f64() * 1000.0;

        let mut results = Vec::with_capacity(scored_docs.len());
        for (i, scored) in scored_docs.iter().enumerate() {
            let (ext_id, doc_len) = match self.doc_table.get(scored.doc_id) {
                Some(entry) => (entry.external_id, entry.doc_len),
                None => (0, 0),
            };

            let snippet = match self.snippet_gen.as_mut() {
                Some(generator) => generator.generate_snippet(scored.doc_id, &terms, 180).ok(),
                None => None,
            };

            results.push(SearchResultItem {
                rank: i + 1,
                doc_id: scored.doc_id,
                external_id: ext_id,
                score: scored.score,
                doc_len,
                snippet,
            });
        }

        return Ok(QueryResponse {
            terms,
            mode,
            latency_ms,
            results,
        });
    }
}
