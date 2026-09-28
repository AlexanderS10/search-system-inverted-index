use crate::tokenizer::tokenize;
use common::{CollectionStats, Posting};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};

const POSTINGS_LIMIT: usize = 5000000;

/// Read the file line by line and call out to parse, tokenize and record stats as well as byte offsets
/// Arguments:
///     input_path: the path to the main file to read which is collection.tsv in this case
/// Behavior:
///     It uses a read buffer so it does not ask the OS everytime for a buffer but I use the same one
///     This buffer read the a line at the time and gives the size so we recod the offset for snippets
///     It uses the POSTINGS_LIMIT to limit how many postings can be in memory before sorting them and writting them out.
///     It clears all buffers and reuses them so they are faster than doing it one at the time.
pub fn run_spimi(input_path: &str) -> std::io::Result<()> {
    //need to ensure the directories exist
    fs::create_dir_all("data/runs")?;
    fs::create_dir_all("data/index")?;

    //lets open it and create a buffer reader
    let input_file = File::open(input_path)?;
    let mut reader = BufReader::new(input_file);

    //open the metadata document doc_table.tsv
    let doc_table_file = File::create("data/index/doc_table.tsv")?;

    //I will add the offsets for snippets in a binary file
    let offset_file = File::create("data/index/passage_offsets.bin")?;
    let mut offset_writer = BufWriter::new(offset_file);

    let mut doc_table_writer = BufWriter::new(doc_table_file);

    let mut current_doc_id: u32 = 0;
    let mut run_number: u32 = 0;
    let mut total_tokens: u64 = 0;
    let mut postings: Vec<Posting> = Vec::with_capacity(POSTINGS_LIMIT);

    //read line by line now
    let mut current_offset: u64 = 0;
    let mut line_buffer = String::new();
    loop {
        let line_start_offset = current_offset;
        line_buffer.clear();
        let bytes_read = reader.read_line(&mut line_buffer)?;
        if bytes_read == 0 {
            break; //this is the end of the file
        }
        current_offset += bytes_read as u64;
        let Some((external_id, passage_text)) = parse_line(&line_buffer) else {
            continue;
        };

        let tokenized = tokenize(passage_text);
        //write the doc metadata with doc_id, external_id, doc_len
        writeln!(
            doc_table_writer,
            "{}\t{}\t{}",
            current_doc_id, external_id, tokenized.doc_len
        )?;

        offset_writer.write_all(&line_start_offset.to_le_bytes())?;

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
    offset_writer.flush()?;
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
