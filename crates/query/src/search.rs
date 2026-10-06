//! # Document-At-A-Time (DAAT) Query Processing
//!
//! Implements Document-At-A-Time (DAAT) traversal for both:
//! - Conjunctive (AND) queries: using the lecture leapfrog / zig-zag join algorithm
//! - Disjunctive (OR) queries: using multi-cursor union traversal
//!
//! Handles block skipping via the index reader, lazy frequency decoding
//! BM25 scoring, and top-k min-heap tracking

use crate::bm25::{DEFAULT_B, DEFAULT_K1, compute_idf, score_term};
use crate::doc_table::DocTable;
use crate::top_k::{ScoredDoc, TopKHeap};
use indexer::{Cursor, IndexReader};
use serde::{Deserialize, Serialize};
use std::io;

/// Query evaluation mode chosen by the user in the REPL
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryMode {
    /// All query terms must be present in the passage
    And,
    /// At least one query term must be present in the passage
    Or,
}

/// Helper struct that holds an open cursor for a query term with its precalculated IDF
struct TermCursor {
    #[allow(dead_code)]
    term: String,
    idf: f64,
    cursor: Cursor,
    current_doc: Option<u32>,
}

/// Main search entrypoint that routes the query to conjunctive or disjunctive execution
///
/// Arguments:
///     index: reference to the open IndexReader
///     doc_table: reference to the loaded DocTable for doc_len and external_id
///     total_docs: total number of passages in MS MARCO collection (N)
///     avg_doc_len: average passage token length across the collection (avgdl)
///     terms: clean deduplicated query terms from the tokenizer
///     mode: QueryMode::And or QueryMode::Or
///     k: number of top results to return (typically 10)
///
/// Behavior:
///     Checks if query terms are empty, and forwards to the matching DAAT algorithm
///
/// Returns:
///     A vector of ScoredDoc sorted from rank 1 (highest score) down to rank k
pub fn search(
    index: &IndexReader,
    doc_table: &DocTable,
    total_docs: u64,
    avg_doc_len: f64,
    terms: &[String],
    mode: QueryMode,
    k: usize,
) -> io::Result<Vec<ScoredDoc>> {
    if terms.is_empty() || k == 0 {
        return Ok(Vec::new());
    }

    match mode {
        QueryMode::And => {
            return search_conjunctive(index, doc_table, total_docs, avg_doc_len, terms, k);
        }
        QueryMode::Or => {
            return search_disjunctive(index, doc_table, total_docs, avg_doc_len, terms, k);
        }
    }
}

/// DAAT Conjunctive (AND) query execution
///
/// Arguments:
///     index: open IndexReader
///     doc_table: loaded DocTable
///     total_docs: collection N
///     avg_doc_len: collection avgdl
///     terms: query terms
///     k: top results count
///
/// Behavior:
///     1. Opens a cursor for each term and computes its IDF
///        If ANY term is missing in the lexicon, intersection is empty and returns immediately
///     2. Sorts cursors by doc_freq ascending (shortest list first)
///     3. Uses the leapfrog algorithm from Lecture 3:
///        - Advances shortest list to did = next_geq(did)
///        - Checks if other lists match did
///        - If any list leaps forward to d > did, updates did = d and repeats
///        - If all match, computes BM25 score, adds to min-heap, and increments did
///     4. Closes all cursors
///
/// Returns:
///     Top-k results sorted in descending score order.
fn search_conjunctive(
    index: &IndexReader,
    doc_table: &DocTable,
    total_docs: u64,
    avg_doc_len: f64,
    terms: &[String],
    k: usize,
) -> io::Result<Vec<ScoredDoc>> {
    let mut cursors: Vec<TermCursor> = Vec::with_capacity(terms.len());

    // Step 1: Open a cursor for each query term
    for term in terms {
        match index.open_list(term)? {
            Some(cursor) => {
                let df = cursor.doc_freq();
                // If a term appears in 0 documents, AND query has no results
                if df == 0 {
                    return Ok(Vec::new());
                }
                let idf = compute_idf(total_docs, df);
                cursors.push(TermCursor {
                    term: term.clone(),
                    idf,
                    cursor,
                    current_doc: None,
                });
            }
            None => {
                // Term does not exist in lexicon, intersection is empty
                return Ok(Vec::new());
            }
        }
    }

    // Step 2: Sort cursors so shortest list is at index 0
    cursors.sort_by_key(|tc| tc.cursor.doc_freq());

    let mut heap = TopKHeap::new(k);
    let mut did: u32 = 0;
    let num_terms = cursors.len();

    // Step 3: Leapfrog loop across the posting lists
    'outer: loop {
        // Find next candidate in the shortest list
        let Some(first_match) = cursors[0].cursor.next_geq(did)? else {
            break 'outer; // Shortest list reached the end
        };
        did = first_match;

        // Check if all other lists contain this exact same did
        for i in 1..num_terms {
            match cursors[i].cursor.next_geq(did)? {
                Some(other_doc) if other_doc == did => {
                    // Match found in this list, continue to check the next list
                    continue;
                }
                Some(other_doc) => {
                    // Leapfrog! This list jumped ahead to other_doc > did
                    // did is not in the intersection, so update did and restart
                    did = other_doc;
                    continue 'outer;
                }
                None => {
                    // This list is exhausted, no further intersection possible
                    break 'outer;
                }
            }
        }

        // If we reach this point, did matched across ALL query terms!
        if let Some(doc_entry) = doc_table.get(did) {
            let mut total_score = 0.0;

            // Lazily decompress frequencies and calculate BM25 score
            for tc in &mut cursors {
                let tf = tc.cursor.get_score()?;
                total_score += score_term(
                    tc.idf,
                    tf,
                    doc_entry.doc_len,
                    avg_doc_len,
                    DEFAULT_K1,
                    DEFAULT_B,
                );
            }

            // Insert into top-k heap
            heap.push(did, total_score);
        }

        // Increment did to look for the next match
        if did == u32::MAX {
            break;
        }
        did += 1;
    }

    // Explicitly close cursors
    for tc in cursors {
        tc.cursor.close_list();
    }

    return Ok(heap.into_sorted_vec());
}

