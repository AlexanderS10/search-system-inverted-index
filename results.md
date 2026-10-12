# Benchmark results

Results obtained on a MacBook Air with an Apple M5 chip, 16 GB RAM, and 512 GB storage.

Some fields in the macOS `/usr/bin/time -l` output are zero because macOS does
not report those resource categories for this process. The raw timing files
are retained for provenance; query memory is reported by the benchmark's
`rss_mb` field, which samples the spawned query process.

## `bench_out/and_cold_run.tsv`

```text
mode	pass	engine_ms	round_trip_ms	mrr@10	empty_rate	rss_mb
and	first	0.19	0.33	0.0613	0.5318	540.6
```

## `bench_out/and_summary.tsv`

```text
mode	pass	engine_ms	round_trip_ms	mrr@10	empty_rate	rss_mb
and	first	0.19	0.33	0.0613	0.5318	540.6
and	warm1	0.11	0.18	0.0613	0.5318	540.6
and	warm2	0.11	0.18	0.0613	0.5318	540.6
```

## `bench_out/and_warm1.tsv`

```text
mode	pass	engine_ms	round_trip_ms	mrr@10	empty_rate	rss_mb
and	warm1	0.11	0.18	0.0613	0.5318	540.6
```

## `bench_out/and_warm2.tsv`

```text
mode	pass	engine_ms	round_trip_ms	mrr@10	empty_rate	rss_mb
and	warm2	0.11	0.18	0.0613	0.5318	540.6
```

## `bench_out/baseline.console.txt`

```text
```

## `bench_out/fan32_indexer.time.txt`

```text
      116.72 real       106.49 user         4.36 sys
             5144576  maximum resident set size
                   0  average shared memory size
                   0  average unshared data size
                   0  average unshared stack size
                 478  page reclaims
                   1  page faults
                   0  swaps
                   0  block input operations
                   2  block output operations
                   0  messages sent
                   0  messages received
                   0  signals received
               13463  voluntary context switches
               45982  involuntary context switches
       2200574020576  instructions retired
        414418920046  cycles elapsed
             4555584  peak memory footprint
```

## `bench_out/or_cold_run.tsv`

```text
mode	pass	engine_ms	round_trip_ms	mrr@10	empty_rate	rss_mb
or	first	108.07	108.10	0.1772	0.0000	565.4
```

## `bench_out/or_summary.tsv`

```text
mode	pass	engine_ms	round_trip_ms	mrr@10	empty_rate	rss_mb
or	first	108.07	108.10	0.1772	0.0000	565.4
or	warm1	108.43	108.48	0.1772	0.0000	565.4
or	warm2	107.95	107.99	0.1772	0.0000	565.4
```

## `bench_out/or_warm1.tsv`

```text
mode	pass	engine_ms	round_trip_ms	mrr@10	empty_rate	rss_mb
or	warm1	108.43	108.48	0.1772	0.0000	565.4
```

## `bench_out/or_warm2.tsv`

```text
mode	pass	engine_ms	round_trip_ms	mrr@10	empty_rate	rss_mb
or	warm2	107.95	107.99	0.1772	0.0000	565.4
```

## `bench_out/parser_time.txt`

```text
      146.26 real       137.46 user         5.32 sys
           327073792  maximum resident set size
                   0  average shared memory size
                   0  average unshared data size
                   0  average unshared stack size
              350377  page reclaims
                   0  page faults
                   0  swaps
                   0  block input operations
                   0  block output operations
                   0  messages sent
                   0  messages received
                   0  signals received
                3570  voluntary context switches
              119126  involuntary context switches
       1514053467190  instructions retired
        534884310255  cycles elapsed
           323012096  peak memory footprint
```

## `bench_out/parser.out`

