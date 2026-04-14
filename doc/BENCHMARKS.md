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

- 651 MiB/s dirty throughput (4 KiB, ~20% ANSI)
- 3.2 GiB/s clean fast path (24 MiB)
- Zero allocation on clean input (`Cow::Borrowed`)
- O(n) linear scaling — constant MiB/s to 1 GiB+
- No temp files, no disk I/O — pure in-memory

## Environmental Concerns

<!-- BENCH:ENV:START -->

| Key        | Value                                    |
| ---------- | ---------------------------------------- |
| CPU        | Intel(R) Core(TM) i7-9750H CPU @ 2.60GHz |
| Arch       | x86_64                                   |
| OS         | macOS 26.4.1                             |
| Rust       | 1.94.1                                   |
| Date       | 2026-04-13                               |
| L1d        | 32.0K                                    |
| L2         | 256.0K                                   |
| L3         | 12.0 MiB                                 |
| RAM        | 32.0 GiB                                 |
| Sizes      | 15 tiers (hardware-adaptive)             |
| Bench time | 2m26s                                    |

<!-- BENCH:ENV:END -->

### Crate Versions

<!-- BENCH:VERSIONS:START -->

| Crate                | Version |
| -------------------- | ------: |
| `distill-strip-ansi` |   0.5.1 |
| `fast-strip-ansi`    |  0.13.1 |
| `console`            |  0.16.3 |
| `strip-ansi-escapes` |   0.2.1 |
| `criterion`          |   0.7.0 |

<!-- BENCH:VERSIONS:END -->

## Crate Footprints

<!-- BENCH:FOOTPRINT:START -->

| Crate                |  Binary | Deps |  Peak RSS |    RSS Δ |    CPU |
| -------------------- | ------: | ---: | --------: | -------: | -----: |
| `distill-strip-ansi` | 1.5 MiB |   24 | 198.1 MiB | 20.9 MiB | 13.4 s |
| `fast-strip-ansi`    |     n/a |    3 | 237.7 MiB | 20.7 MiB | 13.6 s |
| `console`            |     n/a |    — | 181.6 MiB |  1.6 MiB | 12.8 s |
| `strip-ansi-escapes` |     n/a |    2 | 206.8 MiB | 13.1 MiB | 14.9 s |

<!-- BENCH:FOOTPRINT:END -->

No crate uses temp files or disk I/O — stdin only.
Peak RSS, RSS Δ, and CPU measured at largest bench size.
RSS Δ reflects allocator page retention after the last
Criterion iteration — not a leak. CPU is user+sys time
for the benchmark (not wall clock). Resource snapshots
captured via `task_info` (macOS) / `getrusage` (POSIX)
outside the timed loop — no measurement overhead.

## HOWTO: Reproduce

