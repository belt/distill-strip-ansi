<!-- markdownlint-disable-file MD041 -->
<!--
Prose for `target-cpu = sapphirerapids` (Intel 4th gen Xeon
Scalable, 2023). Spliced into `doc/BENCHMARKS-REPRODUCE.md`.
-->

Sapphire Rapids (2023+) is Intel's server-class AVX-512 platform
with AMX (Advanced Matrix Extensions), AVX-512 BF16, and
AVX-512 FP16. All cores are homogeneous (no E-cores), so
AVX-512 runs without frequency throttling concerns. For
this crate: the hot path is `memchr` byte scanning (AVX2
at runtime) and table lookups — AVX-512 gains are marginal.
Palette matrix math could theoretically benefit from wider
FMA, but the 3×3 matrix is too small to amortize the
AVX-512 transition penalty. Use this target-cpu to document
server-class benchmark environments.
