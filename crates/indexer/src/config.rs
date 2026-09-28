use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
#[command(name = "indexer")]
pub struct Config {
    #[arg(
        long,
        env = "INDEXER_INPUT_DIR",
        default_value = "data/fixtures/indexer"
    )]
    pub input_dir: PathBuf,

    #[arg(long, env = "INDEXER_OUTPUT_DIR", default_value = "data/index")]
    pub output_dir: PathBuf,

    #[arg(long, env = "INDEXER_CHUNK_CAPACITY", default_value_t = 64)]
    pub chunk_capacity: u16,

    #[arg(long, env = "INDEXER_FAN_IN", default_value_t = 32)]
    pub fan_in: usize,
}

impl Config {
    pub fn from_args() -> Self {
        dotenvy::dotenv().ok();
        Self::parse()
    }
}
