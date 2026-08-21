# Benchmarks

Criterion.rs statistical benchmarks across the Rust ANSI
stripping ecosystem: `distill-strip-ansi`, `fast-strip-ansi`,
`strip-ansi-escapes`, and `console`.

For reproduction instructions, CPU pinning, PGO, and the mise
task wiring, see `doc/BENCHMARKS-REPRODUCE.md`.

## Reading the Numbers

| Symbol   | Meaning                                              |
| -------- | ---------------------------------------------------- |
| ns       | nanoseconds (10⁻⁹ s)                                 |
| µs       | microseconds (10⁻⁶ s)                                |
| ms       | milliseconds (10⁻³ s)                                |
| MiB/s    | mebibytes/sec (2²⁰ B/s)                              |
| GiB/s    | gibibytes/sec (2³⁰ B/s)                              |
| ×        | multiplier (`base` = distill)                        |
| Ir/MiB   | retired instructions per MiB (callgrind)             |
| ⚠        | dispersed cell (MAD/median ≥ 1%) — see Ir/MiB        |

- Reported times are the **median** of criterion's samples,
  not the mean or the regression slope criterion prints to its
  own stdout. Median is deliberate: these runs carry a heavy
  right tail (10-20% of samples land in it on authors host), and
  the median rejects that tail instead of being dragged by it.
  Expect the numbers here to read faster than criterion's
  console output for the same benchmark.
- Dispersion is reported as MAD/median — the median absolute
  deviation relative to the median — so the spread figure and
  the point estimate are the same kind of statistic. A `⚠`
  means that ratio reached 1%, i.e. the *bulk* of the samples
  is genuinely spread, not merely that outliers exist.
- Both are strictly *within* one run. Run-to-run variation is
  much larger — up to ~20% on identical code on authors host —
  so treat cross-run comparisons of wall-clock numbers with
  suspicion and use `Ir/MiB` for those instead:
  `mise run bench:callgrind`.
- `Ir/MiB` is host-independent: retired instructions per MiB
  of input. Not directly comparable to wall-clock throughput
  (IPC varies by workload), but excellent for capacity
  planning context — CPU, RAM, and cache costs.
  Only present when the report was generated from an
  iai-callgrind run.

## Highlights for Humans

- 928 MiB/s dirty throughput (4 KiB, ~20% ANSI)
- 4.1 GiB/s clean fast path (16 MiB)
- Zero allocation on clean input (`Cow::Borrowed`)
- No temp files, no disk I/O — pure in-memory
- O(n) linear scaling — constant-ish throughput up to 1 GiB+

## Environment

| Key        | Value                                    |
| ---------- | ---------------------------------------- |
| CPU        | Intel(R) Core(TM) i7-4790K CPU @ 4.00GHz |
| Arch       | x86_64                                   |
| OS         | Linux 7.2.4-3-cachyos                    |
| Rust       | 1.98.1                                   |
| Date       | 2026-09-12                               |
| L1d        | 32.0K                                    |
| L2         | 256.0K                                   |
| L3         | 8.0 MiB                                  |
| RAM        | 31.2 GiB                                 |
| target-cpu | `x86-64-v3`                              |
| Sizes      | 16 tiers (hardware-adaptive)             |
| Bench time | 7m27s                                    |

### Crate Versions

| Crate                | Version |
| -------------------- | ------: |
| `distill-strip-ansi` |   0.7.0 |
| `fast-strip-ansi`    |  0.13.1 |
| `console`            |  0.16.6 |
| `strip-ansi-escapes` |   0.2.1 |
| `criterion`          |   0.7.0 |

## Crate Footprints

| Crate                | Deps |  Peak RSS |   RSS Δ |    CPU |
| -------------------- | ---: | --------: | ------: | -----: |
| `distill-strip-ansi` |    2 | 178.5 MiB |  656.0K | 17.0 s |
| `fast-strip-ansi`    |    3 | 227.0 MiB |  508.0K | 15.4 s |
| `console`            |    2 | 194.8 MiB | 1.3 MiB | 18.9 s |
| `strip-ansi-escapes` |    2 | 205.2 MiB | 8.1 MiB | 44.6 s |

`strip-ansi` binary: 4.6 MiB, 24 deps
(includes `clap` for CLI argument parsing).

