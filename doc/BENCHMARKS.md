# Benchmarks

Criterion.rs statistical benchmarks across the Rust ANSI
stripping ecosystem: `distill-strip-ansi`, `fast-strip-ansi`,
`strip-ansi-escapes`, and `console`.

## Symbolic Notation

| Symbol | Meaning                         |
| ------ | ------------------------------- |
| ns     | nanoseconds (10⁻⁹ s)            |
| µs     | microseconds (10⁻⁶ s)           |
| ms     | milliseconds (10⁻³ s)           |
| MiB/s  | mebibytes/sec (2²⁰ B/s)         |
| GiB/s  | gibibytes/sec (2³⁰ B/s)         |
| ×      | multiplier (baseline = distill) |
| RSS Δ  | memory retained after bench     |
| CPU    | user+sys CPU time (bench)       |

## Highlights for Humans

- 471 MiB/s dirty throughput (4 KiB, ~20% ANSI)
- 6.3 GiB/s clean fast path (24 MiB)
- Zero allocation on clean input (`Cow::Borrowed`)
- O(n) linear scaling — constant MiB/s to 1 GiB+
- No temp files, no disk I/O — pure in-memory

## Environmental Concerns

| Key        | Value                        |
| ---------- | ---------------------------- |
| CPU        | unknown                      |
| Arch       | x86_64                       |
| OS         | Linux 7.0.3-1-cachyos        |
| Rust       | 1.95.0                       |
| Date       | 2026-05-07                   |
| L1d        | 32.0K                        |
| L2         | 256.0K                       |
| L3         | 12.0 MiB                     |
| RAM        | 31.2 GiB                     |
| Sizes      | 15 tiers (hardware-adaptive) |
| Bench time | 3m29s                        |

### Crate Versions

| Crate                | Version |
| -------------------- | ------: |
| `distill-strip-ansi` |   0.6.0 |
| `fast-strip-ansi`    |  0.13.1 |
| `console`            |  0.16.3 |
| `strip-ansi-escapes` |   0.2.1 |
| `criterion`          |   0.7.0 |

## Crate Footprints

| Crate                | Deps | Peak RSS |    RSS Δ |    CPU |
| -------------------- | ---: | -------: | -------: | -----: |
| `distill-strip-ansi` |    2 | 77.5 MiB | 24.0 MiB | 29.2 s |
| `fast-strip-ansi`    |    3 | 93.8 MiB | 23.3 MiB | 31.8 s |
| `console`            |    2 | 76.1 MiB | 24.4 MiB | 26.3 s |
| `strip-ansi-escapes` |    2 | 75.1 MiB | 22.1 MiB | 29.2 s |

`strip-ansi` binary: 1.0 MiB, 24 deps
(includes `clap` for CLI argument parsing).

No crate uses temp files or disk I/O — stdin only.
Peak RSS, RSS Δ, and CPU measured at largest bench size.
RSS Δ reflects allocator page retention after the last
Criterion iteration — not a leak. CPU is user+sys time
for the benchmark (not wall clock). Resource snapshots
captured via `task_info` (macOS) / `getrusage` (POSIX)
outside the timed loop — no measurement overhead.

## Build Configuration (v0.6.0+)

Release builds use LTO and target-cpu tuning for maximum throughput.
Both development systems share the x86-64-v3 ISA level (Haswell+).

| Setting         | Value        | Effect                        |
| --------------- | ------------ | ----------------------------- |
| `lto`           | `"thin"`     | Cross-module inlining         |
| `codegen-units` | `1`          | Full optimizer visibility     |
| `target-cpu`    | `x86-64-v3`  | AVX2/FMA auto-vectorization   |
| `panic`         | `"abort"`    | No unwind tables              |
| `strip`         | `"symbols"`  | Reduced binary size           |

The `[profile.bench]` mirrors `lto` and `codegen-units` so
criterion numbers reflect release performance.

### Target CPU: x86-64-v3

Author systems (Intel i7-4790K Haswell, Intel i7-9750H Coffee Lake)
support the x86-64-v3 microarchitecture level:

- SSE4.2, AVX, AVX2, FMA, BMI1, BMI2, POPCNT, MOVBE, F16C, LZCNT

This enables FMA instructions for `palette.rs` matrix multiply
and AVX2 for auto-vectorizable loops. The `memchr` crate uses
runtime SIMD detection independently of this flag.

### Profile-Guided Optimization (PGO)

