<!-- markdownlint-disable-file MD041 -->
<!--
Prose for `target-cpu = alderlake` (Intel 12th gen, hybrid
P+E cores). Spliced into `doc/BENCHMARKS-REPRODUCE.md`.
-->

Intel Alder Lake (2021+) is a hybrid architecture: Performance
cores (Golden Cove) support AVX-512 in hardware but it's
disabled in firmware when Efficiency cores (Gracemont,
x86-64-v3 only) are active. LLVM's `alderlake` target
conservatively emits only AVX2/FMA — equivalent to
`x86-64-v3` for codegen purposes. Benchmark numbers on
Alder Lake can vary depending on which core type the OS
schedules the bench thread onto. Pin with `taskset` to a
P-core for consistent results.
