//! black-box benchmark for the query repl
//!
//! build and run compiled binaries directly:
//! cargo build --release
//! sudo purge
//! /usr/bin/time -l target/release/bench --index-dir data/index \
//!   --queries data/queries/queries.dev.small.tsv \
//!   --qrels data/queries/qrels.dev.small.tsv \
//!   --mode both --warm-passes 2 --label baseline 2> bench_out/query_time.txt
//!
//! macos reports maximum resident set size in bytes
//! page reclaims and page faults are approximate
//! rss growth and cold-vs-warm latency are primary evidence
//! wrapper uses only the query repl

use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::{self, File};
use std::io::{self, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Instant;

#[derive(Clone, Copy)]
enum Mode { And, Or }

impl Mode {
    // returns the mode label used in output files.
    fn name(self) -> &'static str { match self { Self::And => "and", Self::Or => "or" } }

    // returns the command understood by the query repl.
    fn command(self) -> &'static str { match self { Self::And => "AND", Self::Or => "OR" } }
}

struct Args {
    index_dir: PathBuf,
    queries: PathBuf,
    qrels: Option<PathBuf>,
    mode: String,
    warm_passes: usize,
    sample: Option<usize>,
    label: String,
}

struct Query { qid: String, text: String }

struct Reply { engine_ms: f64, ids: Vec<String>, empty: bool }

fn usage() -> ! {
    // prints the command-line form and exits on invalid input.
    eprintln!("usage: bench --index-dir DIR --queries FILE [--qrels FILE] [--mode and|or|both] [--warm-passes N] [--sample N] [--label NAME]");
    std::process::exit(2)
}

fn args() -> Args {
    // parses benchmark options and applies the default paths and settings.
    let mut result = Args { index_dir: PathBuf::from("data/index"), queries: PathBuf::from("data/queries/queries.dev.small.tsv"), qrels: None, mode: "both".into(), warm_passes: 2, sample: None, label: "bench".into() };
    let mut it = env::args().skip(1);
    while let Some(arg) = it.next() {
        let value = |it: &mut std::iter::Skip<env::Args>| it.next().unwrap_or_else(|| usage());
        match arg.as_str() {
            "--index-dir" => result.index_dir = PathBuf::from(value(&mut it)),
            "--queries" => result.queries = PathBuf::from(value(&mut it)),
            "--qrels" => result.qrels = Some(PathBuf::from(value(&mut it))),
            "--mode" => result.mode = value(&mut it),
            "--warm-passes" => result.warm_passes = value(&mut it).parse().unwrap_or_else(|_| usage()),
            "--sample" => result.sample = Some(value(&mut it).parse().unwrap_or_else(|_| usage())),
            "--label" => result.label = value(&mut it),
            _ => usage(),
        }
    }
    result
}

fn load_queries(path: &Path, sample: Option<usize>) -> io::Result<Vec<Query>> {
    // loads query ids and text from the tab-separated query file.
    let text = fs::read_to_string(path)?;
    Ok(text.lines().filter_map(|line| {
        let (qid, text) = line.split_once('\t')?;
        Some(Query { qid: qid.to_owned(), text: text.to_owned() })
    }).take(sample.unwrap_or(usize::MAX)).collect())
}

fn load_qrels(path: Option<&Path>) -> io::Result<HashMap<String, HashSet<String>>> {
    // loads positive relevance ids grouped by query id.
    let mut result = HashMap::new();
    let Some(path) = path else { return Ok(result) };
    for line in fs::read_to_string(path)?.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 4 || fields[3].parse::<i32>().unwrap_or(0) <= 0 { continue; }
        result.entry(fields[0].to_owned()).or_insert_with(HashSet::new).insert(fields[2].to_owned());
    }
    Ok(result)
}

fn spawn_query(index_dir: &Path) -> io::Result<(Child, ChildStdin, BufReader<ChildStdout>)> {
    // starts the existing query repl and returns its stdin and stdout pipes.
    if index_dir != Path::new("data/index") {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "the existing query REPL uses the fixed relative path data/index; --index-dir must be data/index"));
    }
    let mut child = Command::new("target/release/query")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit()).spawn()?;
    let stdin = child.stdin.take().ok_or_else(|| io::Error::other("query stdin unavailable"))?;
    let stdout = child.stdout.take().ok_or_else(|| io::Error::other("query stdout unavailable"))?;
    Ok((child, stdin, BufReader::new(stdout)))
}

fn read_prompt(reader: &mut BufReader<ChildStdout>) -> io::Result<String> {
    // reads repl output until the next prompt and returns the response body.
    let marker = b"search > ";
    let mut data = Vec::new();
    let mut window = Vec::new();
    loop {
        let mut byte = [0u8; 1];
        if reader.read_exact(&mut byte).is_err() { return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "query exited before search prompt")); }
        data.push(byte[0]);
        window.push(byte[0]);
        if window.len() > marker.len() { window.remove(0); }
        if window == marker { data.truncate(data.len() - marker.len()); return String::from_utf8(data).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "query output was not UTF-8")); }
    }
}

