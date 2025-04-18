<!-- markdownlint-disable-file MD041 -->
<!--
Prose for `target-cpu = apple-m3` (Apple Silicon, 3rd gen).
Spliced into `doc/BENCHMARKS-REPRODUCE.md` by the generator.
-->

Apple M3 (2023+) adds over M1: wider decode, deeper OoO buffers,
and hardware-level pointer authentication (PAC). From a
codegen perspective the feature set is the same as M1 for
this crate — NEON is baseline, `memchr` handles SIMD
internally, and the palette path uses scalar `fmla`. The
M3's microarchitectural improvements (branch prediction,
cache bandwidth) show up in wallclock benchmarks without
any compiler flag changes. Use this target-cpu value to
document that benchmarks were collected on M3 hardware.