/// DAAT Disjunctive (OR) query execution
///
/// Arguments:
///     index: open IndexReader
///     doc_table: loaded DocTable
///     total_docs: collection N
///     avg_doc_len: collection avgdl
///     terms: query terms
///     k: top results count
///
/// Behavior:
///     1. Opens cursors for all valid terms present in the lexicon
///     2. Positions each cursor at its first posting (doc_id >= 0)
///     3. Multi-way merge loop:
///        - Finds the minimum doc_id (min_doc) across all active cursors
///        - For all cursors matching min_doc, computes BM25 score and advances them
///        - Pushes (min_doc, score) to min-heap.
///        - Removes exhausted cursors until none remain
///     4. Closes all cursors
///
/// Returns:
///     Top-k results sorted in descending score order
fn search_disjunctive(
    index: &IndexReader,
    doc_table: &DocTable,
    total_docs: u64,
    avg_doc_len: f64,
    terms: &[String],
    k: usize,
) -> io::Result<Vec<ScoredDoc>> {
    let mut cursors: Vec<TermCursor> = Vec::with_capacity(terms.len());

    // Open cursors for terms that exist in the lexicon
    for term in terms {
        if let Some(mut cursor) = index.open_list(term)? {
            let df = cursor.doc_freq();
            if df > 0 {
                let idf = compute_idf(total_docs, df);
                // Move cursor to its first match
                let first_doc = cursor.next_geq(0)?;
                if first_doc.is_some() {
                    cursors.push(TermCursor {
                        term: term.clone(),
                        idf,
                        cursor,
                        current_doc: first_doc,
                    });
                }
            }
        }
    }

    // If none of the query terms exist in the collection, return empty
    if cursors.is_empty() {
        return Ok(Vec::new());
    }

    let mut heap = TopKHeap::new(k);

    // Multi-way union merge loop
    loop {
        // Find smallest doc_id among active cursors
        let mut min_doc: Option<u32> = None;
        for tc in &cursors {
            if let Some(doc) = tc.current_doc {
                min_doc = Some(min_doc.map_or(doc, |m| m.min(doc)));
            }
        }

        let Some(target_doc) = min_doc else {
            break; // All cursors have been exhausted
        };

        // Score all terms that match this target_doc
        let doc_entry = doc_table.get(target_doc);
        let mut total_score = 0.0;

        for tc in &mut cursors {
            if tc.current_doc == Some(target_doc) {
                if let Some(entry) = doc_entry {
                    let tf = tc.cursor.get_score()?;
                    total_score += score_term(
                        tc.idf,
                        tf,
                        entry.doc_len,
                        avg_doc_len,
                        DEFAULT_K1,
                        DEFAULT_B,
                    );
                }

                // Advance cursor to the next posting past target_doc
                tc.current_doc = if target_doc == u32::MAX {
                    None
                } else {
                    tc.cursor.next_geq(target_doc + 1)?
                };
            }
        }

        // Push candidate to top-k heap
        if total_score > 0.0 {
            heap.push(target_doc, total_score);
        }

        // Drop exhausted cursors
        cursors.retain(|tc| tc.current_doc.is_some());
        if cursors.is_empty() {
            break;
        }
    }

    // Explicitly close cursors
    for tc in cursors {
        tc.cursor.close_list();
    }

    return Ok(heap.into_sorted_vec());
}