No crate uses temp files or disk I/O — stdin only.
Peak RSS, RSS Δ, and CPU measured at largest bench size.
RSS Δ reflects allocator page retention after the last
Criterion iteration — not a leak. CPU is user+sys time
for the benchmark (not wall clock). Resource snapshots
captured via `task_info` (macOS) / `getrusage` (POSIX)
outside the timed loop — no measurement overhead.

## Details That Matter

All crates: `&[u8]` input. `console`: `&str`
(conversion outside timed loop). `distill-strip-ansi`
used as baseline (Relative = time / baseline time).

The `Ir/MiB` column, when present, reports deterministic
instruction counts measured under Callgrind — independent
of CPU frequency, thermal state, or scheduler noise. See
the reproduction doc for how to generate it.

### Dirty 2 KiB

| Crate                |     Time | MiB/s |     × | Ir/MiB |
| -------------------- | -------: | ----: | ----: | -----: |
| `distill-strip-ansi` | 2.2 µs ⚠ |   908 |  base |  31.9M |
| `fast-strip-ansi`    |   2.8 µs |   687 |  1.3× |  41.1M |
| `console`            | 8.0 µs ⚠ |   246 |  3.7× | 127.2M |
| `strip-ansi-escapes` |  27.0 µs |    72 | 12.6× | 370.6M |

⚠ marks cells where MAD/median ≥ 1% — re-run `mise run bench:callgrind`
for a deterministic `Ir/MiB` check.

### Dirty 4 KiB

| Crate                |      Time | MiB/s |     × | Ir/MiB |
| -------------------- | --------: | ----: | ----: | -----: |
| `distill-strip-ansi` |    4.2 µs |   928 |  base |  16.0M |
| `fast-strip-ansi`    |  5.8 µs ⚠ |   679 |  1.4× |  20.5M |
| `console`            | 15.3 µs ⚠ |   255 |  3.6× |  63.6M |
| `strip-ansi-escapes` |   54.0 µs |    72 | 12.8× | 185.3M |

⚠ marks cells where MAD/median ≥ 1% — re-run `mise run bench:callgrind`
for a deterministic `Ir/MiB` check.

### Dirty 32 KiB

| Crate                |       Time | MiB/s |     × | Ir/MiB |
| -------------------- | ---------: | ----: | ----: | -----: |
| `distill-strip-ansi` |  34.0 µs ⚠ |   919 |  base |  30.4M |
| `fast-strip-ansi`    |    45.2 µs |   691 |  1.3× |  41.1M |
| `console`            | 148.9 µs ⚠ |   210 |  4.4× | 113.6M |
| `strip-ansi-escapes` |   430.6 µs |    73 | 12.7× | 357.4M |

⚠ marks cells where MAD/median ≥ 1% — re-run `mise run bench:callgrind`
for a deterministic `Ir/MiB` check.

### Dirty 256 KiB

| Crate                |       Time | MiB/s |     × | Ir/MiB |
| -------------------- | ---------: | ----: | ----: | -----: |
| `distill-strip-ansi` | 408.7 µs ⚠ |   612 |  base |   3.8M |
| `fast-strip-ansi`    | 414.3 µs ⚠ |   603 | ~1.0× |   5.1M |
| `console`            |   1.1 ms ⚠ |   230 |  2.7× |  14.2M |
| `strip-ansi-escapes` |   5.6 ms ⚠ |    45 | 13.7× |  44.7M |

⚠ marks cells where MAD/median ≥ 1% — re-run `mise run bench:callgrind`
for a deterministic `Ir/MiB` check.

### Dirty 8 MiB

| Crate                |       Time | MiB/s |     × | Ir/MiB |
| -------------------- | ---------: | ----: | ----: | -----: |
| `distill-strip-ansi` |  12.6 ms ⚠ |   637 |  base |  29.8M |
| `fast-strip-ansi`    |  22.5 ms ⚠ |   356 |  1.8× |  39.0M |
| `console`            |  37.0 ms ⚠ |   216 |  2.9× | 110.6M |
| `strip-ansi-escapes` | 244.9 ms ⚠ |    33 | 19.5× | 354.4M |

⚠ marks cells where MAD/median ≥ 1% — re-run `mise run bench:callgrind`
for a deterministic `Ir/MiB` check.

