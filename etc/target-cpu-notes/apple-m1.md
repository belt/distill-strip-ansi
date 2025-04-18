<!-- markdownlint-disable-file MD041 -->
<!--
Prose for `target-cpu = apple-m1` (Apple Silicon, first gen).
Also covers M1 Pro/Max/Ultra. Spliced into
`doc/BENCHMARKS-REPRODUCE.md` by the generator.
-->

Apple M1 (2020+) is AArch64 with NEON (128-bit SIMD, always
available), plus Apple-specific extensions (AMX for matrix,
not exposed to userspace compilers). The default AArch64
codegen already uses NEON — setting `apple-m1` additionally
enables `sha2`, `aes`, `crc`, `dotprod`, `fp16`, and
`fullfp16` features. For this crate the gain over default
AArch64 is negligible: the hot path is byte-scanning
(`memchr` handles NEON internally) and table lookups. The
palette FMA path doesn't exist on AArch64 (uses `fmla`
natively). Pinning to `apple-m1` is mostly documentation
of intent.
