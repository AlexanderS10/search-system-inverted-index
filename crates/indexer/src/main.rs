//!  this file does not contain index reading behavior
//!  see lib.rs for the reader api
//!
//! entry point for building the index
//!
//! iterates through run files named run_*.tsv from input-dir folder
//! of the shape (term<TAB>doc_id<TAB>freq)
//!
//! call from the cli with: cargo run -p indexer
//!
//! override ex:
//! cargo run -p indexer -- --input-dir data/runs
//!
//!  args:
//!     --input-dir (default data/runs)
//!     --output-dir (default data/index) *holds lexicon.bin and postings.bin
//!     --chunk-capacity type: int (default 64) *defines block size, > 0
//!     --fan-in type: int (default 32) >= 2
//!
//! input contract:
//!     input-dir should contain sorted run_*.tsv files
//!     each row is term<TAB>doc_id<TAB>freq
//!     rows should be ordered by (term, doc_id)
//!     freq must be > 0
//!
//! behavior:
//!     reads parser run files from input-dir
//!     merges runs in (term, doc_id) order
//!     fan-in controls how many runs merge at once
//!     a tournament tree picks the next posting during merges
//!     chunk-capacity controls postings per encoded chunk
//!     writes postings.bin and lexicon.bin in binary format
//!     stages build output in output-dir/.indexer-build first
//!     moves final .bin files into output-dir when done
//!
//! no hard failure for malformed postings, unsorted lines,
//! and duplicate (term, doc_id) entries. reported to stderr and skipped.

use std::io;
use std::path::PathBuf;
use std::time::Instant;

use clap::Parser;

mod builder; // run reader and k-way merge helpers
mod codec; // binary format constants and bit-packing helpers
mod tournament; //tournament tree for ordering postings
mod write; //writes out lexicon.bin / postings.bin

use write::IndexWriter;

//clap CLI & default config
#[derive(Parser)]
#[command(name = "indexer")]
struct Config {
    //where is the parser output, pulls all files with
    // shape "run_*.tsv" fr dir
    #[arg(long, default_value = "data/runs")]
    input_dir: PathBuf,

    //where are we writing the data to when index is built (.bin files)
    #[arg(long, default_value = "data/index")]
    output_dir: PathBuf,

    //# of postings for in-memory flush threshold &
    //on disk chunk size
    #[arg(long, default_value_t = 64)]
    chunk_capacity: u16,

    // k for k-way merge, >= 2 required
    #[arg(long, default_value_t = 32)]
    fan_in: usize,
}

// make an index plz
fn main() {
    let config = Config::parse();
    let start_time = Instant::now();
    if let Err(error) = build_index(&config) {
        eprintln!("index build failed: {error}");
        std::process::exit(1);
    }
    println!("Index compilation complete in {:.2?}", start_time.elapsed());
}

// make the index from parser run files
fn build_index(config: &Config) -> io::Result<()> {
    // k-way needs k >= 2
    if config.fan_in < 2 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "fan-in < 2"));
    }

    std::fs::create_dir_all(&config.output_dir)?;
    let build_dir = config.output_dir.join(".indexer-build");
    std::fs::create_dir_all(&build_dir)?;

    let result = (|| {
        let files = builder::read_directory(&config.input_dir)?;
        let (files, merge_malformed) = builder::merge_passes(files, config.fan_in, &build_dir)?;
        let mut writer = IndexWriter::new(&build_dir, config.chunk_capacity)?;
        let (postings_processed, final_malformed) =
            builder::merge_files(&files, |posting| writer.write_posting(posting))?;
        let malformed_postings = merge_malformed + final_malformed;
        writer.finish()?;
        [codec::POSTINGS_FILE, codec::LEXICON_FILE]
            .into_iter()
            .try_for_each(|name| {
                std::fs::rename(build_dir.join(name), config.output_dir.join(name))
            })?;

        let postings_size = std::fs::metadata(config.output_dir.join(codec::POSTINGS_FILE))?.len();
        let lexicon_size = std::fs::metadata(config.output_dir.join(codec::LEXICON_FILE))?.len();
        println!("index stats:");
        println!("  postings processed: {postings_processed}");
        println!("  malformed postings: {malformed_postings}");
        println!("  lexicon.bin: {lexicon_size} bytes");
        println!("  postings.bin: {postings_size} bytes");
        Ok(())
    })();

    result.and(std::fs::remove_dir_all(build_dir))
}
