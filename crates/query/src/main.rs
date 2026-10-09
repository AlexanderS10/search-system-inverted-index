//! MS MARCO Interactive Search Engine
//!
//! Executable 3 for Assignment #2
//! Supports both interactive terminal REPL and browser Web UI mode

mod bm25;
mod doc_table;
mod engine;
mod search;
mod server;
mod snippets;
mod tokenizer;
mod top_k;

use engine::{QueryResponse, SearchEngine};
use search::QueryMode;
use std::env;
use std::io::{self, BufRead, Write};
use std::sync::{Arc, Mutex};

/// Entrypoint for the query processor
///
/// Behavior:
///     1. Initializes SearchEngine loading the index, doc table, and snippet generator
///     2. If --web is passed, starts the Web UI server
///     3. Otherwise, starts the interactive command-line REPL
fn main() -> io::Result<()> {
    println!("MS MARCO Search Engine (DAAT / BM25)");

    // Initialize core search engine
    let engine = SearchEngine::open(
        "data/index",
        "data/index/collection_stats.json",
        "data/index/doc_table.tsv",
        "data/index/passage_offsets.bin",
        "data/raw/collection.tsv",
    )?;

    if engine.has_snippets() {
        println!(
            "Loaded {} passages (avg len: {:.2}) [snippets active]",
            engine.doc_count(),
            engine.stats().avg_doc_len,
        );
    } else {
        println!(
            "Loaded {} passages (avg len: {:.2})",
            engine.doc_count(),
            engine.stats().avg_doc_len,
        );
    }

    // Check if web mode was requested via command line arguments
    let args: Vec<String> = env::args().collect();
    if let Some(web_idx) = args.iter().position(|a| a == "--web" || a == "-w") {
        let port: u16 = args
            .get(web_idx + 1)
            .and_then(|p| p.parse().ok())
            .unwrap_or(8080);

        let shared_engine = Arc::new(Mutex::new(engine));
        return server::run_web_server(shared_engine, port);
    }

    // Default: run terminal REPL
    return run_repl(engine);
}

/// Runs the interactive terminal REPL
///
/// Arguments:
///     engine: mutable SearchEngine instance
fn run_repl(mut engine: SearchEngine) -> io::Result<()> {
    println!("Ready for queries (<AND|OR> <query terms> or 'exit'):\n");

    let stdin = io::stdin();

    loop {
        print!("search > ");
        io::stdout().flush()?;

        let mut input_line = String::new();
        if stdin.lock().read_line(&mut input_line)? == 0 {
            // End of input stream (Ctrl+D)
            println!("\nGoodbye!");
            break;
        }

        let trimmed = input_line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.eq_ignore_ascii_case("quit") || trimmed.eq_ignore_ascii_case("exit") {
            println!("Goodbye!");
            break;
        }

        // Format is: <AND|OR> <query terms>
        let Some((mode_str, query_text)) = trimmed.split_once(' ') else {
            println!("! Usage: <AND|OR> <query terms> (e.g. AND machine learning)\n");
            continue;
        };

        let mode = if mode_str.eq_ignore_ascii_case("and") {
            QueryMode::And
        } else if mode_str.eq_ignore_ascii_case("or") {
            QueryMode::Or
        } else {
            println!("! Unknown mode '{}'. Use AND or OR\n", mode_str);
            continue;
        };

        let response = engine.search(query_text, mode, 10)?;
        print_repl_results(&response);
    }

    return Ok(());
}

/// Prints formatted search results in the terminal
///
/// Arguments:
///     response: QueryResponse containing ranked matches
fn print_repl_results(response: &QueryResponse) {
    if response.results.is_empty() {
        println!("No matching passages found\n");
        return;
    }

    println!(
        "\nFound {} matches in {:.2} ms (terms: {:?}):",
        response.results.len(),
        response.latency_ms,
        response.terms,
    );

    for item in &response.results {
        println!(
            "#{:<2} [Passage ID: {:<8}] [Score: {:<7.4}] [Tokens: {:<3}]",
            item.rank, item.external_id, item.score, item.doc_len,
        );

        if let Some(ref snippet) = item.snippet {
            if !snippet.is_empty() {
                println!("    \"{}\"", snippet);
            }
        }
    }
    println!();
}
