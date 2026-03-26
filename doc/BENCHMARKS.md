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

- 528 MiB/s dirty throughput (4 KiB, ~20% ANSI)
- 3.1 GiB/s clean fast path (24 MiB)
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
| Date       | 2026-04-09                               |
| L1d        | 32.0K                                    |
| L2         | 256.0K                                   |
| L3         | 12.0 MiB                                 |
| RAM        | 32.0 GiB                                 |
| Sizes      | 15 tiers (hardware-adaptive)             |
| Bench time | 2m35s                                    |

<!-- BENCH:ENV:END -->

### Crate Versions

<!-- BENCH:VERSIONS:START -->

| Crate                | Version |
| -------------------- | ------: |
| `distill-strip-ansi` |   0.5.0 |
| `fast-strip-ansi`    |  0.13.1 |
| `console`            |  0.16.3 |
| `strip-ansi-escapes` |   0.2.1 |
| `criterion`          |   0.7.0 |

<!-- BENCH:VERSIONS:END -->

## Crate Footprints

<!-- BENCH:FOOTPRINT:START -->

| Crate                |  Binary | Deps |  Peak RSS |    RSS Δ |    CPU |
| -------------------- | ------: | ---: | --------: | -------: | -----: |
| `distill-strip-ansi` | 1.5 MiB |   24 | 192.7 MiB | 20.5 MiB | 13.3 s |
| `fast-strip-ansi`    |     n/a |    3 | 232.2 MiB | 19.8 MiB | 13.8 s |
| `console`            |     n/a |    — | 197.5 MiB |  2.1 MiB | 12.6 s |
| `strip-ansi-escapes` |     n/a |    2 | 192.9 MiB |  8.9 MiB | 16.7 s |

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
# Quick run: up to 2×L3 cache (~2m35s)
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

| Crate                |    Time |     MiB/s | Relative |
| -------------------- | ------: | --------: | -------: |
| `distill-strip-ansi` |  3.8 µs | 508 MiB/s | baseline |
| `fast-strip-ansi`    |  4.1 µs | 480 MiB/s |     1.1× |
| `console`            | 11.8 µs | 165 MiB/s |     3.1× |
| `strip-ansi-escapes` | 42.4 µs |  46 MiB/s |    11.0× |
<!-- BENCH:ECO_DIRTY_2048:END -->

### Dirty 4 KiB

<!-- BENCH:ECO_DIRTY_4096:START -->

| Crate                |    Time |     MiB/s | Relative |
| -------------------- | ------: | --------: | -------: |
| `distill-strip-ansi` |  7.4 µs | 528 MiB/s | baseline |
| `fast-strip-ansi`    |  8.1 µs | 484 MiB/s |     1.1× |
| `console`            | 22.4 µs | 174 MiB/s |     3.0× |
| `strip-ansi-escapes` | 86.6 µs |  45 MiB/s |    11.7× |
<!-- BENCH:ECO_DIRTY_4096:END -->

### Dirty 32 KiB

<!-- BENCH:ECO_DIRTY_32768:START -->

| Crate                |     Time |     MiB/s | Relative |
| -------------------- | -------: | --------: | -------: |
| `distill-strip-ansi` |  57.8 µs | 540 MiB/s | baseline |
| `fast-strip-ansi`    |  62.3 µs | 502 MiB/s |     1.1× |
| `console`            | 168.6 µs | 185 MiB/s |     2.9× |
| `strip-ansi-escapes` | 684.3 µs |  46 MiB/s |    11.8× |
<!-- BENCH:ECO_DIRTY_32768:END -->

### Dirty 256 KiB

<!-- BENCH:ECO_DIRTY_262144:START -->

