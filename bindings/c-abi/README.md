# C ABI

`distill-strip-ansi-c` builds `libdistill_strip_ansi_c` as a `cdylib` and
`staticlib`. The public header is `include/distill_strip_ansi.h`. Callers own
input/output storage; output must have at least the input length because
stripping never increases the byte count. The ABI is byte-oriented and reports
the number of output bytes, so embedded NULs and non-UTF-8 input are supported.

Build from the repository root:

```sh
cargo build --release -p distill-strip-ansi-c
```

The shared library is written to `target/release`. Functions are prefixed
`dsa_`; `dsa_abi_version()` currently returns `1`. See the header for pointer
validity and error-code requirements.

Compile the standalone header/ABI smoke test with:

```sh
cc bindings/c-abi/tests/header_smoke.c -Ibindings/c-abi/include \
	-Ltarget/release -Wl,-rpath,"$PWD/target/release" \
	-ldistill_strip_ansi_c -o /tmp/distill-strip-ansi-c-smoke
/tmp/distill-strip-ansi-c-smoke
```