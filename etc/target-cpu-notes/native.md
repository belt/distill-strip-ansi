<!-- markdownlint-disable-file MD041 -->
<!--
Prose for `target-cpu = native` (auto-detect host).
Spliced into `doc/BENCHMARKS-REPRODUCE.md` by the generator.
-->

`native` tells LLVM to detect the host CPU and enable all
features it supports. Produces the fastest binary for *this
specific machine* but is not portable — the binary may use
instructions unavailable on other hardware. Appropriate for
local benchmarking and PGO training runs. Do not distribute
binaries built with `native`; use an explicit level
(`x86-64-v3`, `apple-m1`, etc.) for release artifacts.
