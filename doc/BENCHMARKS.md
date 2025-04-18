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

- 794 MiB/s dirty throughput (4 KiB, ~20% ANSI)
- 2.2 GiB/s clean fast path (24 MiB)
- Zero allocation on clean input (`Cow::Borrowed`)
- No temp files, no disk I/O — pure in-memory
- O(n) linear scaling — constant-ish throughput up to 1 GiB+

## Environment

| Key        | Value                                    |
| ---------- | ---------------------------------------- |
| CPU        | Intel(R) Core(TM) i7-4790K CPU @ 4.00GHz |
| Arch       | x86_64                                   |
| OS         | Linux 7.0.5-1-cachyos                    |
| Rust       | 1.95.0                                   |
| Date       | 2026-05-09                               |
| L1d        | 32.0K                                    |
| L2         | 256.0K                                   |
| L3         | 12.0 MiB                                 |
| RAM        | 31.2 GiB                                 |
| target-cpu | `x86-64-v3`                              |
| Sizes      | 21 tiers (hardware-adaptive)             |
| Bench time | 10m51s                                   |

### Crate Versions

| Crate                | Version |
| -------------------- | ------: |
| `distill-strip-ansi` |   0.6.1 |
| `fast-strip-ansi`    |  0.13.1 |
| `console`            |  0.16.3 |
| `strip-ansi-escapes` |   0.2.1 |
| `criterion`          |   0.7.0 |

## Crate Footprints

| Crate                | Deps | Peak RSS |   RSS Δ |     CPU |
| -------------------- | ---: | -------: | ------: | ------: |
| `distill-strip-ansi` |    2 | 70.7 MiB | 2.9 MiB |  46.8 s |
| `fast-strip-ansi`    |    3 | 84.3 MiB | 5.5 MiB |  74.5 s |
| `console`            |    2 | 93.9 MiB |  236.0K |  55.9 s |
| `strip-ansi-escapes` |    2 | 92.6 MiB | 7.9 MiB | 131.0 s |

`strip-ansi` binary: 7.7 MiB, 24 deps
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
| `distill-strip-ansi` |  2.5 µs ⚠ |   778 |  base |  32.4M |
| `fast-strip-ansi`    |  3.3 µs ⚠ |   593 |  1.3× |  40.4M |
| `console`            |  8.6 µs ⚠ |   227 |  3.4× | 125.4M |
| `strip-ansi-escapes` | 33.7 µs ⚠ |    58 | 13.4× | 371.4M |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

### Dirty 4 KiB

| Crate                |      Time | MiB/s |     × | Ir/MiB |
| -------------------- | --------: | ----: | ----: | -----: |
| `distill-strip-ansi` |  4.9 µs ⚠ |   794 |  base |  16.2M |
| `fast-strip-ansi`    |  6.9 µs ⚠ |   565 |  1.4× |  20.2M |
| `console`            | 16.6 µs ⚠ |   236 |  3.4× |  62.7M |
| `strip-ansi-escapes` | 57.4 µs ⚠ |    68 | 11.7× | 185.7M |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

### Dirty 32 KiB

| Crate                |       Time | MiB/s |     × | Ir/MiB |
| -------------------- | ---------: | ----: | ----: | -----: |
| `distill-strip-ansi` |  38.0 µs ⚠ |   821 |  base |  30.8M |
| `fast-strip-ansi`    |  54.5 µs ⚠ |   573 |  1.4× |  40.8M |
| `console`            | 135.9 µs ⚠ |   230 |  3.6× | 111.4M |
| `strip-ansi-escapes` | 465.3 µs ⚠ |    67 | 12.2× | 357.7M |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

### Dirty 256 KiB

