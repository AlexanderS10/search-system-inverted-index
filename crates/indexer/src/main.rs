use std::io;
use std::path::PathBuf;

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
    #[arg(long, default_value = "data/fixtures/indexer")]
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
    if let Err(error) = build_index(&config) {
        eprintln!("index build failed: {error}");
        std::process::exit(1);
    }
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
        let files = builder::merge_passes(files, config.fan_in, &build_dir)?;
        let mut writer = IndexWriter::new(&build_dir, config.chunk_capacity)?;
        builder::merge_files(&files, |posting| writer.write_posting(posting))?;
        writer.finish()?;
        ["postings.bin", "lexicon.bin"]
            .into_iter()
            .try_for_each(|name| {
                std::fs::rename(build_dir.join(name), config.output_dir.join(name))
            })
    })();

    result.and(std::fs::remove_dir_all(build_dir))
}
