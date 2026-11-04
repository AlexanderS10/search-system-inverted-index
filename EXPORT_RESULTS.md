# BM25 top-K export timing

Hardware: MacBook Air (Apple M3, 8 CPU cores, 16 GB RAM).

The inverted index was built once; K changes how many ranked passages are exported per query. Each cell is one release-build run in seconds, with snippets disabled. Timing includes query filtering, search, and writing the run file, but excludes opening the index.

| K | Dev OR (1,000 queries) | Dev AND (1,000) | Eval one OR (43) | Eval one AND (43) | Eval two OR (54) | Eval two AND (54) |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 100 | 12.141 | 0.035 | 0.547 | 0.008 | 0.563 | 0.009 |
| 200 | 11.891 | 0.033 | 0.557 | 0.008 | 0.562 | 0.009 |
| 500 | 12.474 | 0.033 | 0.523 | 0.008 | 0.572 | 0.009 |
| 1,000 | 12.604 | 0.033 | 0.543 | 0.008 | 0.585 | 0.009 |
