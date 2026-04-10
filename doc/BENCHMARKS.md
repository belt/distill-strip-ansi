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

- 541 MiB/s dirty throughput (4 KiB, ~20% ANSI)
- 3.3 GiB/s clean fast path (24 MiB)
- Zero allocation on clean input (`Cow::Borrowed`)
- O(n) linear scaling — constant MiB/s to 1 GiB+
- No temp files, no disk I/O — pure in-memory

## Environmental Concerns

<!-- BENCH:ENV:START -->

| Key        | Value                                    |
| ---------- | ---------------------------------------- |
| CPU        | Intel(R) Core(TM) i7-9750H CPU @ 2.60GHz |
| Arch       | x86_64                                   |
| OS         | macOS 26.4                               |
| Rust       | 1.94.1                                   |
| Date       | 2026-04-10                               |
| L1d        | 32.0K                                    |
| L2         | 256.0K                                   |
| L3         | 12.0 MiB                                 |
| RAM        | 32.0 GiB                                 |
| Sizes      | 15 tiers (hardware-adaptive)             |
| Bench time | 2m22s                                    |

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
| `distill-strip-ansi` | 1.5 MiB |   24 | 199.7 MiB | 20.2 MiB | 13.8 s |
| `fast-strip-ansi`    |     n/a |    3 | 243.6 MiB | 20.9 MiB | 12.9 s |
| `console`            |     n/a |    — | 174.7 MiB |   132.0K | 11.3 s |
| `strip-ansi-escapes` |     n/a |    2 | 185.2 MiB | 14.4 MiB | 14.8 s |

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
# Quick run: up to 2×L3 cache (~2m22s)
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
| `distill-strip-ansi` |  3.8 µs |   520 | baseline | 2.9 MiB | 12.2 s |
| `fast-strip-ansi`    |  4.0 µs |   490 |     1.1× | 1.9 MiB | 12.4 s |
| `console`            |  9.8 µs |   200 |     2.6× |  916.0K | 12.5 s |
| `strip-ansi-escapes` | 35.5 µs |    55 |     9.4× | 2.7 MiB | 12.3 s |
<!-- BENCH:ECO_DIRTY_2048:END -->

### Dirty 4 KiB

<!-- BENCH:ECO_DIRTY_4096:START -->

| Crate                |    Time | MiB/s |        × |   RSS Δ |    CPU |
| -------------------- | ------: | ----: | -------: | ------: | -----: |
| `distill-strip-ansi` |  7.2 µs |   541 | baseline | 2.8 MiB | 13.2 s |
| `fast-strip-ansi`    |  8.3 µs |   470 |     1.2× | 5.1 MiB | 12.6 s |
| `console`            | 19.4 µs |   201 |     2.7× | 2.9 MiB | 12.6 s |
| `strip-ansi-escapes` | 70.9 µs |    55 |     9.8× |  448.0K | 12.4 s |
<!-- BENCH:ECO_DIRTY_4096:END -->

### Dirty 32 KiB

<!-- BENCH:ECO_DIRTY_32768:START -->

| Crate                |     Time | MiB/s |        × |   RSS Δ |    CPU |
| -------------------- | -------: | ----: | -------: | ------: | -----: |
| `distill-strip-ansi` |  56.5 µs |   553 | baseline | 1.8 MiB | 13.1 s |
| `fast-strip-ansi`    |  61.2 µs |   510 |     1.1× |  836.0K | 12.6 s |
| `console`            | 147.8 µs |   211 |     2.6× |  652.0K | 12.1 s |
| `strip-ansi-escapes` | 595.2 µs |    53 |    10.5× |  680.0K | 12.3 s |
<!-- BENCH:ECO_DIRTY_32768:END -->

### Dirty 256 KiB

<!-- BENCH:ECO_DIRTY_262144:START -->

| Crate                |     Time | MiB/s |        × |   RSS Δ |    CPU |
| -------------------- | -------: | ----: | -------: | ------: | -----: |
| `distill-strip-ansi` | 451.9 µs |   553 | baseline | 2.0 MiB | 13.2 s |
| `fast-strip-ansi`    | 520.7 µs |   480 |     1.2× |   40.0K | 12.6 s |
| `console`            |   1.2 ms |   216 |     2.6× | 1.2 MiB | 12.4 s |
| `strip-ansi-escapes` |   4.7 ms |    53 |    10.5× |  664.0K | 12.3 s |
<!-- BENCH:ECO_DIRTY_262144:END -->