| Crate                |     Time |     MiB/s | Relative |
| -------------------- | -------: | --------: | -------: |
| `distill-strip-ansi` | 468.0 µs | 534 MiB/s | baseline |
| `fast-strip-ansi`    | 559.2 µs | 447 MiB/s |     1.2× |
| `console`            |   1.4 ms | 178 MiB/s |     3.0× |
| `strip-ansi-escapes` |   5.5 ms |  45 MiB/s |    11.8× |
<!-- BENCH:ECO_DIRTY_262144:END -->

### Dirty 24 MiB

<!-- BENCH:ECO_DIRTY_25165824:START -->

| Crate                |     Time |     MiB/s | Relative |
| -------------------- | -------: | --------: | -------: |
| `distill-strip-ansi` |  45.3 ms | 530 MiB/s | baseline |
| `fast-strip-ansi`    |  54.8 ms | 438 MiB/s |     1.2× |
| `console`            | 134.7 ms | 178 MiB/s |     3.0× |
| `strip-ansi-escapes` | 528.4 ms |  45 MiB/s |    11.7× |
<!-- BENCH:ECO_DIRTY_25165824:END -->

### Dirty 32 MiB

<!-- BENCH:ECO_DIRTY_33554432:START -->

| Crate                |     Time |     MiB/s | Relative |
| -------------------- | -------: | --------: | -------: |
| `distill-strip-ansi` |  67.5 ms | 474 MiB/s | baseline |
| `fast-strip-ansi`    |  75.6 ms | 423 MiB/s |     1.1× |
| `console`            | 180.5 ms | 177 MiB/s |     2.7× |
| `strip-ansi-escapes` | 680.0 ms |  47 MiB/s |    10.1× |
<!-- BENCH:ECO_DIRTY_33554432:END -->

### Cargo Output (5 KiB)

<!-- BENCH:ECO_CARGO:START -->

| Crate                |     Time |       MiB/s | Relative |
| -------------------- | -------: | ----------: | -------: |
| `distill-strip-ansi` | 144.6 ns | 36925 MiB/s | baseline |
| `fast-strip-ansi`    |   3.5 µs |  1525 MiB/s |    24.2× |
| `console`            |  17.9 µs |   298 MiB/s |   124.1× |
| `strip-ansi-escapes` | 150.8 µs |    35 MiB/s |  1042.5× |
<!-- BENCH:ECO_CARGO:END -->

### OSC 8 Hyperlinks (4 KiB)

<!-- BENCH:ECO_OSC8:START -->

| Crate                |     Time |       MiB/s | Relative |
| -------------------- | -------: | ----------: | -------: |
| `distill-strip-ansi` | 130.3 ns | 31530 MiB/s | baseline |
| `fast-strip-ansi`    |   2.7 µs |  1509 MiB/s |    20.9× |
| `console`            |  14.0 µs |   294 MiB/s |   107.2× |
| `strip-ansi-escapes` | 116.3 µs |    35 MiB/s |   892.2× |
<!-- BENCH:ECO_OSC8:END -->

### Extended Capabilities

Additional features available in `distill-strip-ansi`.

| Feature                   |     Time |       MiB/s | Others |
| ------------------------- | -------: | ----------: | ------ |
| Classify (parse only)     |  12.0 µs |   349 MiB/s | n/a    |
| Classify + detail         |  13.5 µs |   312 MiB/s | n/a    |
| Filter: SGR mask          |  15.3 µs |   275 MiB/s | n/a    |
| Filter: sanitize preset   |  16.3 µs |   258 MiB/s | n/a    |
| Threat scan (clean)       |  12.5 µs |   337 MiB/s | n/a    |
| Threat scan (dirty)       |  12.8 µs |   330 MiB/s | n/a    |
| Streaming (L1)            |  46.3 µs |   675 MiB/s | n/a    |
| Streaming (L2)            | 372.9 µs |   670 MiB/s | n/a    |
| Streaming (L3)            |  18.2 ms |   659 MiB/s | n/a    |
| Unicode normalize         |  26.1 µs |   124 MiB/s | n/a    |
| Transform: passthrough    | 118.1 ns | 35521 MiB/s | n/a    |
| Transform: truecolor→mono |  27.3 µs |   171 MiB/s | n/a    |
| Transform: truecolor→grey |  28.8 µs |   162 MiB/s | n/a    |
| Transform: truecolor→16   |  28.8 µs |   161 MiB/s | n/a    |
| Transform: truecolor→256  |  28.7 µs |   162 MiB/s | n/a    |
| Transform: 256→16         |  21.6 µs |   180 MiB/s | n/a    |
| Transform: 256→grey       |  21.9 µs |   178 MiB/s | n/a    |
| Transform: basic→mono     |  30.0 µs |   140 MiB/s | n/a    |
| Augment: protanopia       |   3.6 µs |   206 MiB/s | n/a    |
| Augment: deuteranopia     |   4.0 µs |   183 MiB/s | n/a    |
| Augment: sRGB roundtrip   | 810.7 ns |   301 MiB/s | n/a    |

