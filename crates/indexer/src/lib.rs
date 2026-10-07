//!  public api for reading a built index
//!
//! exposes the reader and cursor types for querying built index files
//! index building and run merging are handled by the indexer binary
//!
//! basic flow:
//!
//! ```no_run
//! # fn run() -> std::io::Result<()> {
//! use indexer::IndexReader;
//!
//! let index = IndexReader::open("data/index")?;
//! let Some(mut list) = index.open_list("apple")? else {
//!     return Ok(());
//! };
//!
//! if let Some(doc_id) = list.next_geq(100)? {
//!     let term_freq = list.get_score()?;
//!     println!("{doc_id}: {term_freq}");
//! }
//!
//! list.close_list();
//! # Ok(())
//! # }
//! ```
//!
//! API:
//!
//! - `IndexReader::open(index_dir)` opens and mmaps `postings.bin` and `lexicon.bin`.
//! - `IndexReader::open_list(term)` returns a cursor for one term, or `None`.
//! - `Cursor::doc_freq()` returns the term's document frequency.
//! - `Cursor::next_geq(target_doc_id)` advances to the first doc ID >= target.
//! - `Cursor::get_score()` returns the current posting's term frequency.
//! - `Cursor::close_list()` consumes the cursor; dropping it does the same cleanup.

#[allow(dead_code)]
mod codec;
mod reader;

/// Cursor over one term's posting list.
pub use reader::Cursor;
/// Open, mmap-backed view of `postings.bin` and `lexicon.bin`.
pub use reader::IndexReader;
