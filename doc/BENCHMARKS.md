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
| ⚠        | high-variance cell (CV ≥ 3%) — re-run via callgrind  |

- Wall-clock runs hide the `CV` column to keep tables narrow.
  A `⚠` next to a value means the coefficient of variation
  was ≥ 3% — interpret that cell loosely and cross-check with
  `mise x bench:callgrind` for a deterministic `Ir/MiB` value.
- `Ir/MiB` is host-independent: retired instructions per MiB
  of input. Not directly comparable to wall-clock throughput
  (IPC varies by workload), but excellent for capacity
  planning context — CPU, RAM, and cache costs.
  Only present when the report was generated from an
  iai-callgrind run.

## Highlights for Humans

- 837 MiB/s dirty throughput (4 KiB, ~20% ANSI)
- 5.3 GiB/s clean fast path (24 MiB)
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
| L3         | 12.0 MiB                                 |
| RAM        | 31.2 GiB                                 |
| target-cpu | `x86-64-v3`                              |
| Sizes      | 22 tiers (hardware-adaptive)             |
| Bench time | 9m26s                                    |

### Crate Versions

| Crate                | Version |
| -------------------- | ------: |
| `distill-strip-ansi` |   0.7.0 |
| `fast-strip-ansi`    |  0.13.1 |
| `console`            |  0.16.3 |
| `strip-ansi-escapes` |   0.2.1 |
| `criterion`          |   0.7.0 |

## Crate Footprints

| Crate                | Deps |  Peak RSS |   RSS Δ |    CPU |
| -------------------- | ---: | --------: | ------: | -----: |
| `distill-strip-ansi` |    2 | 163.7 MiB | 1.5 MiB | 31.0 s |
| `fast-strip-ansi`    |    3 | 206.2 MiB | 2.6 MiB | 25.4 s |
| `console`            |    2 | 194.9 MiB | 9.9 MiB | 27.2 s |
| `strip-ansi-escapes` |    2 | 193.5 MiB |  352.0K | 79.6 s |

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

| Crate                |      Time | MiB/s |     × | Ir/MiB |
| -------------------- | --------: | ----: | ----: | -----: |
| `distill-strip-ansi` |  2.7 µs ⚠ |   721 |  base |  32.4M |
| `fast-strip-ansi`    |  3.1 µs ⚠ |   636 |  1.1× |  40.4M |
| `console`            |  7.7 µs ⚠ |   254 |  2.8× | 125.4M |
| `strip-ansi-escapes` | 29.1 µs ⚠ |    67 | 10.8× | 371.4M |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

### Dirty 4 KiB

| Crate                |      Time | MiB/s |     × | Ir/MiB |
| -------------------- | --------: | ----: | ----: | -----: |
| `distill-strip-ansi` |  4.7 µs ⚠ |   837 |  base |  16.2M |
| `fast-strip-ansi`    |  6.2 µs ⚠ |   633 |  1.3× |  20.2M |
| `console`            | 15.0 µs ⚠ |   261 |  3.2× |  62.7M |
| `strip-ansi-escapes` | 59.2 µs ⚠ |    66 | 12.7× | 185.7M |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

### Dirty 32 KiB

| Crate                |       Time | MiB/s |     × | Ir/MiB |
| -------------------- | ---------: | ----: | ----: | -----: |
| `distill-strip-ansi` |  34.8 µs ⚠ |   897 |  base |  30.8M |
| `fast-strip-ansi`    |  50.2 µs ⚠ |   622 |  1.4× |  40.8M |
| `console`            | 124.6 µs ⚠ |   251 |  3.6× | 111.4M |
| `strip-ansi-escapes` | 591.4 µs ⚠ |    53 | 17.0× | 357.7M |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

### Dirty 256 KiB

| Crate                |       Time | MiB/s |     × | Ir/MiB |
| -------------------- | ---------: | ----: | ----: | -----: |
| `distill-strip-ansi` | 333.7 µs ⚠ |   749 |  base |   3.9M |
| `fast-strip-ansi`    | 469.2 µs ⚠ |   533 |  1.4× |   5.1M |
| `console`            | 986.0 µs ⚠ |   254 |  3.0× |  13.9M |
| `strip-ansi-escapes` |   3.9 ms ⚠ |    64 | 11.7× |  44.7M |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

### Dirty 24 MiB

