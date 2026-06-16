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

- 402 MiB/s dirty throughput (4 KiB, ~20% ANSI)
- 3.2 GiB/s clean fast path (24 MiB)
- Zero allocation on clean input (`Cow::Borrowed`)
- No temp files, no disk I/O — pure in-memory
- O(n) linear scaling — constant-ish throughput up to 1 GiB+

## Environment

| Key        | Value                                    |
| ---------- | ---------------------------------------- |
| CPU        | Intel(R) Core(TM) i7-9750H CPU @ 2.60GHz |
| Arch       | x86_64                                   |
| OS         | macOS 26.5.1                             |
| Rust       | 1.96.0                                   |
| Date       | 2026-06-16                               |
| L1d        | 32.0K                                    |
| L2         | 256.0K                                   |
| L3         | 12.0 MiB                                 |
| RAM        | 32.0 GiB                                 |
| target-cpu | `x86-64-v3`                              |
| Sizes      | 15 tiers (hardware-adaptive)             |
| Bench time | 9m35s                                    |

### Crate Versions

| Crate                | Version |
| -------------------- | ------: |
| `distill-strip-ansi` |   0.6.1 |
| `fast-strip-ansi`    |  0.13.1 |
| `console`            |  0.16.3 |
| `strip-ansi-escapes` |   0.2.1 |
| `vtparse`            |   0.7.0 |
| `criterion`          |   0.7.0 |

## Crate Footprints

| Crate                | Deps |  Peak RSS |    RSS Δ |     CPU |
| -------------------- | ---: | --------: | -------: | ------: |
| `distill-strip-ansi` |    2 | 203.9 MiB | 19.4 MiB |  27.4 s |
| `fast-strip-ansi`    |    3 | 262.2 MiB | 19.5 MiB |  30.2 s |
| `console`            |    2 | 212.4 MiB |  1.5 MiB |  57.1 s |
| `strip-ansi-escapes` |    2 | 238.1 MiB | 13.7 MiB | 123.7 s |
| `vtparse`            |    1 | 194.3 MiB | 19.6 MiB |  80.3 s |

`strip-ansi` binary: 944.3K, 24 deps
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

| Crate                |    Time | MiB/s |    × |
| -------------------- | ------: | ----: | ---: |
| `distill-strip-ansi` |  5.2 µs |   376 | base |
| `fast-strip-ansi`    |  6.1 µs |   318 | 1.2× |
| `console`            | 15.2 µs |   129 | 2.9× |
| `strip-ansi-escapes` | 46.1 µs |    42 | 8.9× |
| `vtparse`            | 24.3 µs |    80 | 4.7× |

### Dirty 4 KiB

| Crate                |      Time | MiB/s |    × |
| -------------------- | --------: | ----: | ---: |
| `distill-strip-ansi` |    9.7 µs |   402 | base |
| `fast-strip-ansi`    | 11.5 µs ⚠ |   340 | 1.2× |
| `console`            | 28.7 µs ⚠ |   136 | 3.0× |
| `strip-ansi-escapes` | 86.0 µs ⚠ |    45 | 8.9× |
| `vtparse`            |   45.8 µs |    85 | 4.7× |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

### Dirty 32 KiB

| Crate                |       Time | MiB/s |    × |
| -------------------- | ---------: | ----: | ---: |
| `distill-strip-ansi` |    66.5 µs |   470 | base |
| `fast-strip-ansi`    |  83.7 µs ⚠ |   373 | 1.3× |
| `console`            | 206.4 µs ⚠ |   151 | 3.1× |
| `strip-ansi-escapes` |   609.2 µs |    51 | 9.2× |
| `vtparse`            |   374.7 µs |    83 | 5.6× |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

### Dirty 256 KiB

| Crate                |       Time | MiB/s |    × |
| -------------------- | ---------: | ----: | ---: |
| `distill-strip-ansi` |   536.3 µs |   466 | base |
| `fast-strip-ansi`    | 969.1 µs ⚠ |   258 | 1.8× |
| `console`            |   1.6 ms ⚠ |   158 | 3.0× |
| `strip-ansi-escapes` |     4.9 ms |    51 | 9.2× |
| `vtparse`            |     2.8 ms |    88 | 5.3× |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

