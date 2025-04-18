<!-- markdownlint-disable-file MD041 -->
<!--
Fallback prose spliced in when no
`etc/target-cpu-notes/<target-cpu>.md` file matches the
detected value. Add a file for a new target to swap this
generic note for something specific. Filename matches the
exact `-C target-cpu=…` value from `.cargo/config.toml`.
-->

No per-target notes on file for this `target-cpu` value. The
rustc baseline for the host triple is used — runtime SIMD
detection in `memchr` still applies. Drop a file at
`etc/target-cpu-notes/<target-cpu>.md` to replace this
generic paragraph with architecture-specific context.