```bash
# Quick run: up to 2×L3 cache (~2m26s)
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

<!-- BENCH:ECO_DIRTY_2048:START -->

| Crate                |    Time | MiB/s |        × |   RSS Δ |    CPU |
| -------------------- | ------: | ----: | -------: | ------: | -----: |
| `distill-strip-ansi` |  3.2 µs |   606 | baseline | 3.3 MiB | 12.6 s |
| `fast-strip-ansi`    |  4.1 µs |   475 |     1.3× | 3.0 MiB | 12.1 s |
| `console`            |  9.9 µs |   197 |     3.1× | 2.3 MiB | 12.3 s |
| `strip-ansi-escapes` | 36.6 µs |    53 |    11.4× | 2.4 MiB | 12.3 s |
<!-- BENCH:ECO_DIRTY_2048:END -->

### Dirty 4 KiB

<!-- BENCH:ECO_DIRTY_4096:START -->

| Crate                |    Time | MiB/s |        × |   RSS Δ |    CPU |
| -------------------- | ------: | ----: | -------: | ------: | -----: |
| `distill-strip-ansi` |  6.0 µs |   651 | baseline | 2.9 MiB | 12.6 s |
| `fast-strip-ansi`    |  7.7 µs |   505 |     1.3× | 2.7 MiB | 12.0 s |
| `console`            | 18.9 µs |   206 |     3.2× | 1.1 MiB | 12.2 s |
| `strip-ansi-escapes` | 72.7 µs |    54 |    12.1× | 3.0 MiB | 12.1 s |
<!-- BENCH:ECO_DIRTY_4096:END -->

### Dirty 32 KiB

<!-- BENCH:ECO_DIRTY_32768:START -->

| Crate                |     Time | MiB/s |        × |   RSS Δ |    CPU |
| -------------------- | -------: | ----: | -------: | ------: | -----: |
| `distill-strip-ansi` |  48.0 µs |   651 | baseline | 1.6 MiB | 12.6 s |
| `fast-strip-ansi`    |  59.5 µs |   526 |     1.2× |  732.0K | 13.0 s |
| `console`            | 141.3 µs |   221 |     2.9× |  716.0K | 12.2 s |
| `strip-ansi-escapes` | 592.2 µs |    53 |    12.3× |  408.0K | 12.3 s |
<!-- BENCH:ECO_DIRTY_32768:END -->

### Dirty 256 KiB

<!-- BENCH:ECO_DIRTY_262144:START -->

| Crate                |     Time | MiB/s |        × |   RSS Δ |    CPU |
| -------------------- | -------: | ----: | -------: | ------: | -----: |
| `distill-strip-ansi` | 383.3 µs |   652 | baseline |  188.0K | 12.6 s |
| `fast-strip-ansi`    | 505.0 µs |   495 |     1.3× | 1.1 MiB | 12.1 s |
| `console`            |   1.2 ms |   213 |     3.1× | 1.2 MiB | 12.3 s |
| `strip-ansi-escapes` |   4.8 ms |    53 |    12.4× |  992.0K | 12.4 s |
<!-- BENCH:ECO_DIRTY_262144:END -->

### Dirty 24 MiB

<!-- BENCH:ECO_DIRTY_25165824:START -->

| Crate                |     Time | MiB/s |        × |    RSS Δ |    CPU |
| -------------------- | -------: | ----: | -------: | -------: | -----: |
| `distill-strip-ansi` |  36.9 ms |   650 | baseline | 16.1 MiB | 13.1 s |
| `fast-strip-ansi`    |  47.0 ms |   511 |     1.3× | 30.1 MiB | 14.9 s |
| `console`            | 110.8 ms |   217 |     3.0× | 11.3 MiB | 12.2 s |
| `strip-ansi-escapes` | 446.8 ms |    54 |    12.1× | 29.2 MiB | 13.0 s |
<!-- BENCH:ECO_DIRTY_25165824:END -->

### Dirty 32 MiB

<!-- BENCH:ECO_DIRTY_33554432:START -->

| Crate                |     Time | MiB/s |        × |    RSS Δ |    CPU |
| -------------------- | -------: | ----: | -------: | -------: | -----: |
| `distill-strip-ansi` |  60.9 ms |   525 | baseline | 20.9 MiB | 13.4 s |
| `fast-strip-ansi`    |  65.0 ms |   492 |     1.1× | 20.7 MiB | 13.6 s |
| `console`            | 148.0 ms |   216 |     2.4× |  1.6 MiB | 12.8 s |
| `strip-ansi-escapes` | 587.0 ms |    55 |     9.6× | 13.1 MiB | 14.9 s |
<!-- BENCH:ECO_DIRTY_33554432:END -->

### Cargo Output (5 KiB)

<!-- BENCH:ECO_CARGO:START -->

| Crate                |     Time | MiB/s |        × |   RSS Δ |    CPU |
| -------------------- | -------: | ----: | -------: | ------: | -----: |
| `distill-strip-ansi` | 143.7 ns | 37152 | baseline | 2.8 MiB | 12.2 s |
| `fast-strip-ansi`    |   2.9 µs |  1818 |    20.4× |  648.0K | 12.4 s |
| `console`            |  15.5 µs |   346 |   107.5× |  112.0K | 12.0 s |
| `strip-ansi-escapes` | 132.2 µs |    40 |   919.6× |  660.0K | 12.3 s |
<!-- BENCH:ECO_CARGO:END -->

### OSC 8 Hyperlinks (4 KiB)

<!-- BENCH:ECO_OSC8:START -->

| Crate                |     Time | MiB/s |        × |  RSS Δ |    CPU |
| -------------------- | -------: | ----: | -------: | -----: | -----: |
| `distill-strip-ansi` | 125.2 ns | 32810 | baseline | 404.0K | 12.1 s |
| `fast-strip-ansi`    |   2.3 µs |  1772 |    18.5× |  88.0K | 12.2 s |
| `console`            |  12.3 µs |   333 |    98.6× |  92.0K | 12.7 s |
| `strip-ansi-escapes` | 111.2 µs |    37 |   887.8× |  44.0K | 12.8 s |
<!-- BENCH:ECO_OSC8:END -->

### Extended Capabilities

Additional features available in `distill-strip-ansi`.

| Feature                   |     Time | MiB/s |   RSS Δ |    CPU |
| ------------------------- | -------: | ----: | ------: | -----: |
| Classify (parse only)     |  12.2 µs |   344 |  544.0K | 12.6 s |
| Classify + detail         |  12.7 µs |   329 |  540.0K | 12.6 s |
| Filter: SGR mask          |  15.1 µs |   277 | 1.5 MiB | 12.0 s |
| Filter: sanitize preset   |  15.8 µs |   265 |  608.0K | 12.0 s |
| Threat scan (clean)       |  12.7 µs |   329 |    8.0K | 12.7 s |
| Threat scan (dirty)       |  13.7 µs |   307 |   80.0K | 12.7 s |
| Streaming (L1)            |  49.8 µs |   628 |  988.0K | 12.7 s |
| Streaming (L2)            | 393.7 µs |   635 |  592.0K | 12.6 s |
| Streaming (L3)            |  19.1 ms |   628 | 8.3 MiB | 12.4 s |
| Unicode normalize         |  25.2 µs |   129 |  936.0K | 12.5 s |
| Transform: passthrough    | 114.2 ns | 36740 |       0 | 12.8 s |
| Transform: truecolor→mono |  24.9 µs |   187 | 1.3 MiB | 12.6 s |
| Transform: truecolor→grey |  27.3 µs |   171 |  132.0K | 12.8 s |
| Transform: truecolor→16   |  27.6 µs |   169 |  208.0K | 12.9 s |
| Transform: truecolor→256  |  29.6 µs |   157 |  196.0K | 12.8 s |
| Transform: 256→16         |  20.6 µs |   189 |  208.0K | 12.3 s |
| Transform: 256→grey       |  22.2 µs |   175 |  204.0K | 12.6 s |
| Transform: basic→mono     |  28.2 µs |   149 |  392.0K | 12.8 s |
| Augment: protanopia       |   3.3 µs |   225 |  600.0K | 12.6 s |
| Augment: deuteranopia     |   3.1 µs |   234 | 1.1 MiB | 12.5 s |
| Augment: sRGB roundtrip   | 719.1 ns |   340 |  476.0K | 12.3 s |

## Scaling

Dirty throughput (MiB/s) across input sizes.
Constant bar length = O(n). Shrinking = super-linear.

RSS Δ and CPU shown at largest size only — small-size
values are dominated by benchmark harness overhead.
### `distill-strip-ansi` v0.5.1 — O(n) · RSS Δ 20.9 MiB · CPU 13.4 s

```text
  2 KiB ███████████████████████████ 606
  4 KiB █████████████████████████████ 651
  8 KiB ██████████████████████████████ 667
 16 KiB █████████████████████████████ 660
 32 KiB █████████████████████████████ 651
 64 KiB ████████████████████████████ 640
