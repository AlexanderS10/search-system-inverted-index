//! batch BM25 candidates for the assignment queries
//!
//! run from the index_proj root after building the subset index:
//! target/release/query --batch QUERIES.tsv QRELS.tsv OUTPUT.run K [or|and]
//!
//! 3 and four column qrels work
//! K == BM25 candidate count per query (1000 for reranking).
//! no mode means OR; add `and` after K for a separate AND run.

use crate::{engine::SearchEngine, search::QueryMode};
use std::fs::{self, File};
use std::io::ErrorKind::{InvalidData, InvalidInput};
use std::io::{self, BufWriter, Error, Write};
use std::num::NonZeroUsize;
use std::path::Path;
use std::{collections::HashSet, env};
use std::time::Instant;

const USAGE: &str = "usage: --batch QUERIES QRELS OUTPUT K [or|and]";

pub fn run(engine: &mut SearchEngine) -> io::Result<()> {
    let args: Vec<_> = env::args().collect();
    let [_, _, queries, qrels, output, k, mode_args @ ..] = args.as_slice() else {
        return Err(Error::new(InvalidInput, USAGE));
    };
    let (mode, run_name) = match mode_args {
        [] => (QueryMode::Or, "bm25_or"),
        [arg] if arg == "or" => (QueryMode::Or, "bm25_or"),
        [arg] if arg == "and" => (QueryMode::And, "bm25_and"),
        _ => return Err(Error::new(InvalidInput, USAGE)),
    };
    let k: NonZeroUsize = k.parse().map_err(|e| Error::new(InvalidInput, e))?;
    let started = Instant::now();

    let qrel_text = fs::read_to_string(qrels)?;
    // use qrels as query filter not their labels
    let mut remaining = HashSet::new();
    for line in qrel_text.lines() {
        let qid = line
            .split_whitespace()
            .next()
            .ok_or_else(|| Error::new(InvalidData, "empty qrels row"))?;
        remaining.insert(qid);
    }
    if remaining.is_empty() {
        return Err(Error::new(InvalidData, "no query IDs in qrels"));
    }

    let output = Path::new(output);
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }

    // stage the file so a failed batch does not replace a good run.
    let temporary = output.with_extension("run.tmp");
    let mut writer = BufWriter::new(File::create(&temporary)?);
    let mut query_count = 0;

    // no need to fetch passage text for every candidate.
    engine.disable_snippets();

    for line in fs::read_to_string(queries)?.lines() {
        let (qid, query) = line
            .split_once('\t')
            .ok_or_else(|| Error::new(InvalidData, "query row needs qid and text"))?;
        if !remaining.remove(qid) {
            continue;
        }
        query_count += 1;
        // OR gives a wider pool; AND keeps passages with every query term.
        for item in engine.search(query, mode, k.get())?.results {
            let (pid, rank, score) = (item.external_id, item.rank, item.score);
            writeln!(writer, "{qid}\tQ0\t{pid}\t{rank}\t{score:.10}\t{run_name}")?;
        }
    }

    // catch judged queries with no query text before publishing the run.
    if !remaining.is_empty() {
        return Err(Error::new(
            InvalidData,
            format!("{} qrels IDs missing in queries", remaining.len()),
        ));
    }

    writer.flush()?;
    drop(writer);
    fs::rename(temporary, output)?;
    println!("Exported {query_count} queries in {:.3} s", started.elapsed().as_secs_f64());
    println!("Wrote TREC run to {}", output.display());
    Ok(())
}