## Scaling

Dirty throughput (MiB/s) across input sizes.
Constant bar length = O(n). Shrinking = super-linear.

RSS Δ and CPU shown at largest size only — small-size
values are dominated by benchmark harness overhead.
### `distill-strip-ansi` v0.5.0 — O(n) · RSS Δ 20.5 MiB · CPU 13.3 s

```text
  2 KiB ███████████████████████████ 508
  4 KiB ████████████████████████████ 528
  8 KiB ████████████████████████████ 522
 16 KiB █████████████████████████████ 535
 32 KiB █████████████████████████████ 540
 64 KiB █████████████████████████████ 533
128 KiB █████████████████████████████ 535
256 KiB █████████████████████████████ 534
512 KiB █████████████████████████████ 536
  1 MiB ██████████████████████████████ 549
  2 MiB ██████████████████████████ 484
  4 MiB █████████████████████████ 471
  8 MiB █████████████████████████ 467
 24 MiB █████████████████████████████ 530
 32 MiB █████████████████████████ 474
```

### `fast-strip-ansi` v0.13.1 — O(n) · RSS Δ 19.8 MiB · CPU 13.8 s

```text
  2 KiB ██████████████████████████ 480
  4 KiB ██████████████████████████ 484
  8 KiB ██████████████████████████ 484
 16 KiB ███████████████████████████ 498
 32 KiB ███████████████████████████ 502
 64 KiB ███████████████████████ 435
128 KiB █████████████████████████ 473
256 KiB ████████████████████████ 447
512 KiB █████████████████████████ 461
  1 MiB █████████████████████████ 459
  2 MiB ████████████████████████ 456
  4 MiB ███████████████████████ 422
  8 MiB ██████████████████████ 405
 24 MiB ███████████████████████ 438
 32 MiB ███████████████████████ 423
```

### `console` v0.16.3 — O(n) · RSS Δ 2.1 MiB · CPU 12.6 s

```text
  2 KiB █████████ 165
  4 KiB █████████ 174
  8 KiB █████████ 179
 16 KiB ██████████ 183
 32 KiB ██████████ 185
 64 KiB ██████████ 186
128 KiB █████████ 173
256 KiB █████████ 178
512 KiB █████████ 182
  1 MiB ██████████ 183
  2 MiB █████████ 182
  4 MiB █████████ 182
  8 MiB █████████ 181
 24 MiB █████████ 178
 32 MiB █████████ 177
```

### `strip-ansi-escapes` v0.2.1 — O(n) · RSS Δ 8.9 MiB · CPU 16.7 s

```text
  2 KiB ██ 46
  4 KiB ██ 45
  8 KiB ██ 46
 16 KiB ██ 46
 32 KiB ██ 46
 64 KiB ██ 46
128 KiB ██ 45
256 KiB ██ 45
512 KiB ██ 46
  1 MiB ██ 46
  2 MiB ██ 46
  4 MiB ██ 46
  8 MiB ██ 46
 24 MiB ██ 45
 32 MiB ██ 47
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