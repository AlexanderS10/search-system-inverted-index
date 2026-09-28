mod tokenizer;
mod spimi;
use std::env;
use std::time::Instant;


fn main()->std::io::Result<()> {
    //For testing of files I am allowing for other relative paths why not
    let args:Vec<String>=env::args().collect();
    let default_path = "data/raw/collection.tsv".to_string();
    let input_path = args.get(1).unwrap_or(&default_path);

    println!("Starting the SPIMI in {}", input_path);
    let start_time = Instant::now();

    spimi::run_spimi(input_path)?;

    let elapsed_time = start_time.elapsed();
    println!("The total time taken is {:.2?}", elapsed_time);
    return Ok(());

}

