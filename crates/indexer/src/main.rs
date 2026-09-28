use indexer::Config;

fn main() {
    let config = Config::from_args();
    println!("indexer configuration");
    println!("  input directory:  {}", config.input_dir.display());
    println!("  output directory: {}", config.output_dir.display());
    println!("  chunk capacity:   {}", config.chunk_capacity);
    println!("  merge fan-in:     {}", config.fan_in);
}