| Crate                |       Time | MiB/s |     × | Ir/MiB |
| -------------------- | ---------: | ----: | ----: | -----: |
| `distill-strip-ansi` |  28.9 ms ⚠ |   832 |  base |  10.1M |
| `fast-strip-ansi`    |  52.1 ms ⚠ |   461 |  1.8× |  12.8M |
| `console`            | 125.8 ms ⚠ |   191 |  4.4× |  36.1M |
| `strip-ansi-escapes` | 483.1 ms ⚠ |    50 | 16.7× | 118.2M |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

### Dirty 32 MiB

| Crate                |       Time | MiB/s |     × | Ir/MiB |
| -------------------- | ---------: | ----: | ----: | -----: |
| `distill-strip-ansi` |  48.2 ms ⚠ |   663 |  base |   7.5M |
| `fast-strip-ansi`    |  74.5 ms ⚠ |   429 |  1.5× |   9.6M |
| `console`            | 173.9 ms ⚠ |   184 |  3.6× |  27.1M |
| `strip-ansi-escapes` | 672.4 ms ⚠ |    48 | 13.9× |  88.7M |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

### Dirty 48 MiB

| Crate                |       Time | MiB/s |     × | Ir/MiB |
| -------------------- | ---------: | ----: | ----: | -----: |
| `distill-strip-ansi` |  67.2 ms ⚠ |   714 |  base |   5.0M |
| `fast-strip-ansi`    | 107.2 ms ⚠ |   448 |  1.6× |   6.4M |
| `console`            | 233.0 ms ⚠ |   206 |  3.5× |  18.1M |
| `strip-ansi-escapes` | 952.1 ms ⚠ |    50 | 14.2× |  59.1M |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

### Dirty 1 GiB

| Crate                |         Time | MiB/s |     × | Ir/MiB |
| -------------------- | -----------: | ----: | ----: | -----: |
| `distill-strip-ansi` |  1685.5 ms ⚠ |   608 |  base | 235.7K |
| `fast-strip-ansi`    |  2556.3 ms ⚠ |   401 |  1.5× | 299.2K |
| `console`            |  5005.4 ms ⚠ |   205 |  3.0× | 846.4K |
| `strip-ansi-escapes` | 18020.3 ms ⚠ |    57 | 10.7× |   2.8M |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

### Cargo Output (5 KiB)

| Crate                |       Time | MiB/s |      × | Ir/MiB |
| -------------------- | ---------: | ----: | -----: | -----: |
| `distill-strip-ansi` | 130.0 ns ⚠ | 41076 |   base |  12.4M |
| `fast-strip-ansi`    |   3.7 µs ⚠ |  1433 |  28.7× |  16.3M |
| `console`            |  10.2 µs ⚠ |   523 |  78.6× |  48.5M |
| `strip-ansi-escapes` | 114.8 µs ⚠ |    47 | 883.3× | 147.7M |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

### OSC 8 Hyperlinks (4 KiB)

| Crate                |       Time | MiB/s |      × | Ir/MiB |
| -------------------- | ---------: | ----: | -----: | -----: |
| `distill-strip-ansi` | 106.7 ns ⚠ | 38498 |   base |  11.2M |
| `fast-strip-ansi`    |   2.9 µs ⚠ |  1413 |  27.2× |  33.7M |
| `console`            |   8.4 µs ⚠ |   488 |  79.0× |  25.3M |
| `strip-ansi-escapes` |  83.4 µs ⚠ |    49 | 781.5× |  67.5M |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

### Extended Capabilities

Additional features available in `distill-strip-ansi`.

| Feature                   |       Time | MiB/s | Ir/MiB |
| ------------------------- | ---------: | ----: | -----: |
| Classify (parse only)     |  12.9 µs ⚠ |   325 |  33.5M |
| Classify + detail         |  11.1 µs ⚠ |   376 |  32.6M |
| Filter: SGR mask          |  13.3 µs ⚠ |   316 |  39.5M |
| Filter: sanitize preset   |  11.8 µs ⚠ |   356 |  41.3M |
| Threat scan (clean)       |  12.8 µs ⚠ |   328 |  32.2M |
| Threat scan (dirty)       |  13.5 µs ⚠ |   312 |  32.3M |
| Streaming (L1)            |  39.6 µs ⚠ |   789 |  33.5M |
| Streaming (L2)            | 389.5 µs ⚠ |   642 |   4.2M |
| Streaming (L3)            |  17.8 ms ⚠ |   675 |  21.9M |
| Unicode normalize         |  21.2 µs ⚠ |   153 |  94.1M |
| Transform: passthrough    |  86.7 ns ⚠ | 48406 |   1.9M |
| Transform: truecolor→mono |  23.5 µs ⚠ |   198 |  64.7M |
| Transform: truecolor→grey |  27.8 µs ⚠ |   167 |  68.8M |
| Transform: truecolor→16   |  25.7 µs ⚠ |   181 |  67.5M |
| Transform: truecolor→256  |  23.1 µs ⚠ |   202 |  68.8M |
| Transform: 256→16         |  21.4 µs ⚠ |   182 |  60.7M |
| Transform: 256→grey       |  20.9 µs ⚠ |   186 |  69.8M |
| Transform: basic→mono     |  27.6 µs ⚠ |   152 |  64.4M |
| Augment: protanopia       |   3.2 µs ⚠ |   231 |  52.0M |
| Augment: deuteranopia     |   3.3 µs ⚠ |   223 |  52.0M |
| Augment: sRGB roundtrip   | 840.8 ns ⚠ |   290 |  29.3M |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