### Dirty 32 MiB

| Crate                |       Time | MiB/s |    × | Ir/MiB |
| -------------------- | ---------: | ----: | ---: | -----: |
| `distill-strip-ansi` |  61.6 ms ⚠ |   520 | base |   7.4M |
| `fast-strip-ansi`    |  85.3 ms ⚠ |   375 | 1.4× |   9.8M |
| `console`            | 180.7 ms ⚠ |   177 | 2.9× |  27.6M |
| `strip-ansi-escapes` |   443.2 ms |    72 | 7.2× |  88.6M |

⚠ marks cells where MAD/median ≥ 1% — re-run `mise run bench:callgrind`
for a deterministic `Ir/MiB` check.

### Dirty 64 MiB

| Crate                |        Time | MiB/s |     × | Ir/MiB |
| -------------------- | ----------: | ----: | ----: | -----: |
| `distill-strip-ansi` |  146.2 ms ⚠ |   438 |  base |   3.7M |
| `fast-strip-ansi`    |  147.4 ms ⚠ |   434 | ~1.0× |   4.9M |
| `console`            |  309.7 ms ⚠ |   207 |  2.1× |  13.8M |
| `strip-ansi-escapes` | 1412.4 ms ⚠ |    45 |  9.7× |  44.3M |

⚠ marks cells where MAD/median ≥ 1% — re-run `mise run bench:callgrind`
for a deterministic `Ir/MiB` check.

### Cargo Output (5 KiB)

| Crate                |      Time | MiB/s |      × | Ir/MiB |
| -------------------- | --------: | ----: | -----: | -----: |
| `distill-strip-ansi` |  117.7 ns | 45386 |   base |  12.3M |
| `fast-strip-ansi`    |  3.7 µs ⚠ |  1456 |  31.2× |  16.5M |
| `console`            |   11.2 µs |   478 |  94.9× |  49.1M |
| `strip-ansi-escapes` | 94.0 µs ⚠ |    57 | 798.6× | 147.5M |

⚠ marks cells where MAD/median ≥ 1% — re-run `mise run bench:callgrind`
for a deterministic `Ir/MiB` check.

### OSC 8 Hyperlinks (4 KiB)

| Crate                |      Time | MiB/s |      × | Ir/MiB |
| -------------------- | --------: | ----: | -----: | -----: |
| `distill-strip-ansi` |   91.7 ns | 44784 |   base |  11.1M |
| `fast-strip-ansi`    |    2.7 µs |  1529 |  29.3× |  33.8M |
| `console`            |    8.8 µs |   467 |  95.9× |  25.4M |
| `strip-ansi-escapes` | 89.4 µs ⚠ |    46 | 974.1× |  67.0M |

⚠ marks cells where MAD/median ≥ 1% — re-run `mise run bench:callgrind`
for a deterministic `Ir/MiB` check.

### Extended Capabilities

Additional features available in `distill-strip-ansi`.

| Feature                   |       Time | MiB/s | Ir/MiB |
| ------------------------- | ---------: | ----: | -----: |
| Classify (parse only)     |  12.6 µs ⚠ |   334 |  33.7M |
| Classify + detail         |    10.5 µs |   400 |  32.8M |
| Filter: SGR mask          |    10.2 µs |   413 |  38.8M |
| Filter: sanitize preset   |    10.6 µs |   394 |  40.6M |
| Threat scan (clean)       |  10.3 µs ⚠ |   409 |  32.4M |
| Threat scan (dirty)       |  10.3 µs ⚠ |   410 |  32.4M |
| Streaming (L1)            |  42.2 µs ⚠ |   740 |  33.0M |
| Streaming (L2)            | 280.4 µs ⚠ |   891 |   4.1M |
| Streaming (L3)            |  13.8 ms ⚠ |   582 |  32.3M |
| Unicode normalize         |    16.4 µs |   197 |  93.9M |
| Transform: passthrough    |    76.9 ns | 54592 |   1.8M |
| Transform: truecolor→mono |    19.4 µs |   240 |  61.4M |
| Transform: truecolor→grey |  30.1 µs ⚠ |   155 |  65.6M |
| Transform: truecolor→16   |  20.9 µs ⚠ |   223 |  64.1M |
| Transform: truecolor→256  |    21.3 µs |   218 |  65.6M |
| Transform: 256→16         |  15.1 µs ⚠ |   257 |  58.0M |
| Transform: 256→grey       |    16.9 µs |   230 |  67.1M |
| Transform: basic→mono     |  22.6 µs ⚠ |   186 |  62.0M |
| Augment: protanopia       |     2.8 µs |   259 |  51.9M |
| Augment: deuteranopia     |     2.8 µs |   259 |  51.9M |
| Augment: sRGB roundtrip   |   724.0 ns |   337 |  29.3M |