| Crate                |       Time | MiB/s |     × | Ir/MiB |
| -------------------- | ---------: | ----: | ----: | -----: |
| `distill-strip-ansi` | 329.3 µs ⚠ |   759 |  base |   3.9M |
| `fast-strip-ansi`    | 432.0 µs ⚠ |   579 |  1.3× |   5.1M |
| `console`            |   1.1 ms ⚠ |   227 |  3.3× |  13.9M |
| `strip-ansi-escapes` |   4.4 ms ⚠ |    57 | 13.4× |  44.7M |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

### Dirty 24 MiB

| Crate                |       Time | MiB/s |     × | Ir/MiB |
| -------------------- | ---------: | ----: | ----: | -----: |
| `distill-strip-ansi` |  34.6 ms ⚠ |   693 |  base |  10.1M |
| `fast-strip-ansi`    | 121.9 ms ⚠ |   197 |  3.5× |  12.8M |
| `console`            | 136.5 ms ⚠ |   176 |  3.9× |  36.1M |
| `strip-ansi-escapes` | 426.7 ms ⚠ |    56 | 12.3× | 118.2M |

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
| `distill-strip-ansi` |  75.5 ms ⚠ |   635 |  base |   5.0M |
| `fast-strip-ansi`    |  95.4 ms ⚠ |   503 |  1.3× |   6.4M |
| `console`            | 216.5 ms ⚠ |   222 |  2.9× |  18.1M |
| `strip-ansi-escapes` | 852.9 ms ⚠ |    56 | 11.3× |  59.1M |

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
| `distill-strip-ansi` | 162.7 ns ⚠ | 32824 |   base |  12.4M |
| `fast-strip-ansi`    |   3.7 µs ⚠ |  1439 |  22.8× |  16.3M |
| `console`            |  10.2 µs ⚠ |   524 |  62.6× |  48.5M |
| `strip-ansi-escapes` |  99.7 µs ⚠ |    54 | 612.5× | 147.7M |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

### OSC 8 Hyperlinks (4 KiB)

| Crate                |       Time | MiB/s |      × | Ir/MiB |
| -------------------- | ---------: | ----: | -----: | -----: |
| `distill-strip-ansi` | 136.4 ns ⚠ | 30121 |   base |  11.2M |
| `fast-strip-ansi`    |   3.0 µs ⚠ |  1375 |  21.9× |  33.7M |
| `console`            |   8.9 µs ⚠ |   464 |  65.0× |  25.3M |
| `strip-ansi-escapes` |  84.2 µs ⚠ |    49 | 617.0× |  67.5M |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

### Extended Capabilities

Additional features available in `distill-strip-ansi`.

| Feature                   |       Time | MiB/s | Ir/MiB |
| ------------------------- | ---------: | ----: | -----: |
| Classify (parse only)     |  12.8 µs ⚠ |   327 |  33.5M |
| Classify + detail         |  12.7 µs ⚠ |   331 |  32.6M |
| Filter: SGR mask          |  13.7 µs ⚠ |   307 |  39.5M |
| Filter: sanitize preset   |  13.5 µs ⚠ |   310 |  41.3M |
| Threat scan (clean)       |  12.3 µs ⚠ |   342 |  32.2M |
| Threat scan (dirty)       |  11.1 µs ⚠ |   379 |  32.3M |
| Streaming (L1)            |  43.0 µs ⚠ |   727 |  33.5M |
| Streaming (L2)            | 366.7 µs ⚠ |   682 |   4.2M |
| Streaming (L3)            |  19.6 ms ⚠ |   612 |  21.9M |
| Unicode normalize         |  22.5 µs ⚠ |   144 |  94.1M |
| Transform: passthrough    | 115.6 ns ⚠ | 36286 |   1.9M |
| Transform: truecolor→mono |  26.2 µs ⚠ |   178 |  64.7M |
| Transform: truecolor→grey |  27.6 µs ⚠ |   168 |  68.8M |
| Transform: truecolor→16   |  33.3 µs ⚠ |   140 |  67.5M |
| Transform: truecolor→256  |  26.6 µs ⚠ |   175 |  68.8M |
| Transform: 256→16         |  41.5 µs ⚠ |    94 |  60.7M |
| Transform: 256→grey       |  23.1 µs ⚠ |   169 |  69.8M |
| Transform: basic→mono     |  64.5 µs ⚠ |    65 |  64.4M |
| Augment: protanopia       |   3.1 µs ⚠ |   238 |  52.0M |
| Augment: deuteranopia     |   2.8 µs ⚠ |   261 |  52.0M |
| Augment: sRGB roundtrip   | 782.6 ns ⚠ |   312 |  29.3M |

