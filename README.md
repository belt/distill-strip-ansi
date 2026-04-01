# distill-strip-ansi

Strip ANSI escape sequences from byte streams.

Full ECMA-48 coverage: CSI, OSC, DCS, APC/PM/SOS, SS2/SS3, Fe sequences,
and CAN/SUB abort (§5.6). Zero `unsafe`, `no_std`-compatible, `memchr`
SIMD scanning with a 1-byte state machine.

Returns `Cow::Borrowed` on clean input (zero-alloc fast path).

## Why This Crate

|                  | this crate      | strip-ansi-escapes | fast-strip-ansi  |
| ---------------- | --------------- | ------------------ | ---------------- |
| Parser           | 1B ECMA-48 FSM  | `vte` (Alacritty)  | `vt-push-parser` |
| Clean input      | `Cow::Borrowed` | `Vec` (allocs)     | `Cow`            |
| Streaming        | 1B cross-chunk  | Writer adapter     | none             |
| `no_std`         | yes             | no                 | no               |
| Selective filter | group/sub-kind  | no                 | no               |
| CAN/SUB abort    | yes (§5.6)      | partial            | unknown          |
| Drop-in aliases  | both crates     | —                  | —                |

See [doc/ECOSYSTEM.md](doc/ECOSYSTEM.md) for detailed comparison.

## CLI

```sh
cargo install distill-strip-ansi
```

Pipe any command through `strip-ansi`:

```sh
cargo build --color=always 2>&1 | strip-ansi
docker build --progress=plain . 2>&1 | strip-ansi > build.log
```

Read from a file:

```sh
strip-ansi colored-output.log
```

Check for ANSI sequences (exit 1 if found):

```sh
strip-ansi --check < input.txt
```

Flags:

| Flag             | Description                             |
| ---------------- | --------------------------------------- |
| `--check`        | Detect ANSI sequences without stripping |
| `--no-strip-*`   | Preserve specific groups or sub-kinds   |
| `-n N`, `--head` | Output only the first N lines           |
| `-o`, `--output` | Write to file instead of stdout         |
| `-c`, `--count`  | Print stripped byte count on stderr     |
| `--max-size`     | Stop reading after N bytes of input     |
| `-f`, `--follow` | Keep reading after EOF (`tail -f`)      |

## Library

Add the dependency (library only, no CLI):

```toml
[dependencies.distill-strip-ansi]
version = "0.2"
default-features = false
features = ["std"]
```

Strip bytes or strings:

```rust
use strip_ansi::{strip, strip_str};

let clean = strip(b"\x1b[31mhello\x1b[0m");
assert_eq!(&*clean, b"hello");

let clean = strip_str("\x1b[1mbold\x1b[0m");
assert_eq!(&*clean, "bold");
```

Streaming (chunked input, 1 byte of cross-chunk
state):

```rust
use strip_ansi::StripStream;

let mut stream = StripStream::new();
for chunk in input_chunks {
    for slice in stream.strip_slices(chunk) {
        output.extend_from_slice(slice);
    }
}
```

Migrating from other crates:

```rust
// Drop-in for strip-ansi-escapes::strip()
use strip_ansi::strip_ansi_escapes;

// Drop-in for fast_strip_ansi::strip_ansi_bytes()
use strip_ansi::strip_ansi_bytes;
```

## Feature Flags

| Feature       | Default | Description                         |
| ------------- | ------- | ----------------------------------- |
| `std`         | yes     | `StripWriter` and I/O traits        |
| `cli`         | yes     | Builds the `strip-ansi` binary      |
| `filter`      | yes     | Per-group/sub-kind filtering        |
| `toml-config` | no      | TOML config file for filter rules   |

For `no_std` (requires `alloc`):

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