### Dirty 24 MiB

| Crate                |       Time | MiB/s |    × |
| -------------------- | ---------: | ----: | ---: |
| `distill-strip-ansi` |    53.7 ms |   447 | base |
| `fast-strip-ansi`    |  63.6 ms ⚠ |   378 | 1.2× |
| `console`            | 152.3 ms ⚠ |   158 | 2.8× |
| `strip-ansi-escapes` | 445.8 ms ⚠ |    54 | 8.3× |
| `vtparse`            | 252.8 ms ⚠ |    95 | 4.7× |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

### Dirty 32 MiB

| Crate                |       Time | MiB/s |    × |
| -------------------- | ---------: | ----: | ---: |
| `distill-strip-ansi` |    71.9 ms |   445 | base |
| `fast-strip-ansi`    |  85.6 ms ⚠ |   374 | 1.2× |
| `console`            | 217.0 ms ⚠ |   147 | 3.0× |
| `strip-ansi-escapes` | 562.8 ms ⚠ |    57 | 7.8× |
| `vtparse`            | 340.5 ms ⚠ |    94 | 4.7× |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

### Cargo Output (5 KiB)

| Crate                |     Time | MiB/s |      × |
| -------------------- | -------: | ----: | -----: |
| `distill-strip-ansi` | 214.5 ns | 24902 |   base |
| `fast-strip-ansi`    | 5.1 µs ⚠ |  1051 |  23.7× |
| `console`            |  17.3 µs |   309 |  80.6× |
| `strip-ansi-escapes` | 122.2 µs |    44 | 569.9× |
| `vtparse`            |  58.8 µs |    91 | 274.1× |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

### OSC 8 Hyperlinks (4 KiB)

| Crate                |       Time | MiB/s |      × |
| -------------------- | ---------: | ----: | -----: |
| `distill-strip-ansi` | 185.0 ns ⚠ | 22204 |   base |
| `fast-strip-ansi`    |   4.2 µs ⚠ |   987 |  22.5× |
| `console`            |    13.9 µs |   296 |  74.9× |
| `strip-ansi-escapes` |    99.2 µs |    41 | 535.9× |
| `vtparse`            |  50.8 µs ⚠ |    81 | 274.7× |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

### Extended Capabilities

Additional features available in `distill-strip-ansi`.

| Feature                   |       Time | MiB/s |
| ------------------------- | ---------: | ----: |
| Classify (parse only)     |  12.5 µs ⚠ |   335 |
| Classify + detail         |  13.1 µs ⚠ |   320 |
| Filter: SGR mask          |  14.7 µs ⚠ |   285 |
| Filter: sanitize preset   |  15.7 µs ⚠ |   267 |
| Threat scan (clean)       |  13.0 µs ⚠ |   322 |
| Threat scan (dirty)       |  14.0 µs ⚠ |   302 |
| Streaming (L1)            |  57.4 µs ⚠ |   545 |
| Streaming (L2)            | 458.3 µs ⚠ |   545 |
| Streaming (L3)            |  20.2 ms ⚠ |   593 |
| Unicode normalize         |  36.3 µs ⚠ |    89 |
| Transform: passthrough    | 130.7 ns ⚠ | 32099 |
| Transform: truecolor→mono |  27.8 µs ⚠ |   168 |
| Transform: truecolor→grey |  29.1 µs ⚠ |   160 |
| Transform: truecolor→16   |  30.0 µs ⚠ |   155 |
| Transform: truecolor→256  |  29.9 µs ⚠ |   156 |
| Transform: 256→16         |  22.4 µs ⚠ |   174 |
| Transform: 256→grey       |  24.5 µs ⚠ |   159 |
| Transform: basic→mono     |  30.1 µs ⚠ |   139 |
| Augment: protanopia       |   3.0 µs ⚠ |   245 |
| Augment: deuteranopia     |   3.0 µs ⚠ |   247 |
| Augment: sRGB roundtrip   | 726.6 ns ⚠ |   336 |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

