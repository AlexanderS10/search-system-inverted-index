# Indexer

Build the index from parser run files:

```bash
cargo build --release
target/release/indexer \
  --input-dir data/runs \
  --output-dir data/index \
  --chunk-capacity 64 \
  --fan-in 32
```

The indexer reads sorted `data/runs/run_*.tsv` files and writes:

```text
data/index/postings.bin
data/index/lexicon.bin
```

The chunk capacity controls postings per encoded chunk. The reader and writer
must use the same index format version and chunk metadata.
