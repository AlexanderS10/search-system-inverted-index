use crate::tokenizer::tokenize;
use common::{CollectionStats, Posting};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

const POSTINGS_LIMIT: usize = 5000000;

pub fn run_spimi(input_path: &str) -> std::io::Result<()> {
    //need to ensure the directories exist
    fs::create_dir_all("data/runs")?;
    fs::create_dir_all("data/index")?;

    //lets open it and create a buffer reader
    let input_file = File::open(input_path)?;
    let reader = BufReader::new(input_file);

    //open the metadata document doc_table.tsv
    let doc_table_file = File::create("data/index/doc_table.tsv")?;
    let mut doc_table_writer = BufWriter::new(doc_table_file);

    let mut current_doc_id: u32 = 0;
    let mut run_number: u32 = 0;
    let mut total_tokens: u64 = 0;
    let mut postings: Vec<Posting> = Vec::with_capacity(POSTINGS_LIMIT);

    //read line by line now
    for line_result in reader.lines() {
        let line = line_result?;
        let Some((external_id, passage_text)) = parse_line(&line) else {
            continue;
        };

        let tokenized = tokenize(passage_text);
        //write the doc metadata with doc_id, external_id, doc_len
        writeln!(
            doc_table_writer,
            "{}\t{}\t{}",
            current_doc_id, external_id, tokenized.doc_len
        )?;

        total_tokens += tokenized.doc_len as u64;

        //turn the term counts into postings
        for (term, freq) in tokenized.term_counts {
            postings.push(Posting {
                term,
                doc_id: current_doc_id,
                freq,
            });
        }

        if postings.len() >= POSTINGS_LIMIT {
            flush_run(run_number, &mut postings)?;
            run_number += 1;
        }

        current_doc_id += 1;
    }
    //flush any remaining postings from the last chunk aka did not make it to 5mil
    if !postings.is_empty() {
        flush_run(run_number, &mut postings)?;
    }
    doc_table_writer.flush()?;

    //compute and write collection stats
    let total_docs = current_doc_id as u64;
    let avg_doc_len = if total_docs > 0 {
        total_tokens as f64 / total_docs as f64
    } else {
        0.0
    };
    let stats = CollectionStats {
        total_docs,
        total_tokens,
        avg_doc_len,
    };
    let stats_json = serde_json::to_string_pretty(&stats)?;
    fs::write("data/index/collection_stats.json", stats_json)?;

    println!(
        "Indexing complete! Indexed {} docs ({} tokens). Avg doc len: {:.2}",
        total_docs, total_tokens, avg_doc_len
    );

    return Ok(());
}

/// Helper: Split a line by the  tab
/// Returns (pasage_id, text) or None if the line is broken
fn parse_line(line: &str) -> Option<(u32, &str)> {
    let trimmed = line.trim_end();
    let (id_str, passage_text) = trimmed.split_once('\t')?;
    let external_id = id_str.parse::<u32>().ok()?;
    return Some((external_id, passage_text));
}

/// Helper: sorts the postings in memory and writes them out to data/runs/run_xxx.tsv
/// # Arguments:
///     run_number: Squential run id to name the output file
///     postings: a reference to the vector of postings in memory (mutable)
fn flush_run(run_number: u32, postings: &mut Vec<Posting>) -> std::io::Result<()> {
    if postings.is_empty() {
        return Ok(());
    }
    println!(
        "Flushing run {} ({} postings) to disk...",
        run_number,
        postings.len()
    );

    postings.sort(); //here is hwere the struct order kicks in automatically
    let filename = format!("data/runs/run_{:03}.tsv", run_number);
    let file = File::create(&filename)?;
    let mut writer = BufWriter::new(file);

    //write each term in the correct format
    for p in postings.iter() {
        writeln!(writer, "{}\t{}\t{}", p.term, p.doc_id, p.freq)?;
    }
    writer.flush()?;
    postings.clear();
    return Ok(());
}