## Scaling

Dirty throughput (MiB/s) across input sizes.
Constant bar length = O(n). Shrinking = super-linear.

RSS Δ and CPU shown at largest size only — small-size
values are dominated by benchmark harness overhead.

### `distill-strip-ansi` v0.6.1 — O(n) · RSS Δ 19.4 MiB · CPU 27.4 s

```text
  2 KiB ███████████████████████ 376
  4 KiB ████████████████████████ 402
  8 KiB █████████████████████████ 418
 16 KiB ███████████████████████████ 438
 32 KiB █████████████████████████████ 470
 64 KiB █████████████████████████████ 471
128 KiB ██████████████████████████████ 485
256 KiB ████████████████████████████ 466
512 KiB ██████████████████████████ 432
  1 MiB █████████████████████████ 415
  2 MiB ████████████████████████ 398
  4 MiB ████████████████████████ 400
  8 MiB █████████████████████████ 412
 24 MiB ███████████████████████████ 447
 32 MiB ███████████████████████████ 445
```

### `fast-strip-ansi` v0.13.1 — O(n) · RSS Δ 19.5 MiB · CPU 30.2 s

```text
  2 KiB ███████████████████ 318
  4 KiB █████████████████████ 340
  8 KiB █████████████████████ 346
 16 KiB ██████████████████████ 361
 32 KiB ███████████████████████ 373
 64 KiB ██████████████████ 301
128 KiB ██████████████████ 292
256 KiB ███████████████ 258
512 KiB ██████████████ 240
  1 MiB █████████████████ 275
  2 MiB ██████████████████ 299
  4 MiB ███████████████████ 309
  8 MiB ████████████████████ 331
 24 MiB ███████████████████████ 378
 32 MiB ███████████████████████ 374
```

### `console` v0.16.3 — O(n) · RSS Δ 1.5 MiB · CPU 57.1 s

```text
  2 KiB ███████ 129
  4 KiB ████████ 136
  8 KiB ████████ 143
 16 KiB █████████ 148
 32 KiB █████████ 151
 64 KiB █████████ 147
128 KiB ████████ 143
256 KiB █████████ 158
512 KiB ██████████ 170
  1 MiB ███████████ 180
  2 MiB ██████████ 164
  4 MiB █████████ 155
  8 MiB █████████ 152
 24 MiB █████████ 158
 32 MiB █████████ 147
```

### `strip-ansi-escapes` v0.2.1 — O(n) · RSS Δ 13.7 MiB · CPU 123.7 s

```text
  2 KiB ██ 42
  4 KiB ██ 45
  8 KiB ███ 49
 16 KiB ███ 50
 32 KiB ███ 51
 64 KiB ███ 51
128 KiB ███ 50
256 KiB ███ 51
512 KiB ███ 51
  1 MiB ███ 51
  2 MiB ███ 52
  4 MiB ███ 53
  8 MiB ███ 53
 24 MiB ███ 54
 32 MiB ███ 57
```

### `vtparse` v0.7.0 — O(n) · RSS Δ 19.6 MiB · CPU 80.3 s

```text
  2 KiB ████ 80
  4 KiB █████ 85
  8 KiB █████ 81
 16 KiB █████ 81
 32 KiB █████ 83
 64 KiB █████ 88
128 KiB █████ 91
256 KiB █████ 88
512 KiB █████ 91
  1 MiB █████ 88
  2 MiB █████ 90
  4 MiB █████ 91
  8 MiB █████ 94
 24 MiB █████ 95
 32 MiB █████ 94
```

### Complexity Summary

| Crate                | Dirty | Clean |
| -------------------- | ----- | ----- |
| `distill-strip-ansi` | O(n)  | O(n)  |
| `fast-strip-ansi`    | O(n)  | O(n)  |
| `console`            | O(n)  | O(n)  |
| `strip-ansi-escapes` | O(n)  | O(n)  |
| `vtparse`            | O(n)  | O(n)  |

Complexity estimated per memory tier (L1/L2/L3/DRAM) —
throughput steps between tiers are hardware, not algorithmic.