## Scaling

Dirty throughput (MiB/s) across input sizes.
Constant bar length = O(n). Shrinking = super-linear.

RSS Δ and CPU shown at largest size only — small-size
values are dominated by benchmark harness overhead.

### `distill-strip-ansi` v0.7.0 — O(n)

```text
  2 KiB ████████████████████████ 721
  4 KiB ████████████████████████████ 837
  8 KiB ██████████████████████████ 799
 16 KiB ████████████████████████████ 854
 32 KiB ██████████████████████████████ 897
 64 KiB ███████████████████████████ 818
128 KiB ████████████████████████████ 862
256 KiB █████████████████████████ 749
512 KiB ██████████████████████████████ 898
  1 MiB ███████████████████████████ 813
  2 MiB ████████████████████████████ 855
  4 MiB ██████████████████████████ 789
  8 MiB ███████████████████████████ 811
 24 MiB ███████████████████████████ 832
 32 MiB ██████████████████████ 663
 48 MiB ███████████████████████ 714
 64 MiB ███████████████████████ 703
 96 MiB ██████████████████████ 676
192 MiB ██████████████████ 545
384 MiB ████████████████████ 628
768 MiB ████████████████████ 615
  1 GiB ████████████████████ 608
```

### `fast-strip-ansi` v0.13.1 — O(n)

```text
  2 KiB █████████████████████ 636
  4 KiB █████████████████████ 633
  8 KiB ███████████████████ 587
 16 KiB ███████████████████ 574
 32 KiB ████████████████████ 622
 64 KiB ███████████████████ 569
128 KiB ██████████████████ 546
256 KiB █████████████████ 533
512 KiB ██████████████████ 560
  1 MiB █████████████████ 526
  2 MiB ███████████████████ 570
  4 MiB ██████████████████ 562
  8 MiB █████████████████████ 630
 24 MiB ███████████████ 461
 32 MiB ██████████████ 429
 48 MiB ██████████████ 448
 64 MiB ███████████████ 466
 96 MiB ███████████████ 451
192 MiB ██████████████ 419
384 MiB █████████████ 414
768 MiB █████████████ 415
  1 GiB █████████████ 401
```

### `console` v0.16.3 — O(n)

```text
  2 KiB ████████ 254
  4 KiB ████████ 261
  8 KiB ████████ 259
 16 KiB ████████ 265
 32 KiB ████████ 251
 64 KiB ████████ 259
128 KiB ████████ 249
256 KiB ████████ 254
512 KiB ████████ 261
  1 MiB █████████ 271
  2 MiB ████████ 257
  4 MiB ███████ 236
  8 MiB ███████ 224
 24 MiB ██████ 191
 32 MiB ██████ 184
 48 MiB ██████ 206
 64 MiB ██████ 199
 96 MiB ███████ 215
192 MiB ██████ 199
384 MiB ██████ 191
768 MiB ██████ 199
  1 GiB ██████ 205
```

### `strip-ansi-escapes` v0.2.1 — O(n)

```text
  2 KiB ██ 67
  4 KiB ██ 66
  8 KiB ██ 64
 16 KiB ██ 65
 32 KiB █ 53
 64 KiB ██ 61
128 KiB █ 57
256 KiB ██ 64
512 KiB ██ 63
  1 MiB █ 55
  2 MiB ██ 62
  4 MiB ██ 60
  8 MiB █ 60
 24 MiB █ 50
 32 MiB █ 48
 48 MiB █ 50
 64 MiB █ 53
 96 MiB █ 56
192 MiB █ 57
384 MiB █ 57
768 MiB █ 57
  1 GiB █ 57
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