fn parse_reply(text: &str) -> io::Result<Reply> {
    // extracts engine time, result ids, and empty-result state from repl output.
    let mut engine_ms = None;
    let mut ids = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Found ") {
            let Some((_, rest)) = rest.split_once(" in ") else { continue };
            let Some(value) = rest.split_once(" ms") else { continue };
            engine_ms = value.0.trim().parse().ok();
        }
        if let Some(rest) = line.split_once("[Passage ID: ").map(|x| x.1) {
            let Some((id, _)) = rest.split_once(']') else { return Err(io::Error::new(io::ErrorKind::InvalidData, "malformed passage result")); };
            ids.push(id.trim().to_owned());
        }
    }
    let empty = text.contains("No matching passages found");
    if engine_ms.is_none() && !empty { return Err(io::Error::new(io::ErrorKind::InvalidData, format!("unparseable query response: {text}"))); }
    Ok(Reply { engine_ms: engine_ms.unwrap_or(0.0), ids, empty })
}

fn rss_kb(pid: u32) -> io::Result<u64> {
    // samples the query child's resident memory through ps.
    let output = Command::new("ps").args(["-o", "rss=", "-p", &pid.to_string()]).output()?;
    if !output.status.success() { return Err(io::Error::other("ps failed")); }
    String::from_utf8_lossy(&output.stdout).trim().parse().map_err(|_| io::Error::other("could not parse ps RSS"))
}

fn run_pass(stdin: &mut ChildStdin, reader: &mut BufReader<ChildStdout>, pid: u32, queries: &[Query], mode: Mode, qrels: &HashMap<String, HashSet<String>>) -> io::Result<(f64, f64, f64, usize, f64)> {
    // runs every query once and averages latency, mrr, empty rate, and rss.
    let mut engine_total = 0.0;
    let mut round_trip_total = 0.0;
    let mut reciprocal = 0.0;
    let mut empty = 0;
    for query in queries {
        let start = Instant::now();
        writeln!(stdin, "{} {}", mode.command(), query.text)?;
        stdin.flush()?;
        let reply = parse_reply(&read_prompt(reader)?)?;
        engine_total += reply.engine_ms;
        empty += usize::from(reply.empty);
        if let Some(relevant) = qrels.get(&query.qid) {
            if let Some(rank) = reply.ids.iter().position(|id| relevant.contains(id)) { reciprocal += 1.0 / (rank + 1) as f64; }
        }
        round_trip_total += start.elapsed().as_secs_f64() * 1000.0;
    }
    let rss = rss_kb(pid).unwrap_or(0);
    let n = queries.len().max(1) as f64;
    Ok((engine_total / n, round_trip_total / n, reciprocal / n, empty, rss as f64 / 1024.0))
}

fn main() -> io::Result<()> {
    // loads inputs, runs each requested mode across cold and warm passes, and writes results.
    let args = args();
    let queries = load_queries(&args.queries, args.sample)?;
    let qrels = load_qrels(args.qrels.as_deref())?;
    let modes: Vec<Mode> = match args.mode.as_str() { "and" => vec![Mode::And], "or" => vec![Mode::Or], "both" => vec![Mode::And, Mode::Or], _ => usage() };
    let out_dir = PathBuf::from("bench_out");
    fs::create_dir_all(&out_dir)?;
    let mut summary = File::create(out_dir.join(format!("{}_summary.tsv", args.label)))?;
    writeln!(summary, "mode\tpass\tengine_ms\tround_trip_ms\tmrr@10\tempty_rate\trss_mb")?;
    for mode in modes {
        let (mut child, mut stdin, mut reader) = spawn_query(&args.index_dir)?;
        let _ = read_prompt(&mut reader)?;
        println!("mode\tpass\tengine_ms\tround_trip_ms\tmrr@10\tempty\trss_mb");
        for pass in 0..=args.warm_passes {
            let name = if pass == 0 { "first".to_owned() } else { format!("warm{pass}") };
            let (engine, wall, mrr, empty, rss) = run_pass(&mut stdin, &mut reader, child.id(), &queries, mode, &qrels)?;
            println!("{}\t{}\t{:.2}\t{:.2}\t{:.4}\t{:.4}\t{:.1}", mode.name(), name, engine, wall, mrr, empty as f64 / queries.len().max(1) as f64, rss);
            let path = out_dir.join(format!("{}_{}_{}.tsv", args.label, mode.name(), name));
            let mut file = File::create(path)?;
            let empty_rate = empty as f64 / queries.len().max(1) as f64;
            writeln!(file, "mode\tpass\tengine_ms\tround_trip_ms\tmrr@10\tempty_rate\trss_mb")?;
            writeln!(file, "{}\t{}\t{engine:.2}\t{wall:.2}\t{mrr:.4}\t{empty_rate:.4}\t{rss:.1}", mode.name(), name)?;
            writeln!(summary, "{}\t{}\t{engine:.2}\t{wall:.2}\t{mrr:.4}\t{empty_rate:.4}\t{rss:.1}", mode.name(), name)?;
        }
        writeln!(stdin, "exit")?;
        stdin.flush()?;
        let _ = child.wait();
    }
    Ok(())
}