For maximum throughput (CI release builds):

```bash
# 1. Instrument
RUSTFLAGS="-Cprofile-generate=/tmp/pgo-data" cargo build --release

# 2. Collect profiles
./target/release/strip-ansi < tests/fixtures/ansi-heavy.txt > /dev/null
cargo bench --bench internals -- --profile-time 5

# 3. Merge and rebuild
llvm-profdata merge -o /tmp/pgo-data/merged.profdata /tmp/pgo-data
RUSTFLAGS="-Cprofile-use=/tmp/pgo-data/merged.profdata" cargo build --release
```

Expected gain: 10-20% on hot paths. The 15×256 state table
benefits most from trained branch prediction.

## HOWTO: Reproduce

```bash
# Quick run: up to 2×L3 cache (~3m29s)
./bin/generate-benchmarks-md.py

# Full run: all sizes including GiB-scale (~30 min)
./bin/generate-benchmarks-md.py --max-size 0

# Custom cap
./bin/generate-benchmarks-md.py --max-size 64M

# Report only from existing data (~1 sec)
./bin/generate-benchmarks-md.py --no-run
```

The generator runs five bench suites then renders this doc:

- `cargo bench --bench internals` — library internals:
  strip, stream, classifier, filter, threats, transforms,
  augments, unicode normalize
- `cargo bench -p ecosystem-bench --bench distill`
- `cargo bench -p ecosystem-bench --bench fast_strip`
- `cargo bench -p ecosystem-bench --bench console_bench`
- `cargo bench -p ecosystem-bench --bench strip_escapes`

Each ecosystem bench uses the same harness
(`distill-bench-harness`): identical sizes, config
(10 samples, 3s measurement, 1s warmup), and RSS/CPU
capture. Sizes are hardware-adaptive — the bench detects
L1/L2/L3 cache sizes and RAM, then picks boundary points.

### Test Data Strategy

| Tier            | Source               | Why            |
| --------------- | -------------------- | -------------- |
| ≤32.0K          | fixture or generated | L1 cache       |
| 32.0K–256.0K    | generated in RAM     | L2 cache       |
| 256.0K–12.0 MiB | generated in RAM     | L3 boundary    |
| >12.0 MiB       | generated in RAM     | DRAM bandwidth |

Each size selects the closest `tests/fixtures/*.raw.txt`
file that contains ANSI sequences (0.25×–4× tolerance).
When no fixture fits, synthetic ~20% ANSI data is generated.
Fixtures above ~1 KiB with ANSI are rare, so most tiers
use generated data.

## Details That Matter

All crates: `&[u8]` input. `console`: `&str`
(conversion outside timed loop). `distill-strip-ansi`
used as baseline (Relative = time / baseline time).

### Dirty 2 KiB

| Crate                |    Time | MiB/s |        × |   RSS Δ |    CPU |
| -------------------- | ------: | ----: | -------: | ------: | -----: |
| `distill-strip-ansi` |  3.4 µs |   583 | baseline | 3.5 MiB | 30.8 s |
| `fast-strip-ansi`    |  4.9 µs |   397 |     1.5× | 5.7 MiB | 32.8 s |
| `console`            | 12.5 µs |   157 |     3.7× | 4.2 MiB | 29.4 s |
| `strip-ansi-escapes` | 45.9 µs |    43 |    13.7× | 2.2 MiB | 30.0 s |

### Dirty 4 KiB

| Crate                |    Time | MiB/s |        × |   RSS Δ |    CPU |
| -------------------- | ------: | ----: | -------: | ------: | -----: |
| `distill-strip-ansi` |  8.3 µs |   471 | baseline | 5.1 MiB | 29.8 s |
| `fast-strip-ansi`    | 11.1 µs |   351 |     1.3× | 6.8 MiB | 31.9 s |
| `console`            | 32.7 µs |   120 |     3.9× | 3.2 MiB | 28.9 s |
| `strip-ansi-escapes` | 94.1 µs |    41 |    11.3× | 4.9 MiB | 30.1 s |

### Dirty 32 KiB

| Crate                |     Time | MiB/s |        × |   RSS Δ |    CPU |
| -------------------- | -------: | ----: | -------: | ------: | -----: |
| `distill-strip-ansi` |  57.9 µs |   539 | baseline | 3.5 MiB | 30.1 s |
| `fast-strip-ansi`    |  76.3 µs |   410 |     1.3× | 5.0 MiB | 32.1 s |
| `console`            | 197.6 µs |   158 |     3.4× | 5.4 MiB | 30.1 s |
| `strip-ansi-escapes` | 754.6 µs |    41 |    13.0× | 3.9 MiB | 29.7 s |

