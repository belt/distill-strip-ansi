# distill-strip-ansi

Strip ANSI escape sequences from byte streams.

Covers the full ECMA-48 specification: CSI, OSC, DCS, APC/PM/SOS, SS2/SS3,
and Fe sequences. Zero `unsafe`, `no_std`-compatible, `memchr` SIMD scanning
with a 1-byte state machine. Returns `Cow::Borrowed` on clean input (zero-alloc
fast path).

## CLI

Install the binary:

```sh
cargo install distill-strip-ansi
```

Pipe any command through `strip-ansi` to remove escape sequences:

```sh
cargo build --color=always 2>&1 | strip-ansi
docker build --progress=plain . 2>&1 | strip-ansi > build.log
```

Read from a file:

```sh
strip-ansi colored-output.log
```

Check whether input contains ANSI sequences (exit 1 if found):

```sh
strip-ansi --check < input.txt
```

Flags:

| Flag                | Description                                    |
| ------------------- | ---------------------------------------------- |
| `--check`           | Detect ANSI sequences without stripping        |
| `--no-strip-*`      | Preserve specific ANSI groups or sub-kinds     |
| `-n N`, `--head`    | Output only the first N lines after stripping  |
| `-o`, `--output`    | Write to file instead of stdout                |
| `-c`, `--count`     | Print stripped byte count to stderr on exit    |
| `--max-size`        | Stop reading after N bytes of input            |
| `-f`, `--follow`    | Keep reading after EOF (like `tail -f`)        |

## Library

Add the dependency (library only, no CLI):

```toml
[dependencies]
distill-strip-ansi = { version = "0.2", default-features = false, features = ["std"] }
```

Strip bytes or strings:

```rust
use strip_ansi::{strip, strip_str};

// Byte slices — returns Cow<[u8]>
let clean = strip(b"\x1b[31mhello\x1b[0m");
assert_eq!(&*clean, b"hello");

// UTF-8 strings — returns Cow<str>
let clean = strip_str("\x1b[1mbold\x1b[0m");
assert_eq!(&*clean, "bold");
```

Streaming (chunked input):

```rust
use strip_ansi::StripStream;

let mut stream = StripStream::new();
for chunk in input_chunks {
    for slice in stream.strip_slices(chunk) {
        output.extend_from_slice(slice);
    }
}
```

## Feature Flags

| Feature  | Default | Description                          |
| -------- | ------- | ------------------------------------ |
| `std`    | yes     | Enables `StripWriter` and I/O traits |
| `cli`    | yes     | Builds the `strip-ansi` binary       |
| `filter` | yes     | Configurable per-group filtering     |

To use as a `no_std` library (requires `alloc`):

```toml
[dependencies]
distill-strip-ansi = { version = "0.2", default-features = false }
```

## MSRV

Rust 1.85+ (edition 2024).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE)
or [MIT License](LICENSE-MIT) at your option.

SPDX-License-Identifier: `MIT OR Apache-2.0`