```text
Starting the SPIMI in data/raw/collection.tsv
Flushing run 0 (5000019 postings) to disk...
Flushing run 1 (5000007 postings) to disk...
Flushing run 2 (5000030 postings) to disk...
Flushing run 3 (5000024 postings) to disk...
Flushing run 4 (5000021 postings) to disk...
Flushing run 5 (5000018 postings) to disk...
Flushing run 6 (5000010 postings) to disk...
Flushing run 7 (5000017 postings) to disk...
Flushing run 8 (5000013 postings) to disk...
Flushing run 9 (5000037 postings) to disk...
Flushing run 10 (5000040 postings) to disk...
Flushing run 11 (5000016 postings) to disk...
Flushing run 12 (5000022 postings) to disk...
Flushing run 13 (5000004 postings) to disk...
Flushing run 14 (5000029 postings) to disk...
Flushing run 15 (5000018 postings) to disk...
Flushing run 16 (5000024 postings) to disk...
Flushing run 17 (5000019 postings) to disk...
Flushing run 18 (5000030 postings) to disk...
Flushing run 19 (5000013 postings) to disk...
Flushing run 20 (5000003 postings) to disk...
Flushing run 21 (5000010 postings) to disk...
Flushing run 22 (5000018 postings) to disk...
Flushing run 23 (5000006 postings) to disk...
Flushing run 24 (5000038 postings) to disk...
Flushing run 25 (5000026 postings) to disk...
Flushing run 26 (5000006 postings) to disk...
Flushing run 27 (5000027 postings) to disk...
Flushing run 28 (5000023 postings) to disk...
Flushing run 29 (5000019 postings) to disk...
Flushing run 30 (5000012 postings) to disk...
Flushing run 31 (5000031 postings) to disk...
Flushing run 32 (5000007 postings) to disk...
Flushing run 33 (5000007 postings) to disk...
Flushing run 34 (5000002 postings) to disk...
Flushing run 35 (5000000 postings) to disk...
Flushing run 36 (5000007 postings) to disk...
Flushing run 37 (5000018 postings) to disk...
Flushing run 38 (5000007 postings) to disk...
Flushing run 39 (5000004 postings) to disk...
Flushing run 40 (5000024 postings) to disk...
Flushing run 41 (5000003 postings) to disk...
Flushing run 42 (5000055 postings) to disk...
Flushing run 43 (5000001 postings) to disk...
Flushing run 44 (5000028 postings) to disk...
Flushing run 45 (5000015 postings) to disk...
Flushing run 46 (5000003 postings) to disk...
Flushing run 47 (5000050 postings) to disk...
Flushing run 48 (5000012 postings) to disk...
Flushing run 49 (5000014 postings) to disk...
Flushing run 50 (5000014 postings) to disk...
Flushing run 51 (5000004 postings) to disk...
Flushing run 52 (5000011 postings) to disk...
Flushing run 53 (5000003 postings) to disk...
Flushing run 54 (5000015 postings) to disk...
Flushing run 55 (5000005 postings) to disk...
Flushing run 56 (5000031 postings) to disk...
Flushing run 57 (5000019 postings) to disk...
Flushing run 58 (5000027 postings) to disk...
Flushing run 59 (5000053 postings) to disk...
Flushing run 60 (5000005 postings) to disk...
Flushing run 61 (5000027 postings) to disk...
Flushing run 62 (5000026 postings) to disk...
Flushing run 63 (5000002 postings) to disk...
Flushing run 64 (5000039 postings) to disk...
Flushing run 65 (5000035 postings) to disk...
Flushing run 66 (5000011 postings) to disk...
Flushing run 67 (3284469 postings) to disk...
Indexing complete! Indexed 8841823 docs (481588616 tokens). Avg doc len: 54.47
The total time taken is 146.12s
```

## `bench_out/fan32_indexer.out`

```text
Processed 10/32 run files
Processed 20/32 run files
Processed 30/32 run files
Processed 32/32 run files
Processed 10/32 run files
Processed 20/32 run files
Processed 30/32 run files
Processed 32/32 run files
Processed 4/4 run files
Processed 3/3 run files
index stats:
  postings processed: 338285683
  malformed postings: 0
  lexicon.bin: 49054892 bytes
  postings.bin: 589818024 bytes
Index compilation complete in 116.54s
```

## `bench_out/fan16_indexer_cold.out`

```text
Processed 10/16 run files
Processed 16/16 run files
Processed 10/16 run files
Processed 16/16 run files
Processed 10/16 run files
Processed 16/16 run files
Processed 10/16 run files
Processed 16/16 run files
Processed 4/4 run files
Processed 5/5 run files
index stats:
  postings processed: 338285683
  malformed postings: 0
  lexicon.bin: 49054892 bytes
  postings.bin: 589818024 bytes
Index compilation complete in 114.43s
```

## `bench_out/fan16_indexer_cold.time.txt`

```text
      114.44 real       107.23 user         4.34 sys
             4194304  maximum resident set size
                   0  average shared memory size
                   0  average unshared data size
                 409  page reclaims
                  76  page faults
                   0  swaps
                   0  block input operations
                   0  block output operations
                   0  messages sent
                   0  messages received
                   0  signals received
                3593  voluntary context switches
               53833  involuntary context switches
       2206088111412  instructions retired
        416722662877  cycles elapsed
             4064064  peak memory footprint
```

## `bench_out/fan64_indexer_cold.out`

```text
Processed 10/64 run files
Processed 20/64 run files
Processed 30/64 run files
Processed 40/64 run files
Processed 50/64 run files
Processed 60/64 run files
Processed 64/64 run files
Processed 4/4 run files
Processed 2/2 run files
index stats:
  postings processed: 338285683
  malformed postings: 0
  lexicon.bin: 49054892 bytes
  postings.bin: 589818024 bytes
Index compilation complete in 115.76s
```

## `bench_out/fan64_indexer_cold.time.txt`

```text
      115.77 real       105.80 user         4.38 sys
             4259840  maximum resident set size
                   0  average shared memory size
                   0  average unshared data size
                 398  page reclaims
                  76  page faults
                   0  swaps
                   0  block input operations
                   0  block output operations
                   0  messages sent
                   0  signals received
               30588  voluntary context switches
               42074  involuntary context switches
       2196036233546  instructions retired
        411709158361  cycles elapsed
             4293376  peak memory footprint
```

## `bench_out/fan128_indexer_cold.out`

```text
Processed 10/68 run files
Processed 20/68 run files
Processed 30/68 run files
Processed 40/68 run files
Processed 50/68 run files
Processed 60/68 run files
Processed 68/68 run files
index stats:
  postings processed: 338285683
  malformed postings: 0
  lexicon.bin: 49054892 bytes
  postings.bin: 589818024 bytes
Index compilation complete in 56.16s
```

## `bench_out/fan128_indexer_cold.time.txt`

```text
       56.17 real        54.14 user         1.36 sys
             5079040  maximum resident set size
                   0  average shared memory size
                   0  average unshared data size
                 367  page reclaims
                  76  page faults
                   0  swaps
                   0  block input operations
                   0  block output operations
                   0  messages sent
                   0  messages received
                   0  signals received
                 741  voluntary context switches
               22872  involuntary context switches
       1097812864234  instructions retired
        207471699462  cycles elapsed
             4408128  peak memory footprint
```

