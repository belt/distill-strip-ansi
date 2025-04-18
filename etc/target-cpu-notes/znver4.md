<!-- markdownlint-disable-file MD041 -->
<!--
Prose for `target-cpu = znver4` (AMD Zen 4 — Ryzen 7000,
EPYC 9004). Spliced into `doc/BENCHMARKS-REPRODUCE.md`.
-->

AMD Zen 4 (2022+, `znver4`) is the first AMD architecture with
full AVX-512 support (F, BW, CD, DQ, VL, VNNI, BF16,
IFMA, VPOPCNTDQ). Unlike Intel's split-register
implementation, Zen 4 runs AVX-512 at full width without
frequency throttling. This makes `x86-64-v4`-level
auto-vectorization safe for sustained workloads. For this
crate: palette matrix math and bulk byte transforms may
benefit from 512-bit lanes; the `memchr` hot path already
uses AVX2 at runtime and doesn't currently exploit
AVX-512.
