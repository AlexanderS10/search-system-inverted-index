//!  merges sorted parser run files
//!
//! reads postings from run_*.tsv files and skips malformed or out-of-order rows
//! uses a tournament tree to merge runs by term and doc id
//! merges in bounded passes when the run count is larger than fan-in
//!
//! used by main.rs to send sorted postings to the index writer

use std::cmp::Ordering;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Lines, Write};
use std::path::{Path, PathBuf};

use common::Posting;

use crate::tournament::TournamentTree;

// sorting key def, term first then doc_id
fn key(posting: &Posting) -> (&str, u32) {
    (&posting.term, posting.doc_id)
}

// one run file participating in a merge
struct Run {
    path: PathBuf,
    reader: Lines<BufReader<File>>,
    // current posting in this run
    head: Option<Posting>,
    // counter to help w/ stderr messages
    line_number: usize,
    malformed_postings: usize,
}

impl Run {
    // open one file and prime its first valid posting
    fn open(path: PathBuf) -> io::Result<Self> {
        let reader = BufReader::new(File::open(&path)?).lines();
        let mut run = Self {
            path,
            reader,
            head: None,
            line_number: 0,
            malformed_postings: 0,
        };
        // read until first usable posting or EOF
        run.read_next(None)?;
        Ok(run)
    }

    // scan forward until next valid posting or EOF
    fn read_next(&mut self, after: Option<(&str, u32)>) -> io::Result<()> {
        // keep trucking through bad lines, no recursion here
        loop {
            // run exhausted
            let Some(line_result) = self.reader.next() else {
                self.head = None;
                return Ok(());
            };

            self.line_number += 1;
            let line = line_result?;
            // validate tsv shape
            let Some(posting) = parse_posting(&line) else {
                self.malformed_postings += 1;
                self.report_skip("malformed posting");
                continue;
            };
            // skip only this line if it breaks per-run ordering
            let issue = after.and_then(|previous| match key(&posting).cmp(&previous) {
                Ordering::Less => Some("unsorted posting"),
                Ordering::Equal => Some("duplicate (term, doc_id)"),
                Ordering::Greater => None,
            });
            if let Some(reason) = issue {
                self.report_skip(reason);
                continue;
            }

            // found the next head for this run
            self.head = Some(posting);
            return Ok(());
        }
    }

    // print a skipped input line with file + line number
    fn report_skip(&self, reason: &str) {
        let (line, path) = (self.line_number, self.path.display());
        eprintln!("{reason} at line {line} in {path}");
    }
}

// collapse many runs into <= fan_in files
pub(crate) fn merge_passes(
    mut files: Vec<PathBuf>,
    fan_in: usize,
    build_dir: &Path,
) -> io::Result<(Vec<PathBuf>, usize)> {
    let mut pass = 0;
    let mut malformed_postings = 0;
    while files.len() > fan_in {
        // each pass merges bounded batches into temp run files
        let mut next_files = Vec::with_capacity(files.len().div_ceil(fan_in));
        for (batch, paths) in files.chunks(fan_in).enumerate() {
            let output_path = build_dir.join(format!("merge_{pass:03}_{batch:06}.tsv"));
            let mut output = BufWriter::new(File::create(&output_path)?);
            // intermediate runs stay in parser tsv format
            let (_, batch_malformed) = merge_files(paths, |posting| {
                writeln!(
                    output,
                    "{}\t{}\t{}",
                    posting.term, posting.doc_id, posting.freq
                )
            })?;
            malformed_postings += batch_malformed;
            output.flush()?;
            next_files.push(output_path);
        }
        files = next_files;
        pass += 1;
    }
    Ok((files, malformed_postings))
}

// merge sorted run files and emit one clean sorted stream
pub(crate) fn merge_files(
    files: &[PathBuf],
    mut emit: impl FnMut(Posting) -> io::Result<()>,
) -> io::Result<(usize, usize)> {
    // open runs and prime each head posting
    let mut runs = Vec::with_capacity(files.len());
    for (index, path) in files.iter().cloned().enumerate() {
        runs.push(Run::open(path)?);
        let processed = index + 1;
        if processed % 10 == 0 || processed == files.len() {
            println!("Processed {processed}/{} run files", files.len());
        }
    }
    let mut tree = TournamentTree::build(runs.len(), |index| head_key(&runs, index));

    // keep taking the smallest run head until all runs empty
    let mut postings_processed = 0;
    while let Some(run_index) = tree.winner() {
        let posting = advance(&mut runs, run_index, &mut tree)?;

        // discard duplicate keys from other runs
        while let Some(other_run) = tree
            .winner()
            .filter(|&index| head_key(&runs, index) == Some(key(&posting)))
        {
            runs[other_run].report_skip("duplicate (term, doc_id) across runs");
            advance(&mut runs, other_run, &mut tree)?;
        }

        emit(posting)?;
        postings_processed += 1;
    }
    Ok((
        postings_processed,
        runs.iter().map(|run| run.malformed_postings).sum(),
    ))
}

// take current head from one run, advance it, update tree
fn advance(runs: &mut [Run], run_index: usize, tree: &mut TournamentTree) -> io::Result<Posting> {
    let posting = runs[run_index]
        .head
        .take()
        .expect("tournament winner has a posting");
    runs[run_index].read_next(Some(key(&posting)))?;
    tree.update(run_index, |index| head_key(runs, index));
    Ok(posting)
}

// current sort key for a run's head posting
fn head_key(runs: &[Run], index: usize) -> Option<(&str, u32)> {
    runs[index].head.as_ref().map(key)
}

// find parser output files in stable order
pub(crate) fn read_directory(path: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = std::fs::read_dir(path)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<Vec<_>>>()?;
    files.retain(|path| {
        path.file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("run_"))
            && path.extension().is_some_and(|ext| ext == "tsv")
    });
    files.sort();
    Ok(files)
}

// parse one parser tsv posting line
pub(crate) fn parse_posting(line: &str) -> Option<Posting> {
    let mut fields = line.split('\t');
    let term = fields.next()?;
    let doc_id = fields.next()?.parse::<u32>().ok()?;
    let freq = fields.next()?.parse::<u32>().ok()?;
    let valid = fields.next().is_none() && !term.is_empty() && freq > 0;
    valid.then(|| Posting {
        term: term.to_owned(),
        doc_id,
        freq,
    })
}
