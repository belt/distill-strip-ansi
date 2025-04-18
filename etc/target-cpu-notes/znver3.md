<!-- markdownlint-disable-file MD041 -->
<!--
Prose for `target-cpu = znver3` (AMD Zen 3 — Ryzen 5000,
EPYC 7003). Spliced into `doc/BENCHMARKS-REPRODUCE.md`.
-->

AMD Zen 3 (2020+, `znver3`) supports the full x86-64-v3 feature
set (AVX2, FMA, BMI1/2) plus additional AMD-specific
extensions: VAES, VPCLMULQDQ, and improved branch
prediction. Setting `znver3` over `x86-64-v3` lets LLVM
use Zen 3's scheduling model for better instruction
ordering — measurable on tight loops but unlikely to move
the needle for this crate's byte-scanning hot path. Use
when benchmarking on Zen 3 hardware to document the exact
target.