### Dirty 24 MiB

<!-- BENCH:ECO_DIRTY_25165824:START -->

| Crate                |     Time | MiB/s |        × |    RSS Δ |    CPU |
| -------------------- | -------: | ----: | -------: | -------: | -----: |
| `distill-strip-ansi` |  44.9 ms |   535 | baseline | 15.2 MiB | 14.7 s |
| `fast-strip-ansi`    |  47.5 ms |   505 |     1.1× | 32.4 MiB | 15.1 s |
| `console`            | 111.7 ms |   215 |     2.5× | 16.1 MiB | 12.4 s |
| `strip-ansi-escapes` | 460.9 ms |    52 |    10.3× | 18.0 MiB | 13.2 s |
<!-- BENCH:ECO_DIRTY_25165824:END -->

### Dirty 32 MiB

<!-- BENCH:ECO_DIRTY_33554432:START -->

| Crate                |     Time | MiB/s |        × |    RSS Δ |    CPU |
| -------------------- | -------: | ----: | -------: | -------: | -----: |
| `distill-strip-ansi` |  64.1 ms |   499 | baseline | 20.2 MiB | 13.8 s |
| `fast-strip-ansi`    |  65.3 ms |   490 |    ~1.0× | 20.9 MiB | 12.9 s |
| `console`            | 148.6 ms |   215 |     2.3× |   132.0K | 11.3 s |
| `strip-ansi-escapes` | 576.3 ms |    56 |     9.0× | 14.4 MiB | 14.8 s |
<!-- BENCH:ECO_DIRTY_33554432:END -->

### Cargo Output (5 KiB)

<!-- BENCH:ECO_CARGO:START -->

| Crate                |     Time | MiB/s |        × |   RSS Δ |    CPU |
| -------------------- | -------: | ----: | -------: | ------: | -----: |
| `distill-strip-ansi` | 146.8 ns | 36377 | baseline |  792.0K | 12.4 s |
| `fast-strip-ansi`    |   2.9 µs |  1814 |    20.1× | 1.2 MiB | 12.6 s |
| `console`            |  15.6 µs |   343 |   106.0× |  640.0K | 12.2 s |
| `strip-ansi-escapes` | 132.5 µs |    40 |   902.6× | 1.4 MiB | 12.0 s |
<!-- BENCH:ECO_CARGO:END -->

### OSC 8 Hyperlinks (4 KiB)

<!-- BENCH:ECO_OSC8:START -->

| Crate                |     Time | MiB/s |        × |   RSS Δ |    CPU |
| -------------------- | -------: | ----: | -------: | ------: | -----: |
| `distill-strip-ansi` | 128.5 ns | 31961 | baseline |  928.0K | 12.4 s |
| `fast-strip-ansi`    |   2.3 µs |  1778 |    18.0× |  988.0K | 12.1 s |
| `console`            |  12.0 µs |   343 |    93.3× | 2.3 MiB | 12.7 s |
| `strip-ansi-escapes` | 103.6 µs |    40 |   806.0× |  784.0K | 12.8 s |
<!-- BENCH:ECO_OSC8:END -->

### Extended Capabilities

Additional features available in `distill-strip-ansi`.

