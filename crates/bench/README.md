# Query benchmark

Build from the repository root:

```bash
mkdir -p bench_out

/usr/bin/time -l cargo build --release \
  > bench_out/build.out 2> bench_out/build_time.txt
```

On Linux, use `/usr/bin/time -v` instead of `/usr/bin/time -l`.
`build.out` contains compiler output. `build_time.txt` contains the external
wall-clock and resource report. Read `elapsed` and `maximum resident set size`
on macOS, or `Elapsed (wall clock) time` and `Maximum resident set size` on
Linux.

Inputs:

```text
data/queries/queries.dev.small.tsv   qid<TAB>query text
data/queries/qrels.dev.small.tsv     qid  iteration  passage_id  relevance
```

Download the MS MARCO small development sets into those exact paths:

```bash
mkdir -p data/queries
curl -fL --retry 3 \
  https://msmarco.blob.core.windows.net/msmarcoranking/queries.dev.small.tsv \
  -o data/queries/queries.dev.small.tsv
curl -fL --retry 3 \
  https://msmarco.blob.core.windows.net/msmarcoranking/qrels.dev.small.tsv \
  -o data/queries/qrels.dev.small.tsv
```

Check that the files are TSVs before benchmarking:

```bash
head -n 1 data/queries/queries.dev.small.tsv
head -n 1 data/queries/qrels.dev.small.tsv
```

The first command should not print an HTML document.

For independent cold AND and OR measurements, run each mode separately and
clear the file cache before each mode. The query REPL enables snippet
generation when the offsets file is present, so disable it during the run:

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

On Linux, replace `sudo purge` with:

```bash
sync
echo 3 | sudo tee /proc/sys/vm/drop_caches
```

Snippet generation adds passage-file I/O and result-processing work. Disabling
it keeps the measurement focused on query execution without conflating search
performance with snippet generation.

The console and TSV output report engine latency, end-to-end REPL round-trip
latency, MRR@10, empty-result rate, and per-pass RSS. Results are written to
`bench_out/<label>_summary.tsv` and per-mode/per-pass TSV files. For peak RSS
and wall time, wrap the command with `/usr/bin/time -l`.

The raw macOS `time -l` file may contain unsupported resource fields reported
as zero. Keep it as provenance, but use the benchmark's `rss_mb` column for
query-process memory: `/usr/bin/time -l` measures the benchmark parent, while
`rss_mb` samples the spawned query child.