⚠ marks cells where MAD/median ≥ 1% — re-run `mise run bench:callgrind`
for a deterministic `Ir/MiB` check.

## Scaling

Dirty throughput (MiB/s) across input sizes.
Constant bar length = O(n). Shrinking = super-linear.

RSS Δ and CPU shown at largest size only — small-size
values are dominated by benchmark harness overhead.

### `distill-strip-ansi` v0.7.0 — O(n) · RSS Δ 656.0K · CPU 17.0 s

```text
  2 KiB █████████████████████████████ 908
  4 KiB █████████████████████████████ 928
  8 KiB ██████████████████████████████ 929
 16 KiB █████████████████████████████ 925
 32 KiB █████████████████████████████ 919
 64 KiB ████████████████████████████ 888
128 KiB █████████████████████████████ 918
256 KiB ███████████████████ 612
512 KiB █████████████████████ 668
  1 MiB █████████████████████████████ 906
  2 MiB █████████████████████████████ 917
  4 MiB ████████████████████████ 773
  8 MiB ████████████████████ 637
 16 MiB ██████████████████ 561
 32 MiB ████████████████ 520
 64 MiB ██████████████ 438
```

### `fast-strip-ansi` v0.13.1 — O(n) · RSS Δ 508.0K · CPU 15.4 s

```text
  2 KiB ██████████████████████ 687
  4 KiB █████████████████████ 679
  8 KiB ███████████████████ 592
 16 KiB ██████████████████████ 695
 32 KiB ██████████████████████ 691
 64 KiB ██████████████████████ 683
128 KiB ██████████████████████ 685
256 KiB ███████████████████ 603
512 KiB █████████████ 422
  1 MiB ██████████████████████ 686
  2 MiB ████████████████ 507
  4 MiB ███████████ 360
  8 MiB ███████████ 356
 16 MiB ████████████ 378
 32 MiB ████████████ 375
 64 MiB ██████████████ 434
```

### `console` v0.16.6 — O(n) · RSS Δ 1.3 MiB · CPU 18.9 s

```text
  2 KiB ███████ 246
  4 KiB ████████ 255
  8 KiB ████████ 255
 16 KiB ████████ 256
 32 KiB ██████ 210
 64 KiB ████████ 258
128 KiB ████████ 256
256 KiB ███████ 230
512 KiB ████████ 256
  1 MiB ████████ 259
  2 MiB ███████ 221
  4 MiB █████ 173
  8 MiB ██████ 216
 16 MiB ███████ 231
 32 MiB █████ 177
 64 MiB ██████ 207
```

### `strip-ansi-escapes` v0.2.1 — O(n) · RSS Δ 8.1 MiB · CPU 44.6 s

```text
  2 KiB ██ 72
  4 KiB ██ 72
  8 KiB ██ 73
 16 KiB ██ 73
 32 KiB ██ 73
 64 KiB ██ 73
128 KiB ██ 70
256 KiB █ 45
512 KiB █ 47
  1 MiB █ 53
  2 MiB █ 46
  4 MiB █ 53
  8 MiB █ 33
 16 MiB █ 43
 32 MiB ██ 72
 64 MiB █ 45
```

### Complexity Summary

| Crate                | Dirty | Clean |
| -------------------- | ----- | ----- |
| `distill-strip-ansi` | O(n)  | O(n)  |
| `fast-strip-ansi`    | O(n)  | O(n)  |
| `console`            | O(n)  | O(n)  |
| `strip-ansi-escapes` | O(n)  | O(n)  |

Complexity estimated per memory tier (L1/L2/L3/DRAM) —
throughput steps between tiers are hardware, not algorithmic.
