# Quickstart

Run every command from the repository root.

The current benchmark runner supports macOS and Linux. Windows requires
platform-specific process/RSS sampling and executable commands, so the
benchmark commands below are Unix-oriented.

## Full build and index

```bash
cargo build --release
mkdir -p bench_out data/runs data/queries
rm -f data/runs/run_*.tsv

target/release/parser > bench_out/parser.out 2> bench_out/parser_time.txt

target/release/indexer \
  --input-dir data/runs \
  --output-dir data/index \
  --chunk-capacity 64 \
  --fan-in 32 \
  > bench_out/indexer.out 2> bench_out/indexer_time.txt
```

The parser writes sorted run files under `data/runs/`. The indexer writes the
compressed index and metadata under `data/index/`.

## Benchmark setup

Place these files under `data/queries/`:

```text
queries.dev.small.tsv: qid<TAB>query text
qrels.dev.small.tsv:  qid iteration passage_id relevance
```

### Clear the file cache

Run the platform-specific command immediately before a cold benchmark.

macOS:

```bash
sudo purge
```

Linux:

```bash
sync
echo 3 | sudo tee /proc/sys/vm/drop_caches
```

Windows has no built-in `sudo purge` equivalent. Use Microsoft Sysinternals
RAMMap and choose **Empty → Empty Standby List**, or reboot before measuring.

### Run the benchmark

For independent cold AND and OR measurements, run each mode in a separate
process and clear the file cache before each mode. The loop below is for
macOS; use the Linux cache command above in place of `sudo purge` on Linux.
Snippets are disabled for the benchmark and the offsets file is restored at
the end:

```bash
mv data/index/passage_offsets.bin bench_out/passage_offsets.bin.disabled

for mode in and or; do
  sudo purge

  /usr/bin/time -l target/release/bench \
    --index-dir data/index \
    --queries data/queries/queries.dev.small.tsv \
    --qrels data/queries/qrels.dev.small.tsv \
    --mode "$mode" --warm-passes 2 --label "${mode}_cold" \
    > "bench_out/${mode}_cold.console.txt" \
    2> "bench_out/${mode}_cold.time.txt"
done

mv bench_out/passage_offsets.bin.disabled data/index/passage_offsets.bin
```

Results are in `bench_out/and_cold_summary.tsv` and
`bench_out/or_cold_summary.tsv`, plus the per-pass TSV files. They report
engine latency, end-to-end latency, MRR@10, empty rate, and RSS. Snippets are
disabled to avoid conflating query performance with snippet generation and
passage I/O; details are in `crates/bench/README.md`.

## Web UI

Start the query web server after building the index:

```bash
target/release/query --web 8080
```

Open `http://localhost:8080` in a browser. The default port is `8080`; pass a
different port to change it.