### Dirty 256 KiB

| Crate                |     Time | MiB/s |        × |   RSS Δ |    CPU |
| -------------------- | -------: | ----: | -------: | ------: | -----: |
| `distill-strip-ansi` | 493.3 µs |   507 | baseline | 4.3 MiB | 29.8 s |
| `fast-strip-ansi`    | 767.7 µs |   326 |     1.6× | 4.7 MiB | 30.5 s |
| `console`            |   1.6 ms |   161 |     3.1× | 4.9 MiB | 31.1 s |
| `strip-ansi-escapes` |   4.9 ms |    51 |    10.0× | 2.4 MiB | 32.3 s |

### Dirty 24 MiB

| Crate                |     Time | MiB/s |        × |    RSS Δ |    CPU |
| -------------------- | -------: | ----: | -------: | -------: | -----: |
| `distill-strip-ansi` |  43.4 ms |   553 | baseline | 19.8 MiB | 31.6 s |
| `fast-strip-ansi`    |  66.4 ms |   362 |     1.5× | 33.4 MiB | 31.0 s |
| `console`            | 182.6 ms |   131 |     4.2× | 18.8 MiB | 25.7 s |
| `strip-ansi-escapes` | 522.6 ms |    46 |    12.0× | 18.3 MiB | 27.8 s |

### Dirty 32 MiB

| Crate                |     Time | MiB/s |        × |    RSS Δ |    CPU |
| -------------------- | -------: | ----: | -------: | -------: | -----: |
| `distill-strip-ansi` |  54.4 ms |   588 | baseline | 24.0 MiB | 29.2 s |
| `fast-strip-ansi`    |  86.5 ms |   370 |     1.6× | 23.3 MiB | 31.8 s |
| `console`            | 180.1 ms |   178 |     3.3× | 24.4 MiB | 26.3 s |
| `strip-ansi-escapes` | 651.5 ms |    49 |    12.0× | 22.1 MiB | 29.2 s |

### Cargo Output (5 KiB)

| Crate                |     Time | MiB/s |        × |   RSS Δ |    CPU |
| -------------------- | -------: | ----: | -------: | ------: | -----: |
| `distill-strip-ansi` | 261.2 ns | 20449 | baseline | 4.7 MiB | 34.4 s |
| `fast-strip-ansi`    |   5.7 µs |   934 |    21.9× | 5.3 MiB | 29.1 s |
| `console`            |  16.0 µs |   334 |    61.1× | 6.7 MiB | 28.0 s |
| `strip-ansi-escapes` | 177.7 µs |    30 |   680.4× | 6.0 MiB | 28.7 s |

### OSC 8 Hyperlinks (4 KiB)

| Crate                |     Time | MiB/s |        × |   RSS Δ |    CPU |
| -------------------- | -------: | ----: | -------: | ------: | -----: |
| `distill-strip-ansi` | 190.9 ns | 21522 | baseline | 4.8 MiB | 33.1 s |
| `fast-strip-ansi`    |   5.4 µs |   767 |    28.1× | 5.6 MiB | 27.2 s |
| `console`            |  11.8 µs |   349 |    61.7× | 5.9 MiB | 29.3 s |
| `strip-ansi-escapes` | 135.8 µs |    30 |   711.4× | 5.6 MiB | 29.8 s |

### Extended Capabilities

Additional features available in `distill-strip-ansi`.