⚠ marks cells where CV ≥ 3% — re-run `mise x bench:callgrind`
for a deterministic `Ir/MiB` check.

## Scaling

Dirty throughput (MiB/s) across input sizes.
Constant bar length = O(n). Shrinking = super-linear.

RSS Δ and CPU shown at largest size only — small-size
values are dominated by benchmark harness overhead.

### `distill-strip-ansi` v0.6.1 — O(n)

```text
  2 KiB ██████████████████████████ 778
  4 KiB ███████████████████████████ 794
  8 KiB ██████████████████████████ 751
 16 KiB ████████████████████████████ 835
 32 KiB ████████████████████████████ 821
 64 KiB ████████████████████████████ 813
128 KiB █████████████████████████ 735
256 KiB ██████████████████████████ 759
512 KiB ██████████████████████████ 762
  1 MiB ██████████████████████████████ 867
  2 MiB ██████████████████████████ 762
  4 MiB ██████████████████████████ 756
  8 MiB ████████████████████████ 709
 24 MiB ████████████████████████ 693
 32 MiB ██████████████████████ 663
 48 MiB ██████████████████████ 635
 96 MiB ███████████████████████ 676
192 MiB ██████████████████ 545
384 MiB █████████████████████ 628
768 MiB █████████████████████ 615
  1 GiB █████████████████████ 608
```

### `fast-strip-ansi` v0.13.1 — O(n)

```text
  2 KiB ████████████████████ 593
  4 KiB ███████████████████ 565
  8 KiB ███████████████████ 563
 16 KiB █████████████████ 512
 32 KiB ███████████████████ 573
 64 KiB ██████████████████ 541
128 KiB ███████████████████ 573
256 KiB ████████████████████ 579
512 KiB ██████████████████ 529
  1 MiB ████████████████████ 598
  2 MiB █████████████████████ 622
  4 MiB █████████████████ 514
  8 MiB █████████████████ 511
 24 MiB ██████ 197
 32 MiB ██████████████ 429
 48 MiB █████████████████ 503
 96 MiB ███████████████ 451
192 MiB ██████████████ 419
384 MiB ██████████████ 414
768 MiB ██████████████ 415
  1 GiB █████████████ 401
```

### `console` v0.16.3 — O(n)

```text
  2 KiB ███████ 227
  4 KiB ████████ 236
  8 KiB ████████ 256
 16 KiB ████████ 253
 32 KiB ███████ 230
 64 KiB ███████ 212
128 KiB ████████ 239
256 KiB ███████ 227
512 KiB ████████ 232
  1 MiB ███████ 230
  2 MiB ████████ 232
  4 MiB ███████ 210
  8 MiB ███████ 205
 24 MiB ██████ 176
 32 MiB ██████ 184
 48 MiB ███████ 222
 96 MiB ███████ 215
192 MiB ██████ 199
384 MiB ██████ 191
768 MiB ██████ 199
  1 GiB ███████ 205
```

### `strip-ansi-escapes` v0.2.1 — O(n)

```text
  2 KiB ██ 58
  4 KiB ██ 68
  8 KiB ██ 63
 16 KiB ██ 62
 32 KiB ██ 67
 64 KiB ██ 61
128 KiB ██ 61
256 KiB █ 57
512 KiB ██ 67
  1 MiB ██ 65
  2 MiB ██ 62
  4 MiB █ 58
  8 MiB █ 57
 24 MiB █ 56
 32 MiB █ 48
 48 MiB █ 56
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