| Feature                   |     Time | MiB/s |   RSS Δ |    CPU |
| ------------------------- | -------: | ----: | ------: | -----: |
| Classify (parse only)     |  13.0 µs |   324 | 1.1 MiB | 13.1 s |
| Classify + detail         |  13.1 µs |   321 |   16.0K | 13.2 s |
| Filter: SGR mask          |  16.5 µs |   254 |  228.0K | 12.7 s |
| Filter: sanitize preset   |  16.4 µs |   256 | 1.1 MiB | 12.6 s |
| Threat scan (clean)       |  13.0 µs |   323 |  760.0K | 13.2 s |
| Threat scan (dirty)       |  13.8 µs |   305 |  128.0K | 13.2 s |
| Streaming (L1)            |  46.3 µs |   675 | 1.0 MiB | 13.1 s |
| Streaming (L2)            | 366.0 µs |   683 |  428.0K | 13.0 s |
| Streaming (L3)            |  23.2 ms |   518 | 8.8 MiB | 13.3 s |
| Unicode normalize         |  25.3 µs |   128 |  104.0K | 13.0 s |
| Transform: passthrough    | 114.7 ns | 36581 |  792.0K | 13.3 s |
| Transform: truecolor→mono |  26.1 µs |   178 |  316.0K | 13.1 s |
| Transform: truecolor→grey |  27.7 µs |   168 |    8.0K | 13.2 s |
| Transform: truecolor→16   |  28.8 µs |   162 |    4.0K | 13.3 s |
| Transform: truecolor→256  |  31.3 µs |   149 |    4.0K | 12.3 s |
| Transform: 256→16         |  21.3 µs |   183 |  664.0K | 12.7 s |
| Transform: 256→grey       |  22.3 µs |   175 | 1.7 MiB | 12.9 s |
| Transform: basic→mono     |  28.9 µs |   145 |  424.0K | 13.2 s |
| Augment: protanopia       |   3.4 µs |   218 |  320.0K | 13.1 s |
| Augment: deuteranopia     |   3.3 µs |   224 | 1.0 MiB | 13.0 s |
| Augment: sRGB roundtrip   | 735.9 ns |   332 |    4.0K | 12.8 s |

## Scaling

Dirty throughput (MiB/s) across input sizes.
Constant bar length = O(n). Shrinking = super-linear.

RSS Δ and CPU shown at largest size only — small-size
values are dominated by benchmark harness overhead.
### `distill-strip-ansi` v0.5.0 — O(n) · RSS Δ 20.2 MiB · CPU 13.8 s

```text
  2 KiB ████████████████████████████ 520
  4 KiB █████████████████████████████ 541
  8 KiB █████████████████████████████ 554
 16 KiB ██████████████████████████████ 555
 32 KiB █████████████████████████████ 553
 64 KiB █████████████████████████████ 542
128 KiB █████████████████████████████ 554
256 KiB █████████████████████████████ 553
512 KiB █████████████████████████████ 540
  1 MiB ███████████████████████████ 505
  2 MiB ███████████████████████████ 509
  4 MiB ███████████████████████████ 505
  8 MiB ████████████████████████████ 528
 24 MiB ████████████████████████████ 535
 32 MiB ███████████████████████████ 499
```

### `fast-strip-ansi` v0.13.1 — O(n) · RSS Δ 20.9 MiB · CPU 12.9 s

```text
  2 KiB ██████████████████████████ 490
  4 KiB █████████████████████████ 470
  8 KiB ██████████████████████████ 486
 16 KiB ███████████████████████████ 507
 32 KiB ███████████████████████████ 510
 64 KiB ██████████████████████ 418
128 KiB █████████████████████████ 473
256 KiB █████████████████████████ 480
512 KiB ███████████████████████████ 499
  1 MiB ██████████████████████████ 486
  2 MiB ████████████████████████ 450
  4 MiB ████████████████████████ 448
  8 MiB ██████████████████████████ 484
 24 MiB ███████████████████████████ 505
 32 MiB ██████████████████████████ 490
```

### `console` v0.16.3 — O(n) · RSS Δ 132.0K · CPU 11.3 s

```text
  2 KiB ██████████ 200
  4 KiB ██████████ 201
  8 KiB ███████████ 211
 16 KiB ███████████ 214
 32 KiB ███████████ 211
 64 KiB ███████████ 214
128 KiB ███████████ 206
256 KiB ███████████ 216
512 KiB ████████████ 222
  1 MiB ███████████ 203
  2 MiB ███████████ 220
  4 MiB ███████████ 220
  8 MiB ███████████ 213
 24 MiB ███████████ 215
 32 MiB ███████████ 215
```

### `strip-ansi-escapes` v0.2.1 — O(n) · RSS Δ 14.4 MiB · CPU 14.8 s

```text
  2 KiB ██ 55
  4 KiB ██ 55
  8 KiB ███ 56
 16 KiB ██ 54
 32 KiB ██ 53
 64 KiB ██ 54
128 KiB ██ 53
256 KiB ██ 53
512 KiB ██ 54
  1 MiB ██ 53
  2 MiB ██ 52
  4 MiB ███ 56
  8 MiB ██ 52
 24 MiB ██ 52
 32 MiB ███ 56
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