| Feature                   |     Time | MiB/s |    RSS Δ |    CPU |
| ------------------------- | -------: | ----: | -------: | -----: |
| Classify (parse only)     |  16.0 µs |   263 |  4.8 MiB | 29.7 s |
| Classify + detail         |  14.8 µs |   284 |  4.9 MiB | 32.3 s |
| Filter: SGR mask          |  17.4 µs |   241 |  4.8 MiB | 29.9 s |
| Filter: sanitize preset   |  22.8 µs |   184 |  4.9 MiB | 29.7 s |
| Threat scan (clean)       |  18.5 µs |   227 |  4.9 MiB | 30.9 s |
| Threat scan (dirty)       |  19.5 µs |   216 |  4.9 MiB | 30.1 s |
| Streaming (L1)            |  61.1 µs |   511 |  4.3 MiB | 29.2 s |
| Streaming (L2)            | 585.8 µs |   427 |  3.2 MiB | 28.7 s |
| Streaming (L3)            |  29.3 ms |   409 | 18.3 MiB | 30.0 s |
| Unicode normalize         |  25.5 µs |   127 |  4.9 MiB | 30.3 s |
| Transform: passthrough    | 187.0 ns | 22443 |  4.9 MiB | 30.2 s |
| Transform: truecolor→mono |  36.8 µs |   127 |  5.4 MiB | 31.2 s |
| Transform: truecolor→grey |  30.9 µs |   151 |  4.8 MiB | 32.8 s |
| Transform: truecolor→16   |  35.1 µs |   133 |  5.0 MiB | 30.3 s |
| Transform: truecolor→256  |  36.7 µs |   127 |  5.5 MiB | 31.5 s |
| Transform: 256→16         |  23.6 µs |   165 |  4.8 MiB | 32.1 s |
| Transform: 256→grey       |  32.4 µs |   120 |  5.4 MiB | 30.3 s |
| Transform: basic→mono     |  37.4 µs |   112 |  4.8 MiB | 30.3 s |
| Augment: protanopia       |   5.0 µs |   148 |  4.8 MiB | 29.9 s |
| Augment: deuteranopia     |   4.6 µs |   160 |  4.9 MiB | 29.6 s |
| Augment: sRGB roundtrip   |   1.0 µs |   235 |  5.0 MiB | 29.6 s |

## Scaling

Dirty throughput (MiB/s) across input sizes.
Constant bar length = O(n). Shrinking = super-linear.

RSS Δ and CPU shown at largest size only — small-size
values are dominated by benchmark harness overhead.

### `distill-strip-ansi` v0.6.0 — O(n) · RSS Δ 24.0 MiB · CPU 29.2 s

```text
  2 KiB ████████████████████████████ 583
  4 KiB ██████████████████████ 471
  8 KiB ████████████████████████████ 584
 16 KiB ██████████████████████████ 540
 32 KiB ██████████████████████████ 539
 64 KiB █████████████████████████ 526
128 KiB █████████████████████████ 526
256 KiB ████████████████████████ 507
512 KiB ████████████████████████████ 584
  1 MiB ████████████████████████████ 589
  2 MiB █████████████████████████ 532
  4 MiB ██████████████████████████████ 623
  8 MiB ████████████████████████ 513
 24 MiB ██████████████████████████ 553
 32 MiB ████████████████████████████ 588
```

### `fast-strip-ansi` v0.13.1 — O(n) · RSS Δ 23.3 MiB · CPU 31.8 s

```text
  2 KiB ███████████████████ 397
  4 KiB ████████████████ 351
  8 KiB ██████████████████ 385
 16 KiB ████████████████████ 422
 32 KiB ███████████████████ 410
 64 KiB ██████████████████ 392
128 KiB ███████████████ 318
256 KiB ███████████████ 326
512 KiB ███████████████████ 397
  1 MiB ███████████████████ 411
  2 MiB █████████████████ 367
  4 MiB ███████████████████ 394
  8 MiB ███████████████████ 396
 24 MiB █████████████████ 362
 32 MiB █████████████████ 370
```

### `console` v0.16.3 — O(n) · RSS Δ 24.4 MiB · CPU 26.3 s

```text
  2 KiB ███████ 157
  4 KiB █████ 120
  8 KiB ███████ 154
 16 KiB ██████ 135
 32 KiB ███████ 158
 64 KiB ███████ 155
128 KiB ███████ 165
256 KiB ███████ 161
512 KiB ███████ 163
  1 MiB ██████ 142
  2 MiB ███████ 147
  4 MiB ██████ 142
  8 MiB ████████ 171
 24 MiB ██████ 131
 32 MiB ████████ 178
```

### `strip-ansi-escapes` v0.2.1 — O(n) · RSS Δ 22.1 MiB · CPU 29.2 s

```text
  2 KiB ██ 43
  4 KiB ██ 41
  8 KiB █ 41
 16 KiB █ 37
 32 KiB █ 41
 64 KiB █ 38
128 KiB ██ 46
256 KiB ██ 51
512 KiB ██ 42
  1 MiB █ 41
  2 MiB ██ 45
  4 MiB ██ 43
  8 MiB █ 41
 24 MiB ██ 46
 32 MiB ██ 49
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