128 KiB ████████████████████████████ 640
256 KiB █████████████████████████████ 652
512 KiB ████████████████████████████ 637
  1 MiB █████████████████████████████ 649
  2 MiB ██████████████████████ 493
  4 MiB █████████████████████ 484
  8 MiB ██████████████████████ 489
 24 MiB █████████████████████████████ 650
 32 MiB ███████████████████████ 525
```

### `fast-strip-ansi` v0.13.1 — O(n) · RSS Δ 20.7 MiB · CPU 13.6 s

```text
  2 KiB █████████████████████ 475
  4 KiB ██████████████████████ 505
  8 KiB ███████████████████████ 517
 16 KiB ███████████████████████ 524
 32 KiB ███████████████████████ 526
 64 KiB ████████████████████ 461
128 KiB █████████████████████ 477
256 KiB ██████████████████████ 495
512 KiB ██████████████████████ 501
  1 MiB ███████████████████████ 519
  2 MiB ██████████████████████ 511
  4 MiB ██████████████████████ 497
  8 MiB █████████████████████ 488
 24 MiB ██████████████████████ 511
 32 MiB ██████████████████████ 492
```

### `console` v0.16.3 — O(n) · RSS Δ 1.6 MiB · CPU 12.8 s

```text
  2 KiB ████████ 197
  4 KiB █████████ 206
  8 KiB █████████ 212
 16 KiB █████████ 214
 32 KiB █████████ 221
 64 KiB █████████ 209
128 KiB █████████ 202
256 KiB █████████ 213
512 KiB █████████ 215
  1 MiB █████████ 217
  2 MiB █████████ 218
  4 MiB █████████ 211
  8 MiB █████████ 218
 24 MiB █████████ 217
 32 MiB █████████ 216
```

### `strip-ansi-escapes` v0.2.1 — O(n) · RSS Δ 13.1 MiB · CPU 14.9 s

```text
  2 KiB ██ 53
  4 KiB ██ 54
  8 KiB ██ 55
 16 KiB ██ 55
 32 KiB ██ 53
 64 KiB ██ 53
128 KiB ██ 52
256 KiB ██ 53
512 KiB ██ 53
  1 MiB ██ 54
  2 MiB ██ 51
  4 MiB ██ 52
  8 MiB ██ 53
 24 MiB ██ 54
 32 MiB ██ 55
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