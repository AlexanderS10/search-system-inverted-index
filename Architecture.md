# System Architecture

## Overview
The search engine is split into three separate executables that communicate through files on disk:
1. `parser`: Reads raw passages, tokenizes, and outputs sorted intermediate run files + document metadata.
2. `indexer`: Merges intermediate runs and builds the final compressed inverted index.
3. `query`: Interactive query interface that runs DAAT search and BM25 ranking.

---

## 1. Parser (Executable 1)

### What it does
- Streams `collection.tsv` line-by-line using buffered I/O.
- Line format: `passage_id \t passage_text`.
- Assigns continuous doc IDs starting from 0 (`doc_id = 0, 1, 2, ...`).
- Cleans and tokenizes text:
  - Lowercase all words.
  - Split on non-alphanumeric characters (UTF-8 safe).
  - Filter out terms with length < 2 or > 30, and terms with more than 5 digits.
- Counts term frequency for each document.
- Accumulates postings `(term, doc_id, freq)` in a memory buffer.
- When buffer is full (e.g. ~5 million postings), sorts the buffer by `term` ASC then `doc_id` ASC, and writes out a sorted run file (`run_XXX.tsv`).
- Emits document table, collection stats, and passage byte offsets.

### Output Files (Handoff to Indexer)

All output files are written to `data/runs/` and `data/index/`.

#### 1. Intermediate Runs (`data/runs/run_XXX.tsv`)
- TSV format, sorted primary by `term` (alphabetical A-Z), secondary by `doc_id` (ascending).
- Schema:
  ```tsv
  term<TAB>doc_id<TAB>term_freq
  ```
- Example:
  ```tsv
  algorithm	12	2
  algorithm	45	1
  apple	3	1
  apple	14	4
  banana	8	2
  ```

#### 2. Document Table (`data/index/doc_table.tsv`)
- Maps internal `doc_id` to external MS MARCO passage ID and doc token length (needed for BM25).
- Schema:
  ```tsv
  doc_id<TAB>external_passage_id<TAB>doc_len
  ```
- Example:
  ```tsv
  0	8234125	48
  1	194821	72
  ```

#### 3. Collection Stats (`data/index/collection_stats.json`)
- Global numbers needed for BM25 calculation:
  ```json
  {
    "total_docs": 8841823,
    "total_tokens": 523912041,
    "avg_doc_len": 59.25
  }
  ```

#### 4. Passage Byte Offsets (`data/index/passage_offsets.bin`)
- Binary array of `u64` little-endian file byte offsets into `collection.tsv`.
- Offset for doc `i` is stored at byte index `i * 8`.
- Lets the query engine do $O(1)$ seeks into `collection.tsv` to grab raw passage text for snippets.

---

## 2. Indexer (Executable 2)


---

## 3. Query Processor (Executable 3)
*(TInverted Index API, DAAT AND/OR query traversal, BM25 ranking, and CLI)*
