# distill-strip-ansi

Strip ANSI escape sequences from byte streams — fast,
correct, and security-aware.

## What It Does

Removes terminal control sequences (colors, cursor
movement, hyperlinks, window titles) from text. Handles
the full ECMA-48 spec: CSI, OSC, DCS, APC/PM/SOS,
SS2/SS3, Fe, and CAN/SUB abort.

## Why distill-strip-ansi?

- 1-byte state machine, zero `unsafe`, `no_std` compatible
- SIMD-accelerated scanning via `memchr`
- Zero-alloc fast path on clean input (`Cow::Borrowed`)
- Streaming with 1 byte of cross-chunk state
- Selective filtering by sequence group, sub-kind,
  SGR color depth, and OSC type
- Security-aware: auto-detect caps at `sanitize`,
  echoback vectors stripped by default
- Drop-in replacements for `strip-ansi-escapes` and
  `fast-strip-ansi`

See [doc/ECOSYSTEM.md](doc/ECOSYSTEM.md) for a detailed
crate comparison.

## Install

```sh
cargo install distill-strip-ansi
```

## Quick Start

```sh
# Strip all ANSI from build output
cargo build --color=always 2>&1 | strip-ansi

# Save clean log
docker build . 2>&1 | strip-ansi > build.log

# Check for ANSI sequences (exit 1 if found)
strip-ansi --check < input.txt

# Scan for echoback attack vectors (exit 77 if found)
strip-ansi --check-threats < untrusted.log

# Strip threats + report to stderr
strip-ansi --check-threats --on-threat=strip < input.log
```

## Terminal Presets

Auto-detect picks the right preset for your output.
Piping to a file → `dumb` (strip all). Terminal output
→ `sanitize` (safe sequences preserved, echoback vectors
stripped).

| Preset     | Preserves                  | --unsafe |
| ---------- | -------------------------- | -------- |
| `dumb`     | nothing                    |          |
| `color`    | SGR (colors/styles)        |          |
| `vt100`    | + cursor, erase            |          |
| `tmux`     | + all CSI, Fe              |          |
| `sanitize` | + safe OSC (titles, links) |          |
| `xterm`    | + all OSC                  | required |
| `full`     | everything                 | required |

Aliases: `pipe`=dumb, `ci`/`pager`=color, `screen`=tmux,
`safe`=sanitize, `modern`=full.

```sh
strip-ansi --preset color           # keep colors only
strip-ansi --preset sanitize        # safe default
strip-ansi --preset xterm --unsafe  # pen-testing
strip-ansi --preset dumb            # force strip-all
```

See [doc/PRESETS.md](doc/PRESETS.md) for the full
reference.

## Security

The preset gradient is the security model. Auto-detect
never goes above `sanitize` — all known echoback vectors
(DECRQSS, OSC 50, OSC 52, CSI 21t, CSI 6n) are stripped
by default.

`--check-threats` scans for echoback vectors and reports
in structured key=value format for CI integration:

```text
[strip-ansi:threat] type=csi_21t line=12 pos=3 offset=142 len=5 cve=CVE-2003-0063 ref=https://nvd.nist.gov/vuln/detail/CVE-2003-0063
```

External threat databases extend the built-in patterns
without recompiling:

```sh
strip-ansi --check-threats --threat-db custom-threats.toml < input.log
```

See [doc/SECURITY.md](doc/SECURITY.md) for the threat
model and [doc/threat-db.toml](doc/threat-db.toml) for
the database format.

## CLI Reference

| Flag                 | Description                             |
| -------------------- | --------------------------------------- |
| `--check`            | Detect ANSI sequences (exit 1 if found) |
| `--check-threats`    | Scan for echoback vectors (exit 77)     |
| `--on-threat=MODE`   | `fail` (default) or `strip`             |
| `--no-threat-report` | Suppress stderr threat output           |
| `--threat-db=PATH`   | Load external threat database (TOML)    |
| `--preset NAME`      | Force a terminal preset                 |
| `--unsafe`           | Allow xterm/full presets                |
| `--no-strip-*`       | Preserve specific groups or sub-kinds   |
| `-n N` / `--head`    | Output first N lines only               |
| `-o` / `--output`    | Write to file instead of stdout         |
| `-c` / `--count`     | Print stripped byte count on stderr     |
| `--max-size`         | Stop reading after N bytes              |
| `-f` / `--follow`    | Keep reading after EOF                  |
| `--config PATH`      | Load TOML config file                   |

## Library Usage

See [doc/DESIGN.md](doc/DESIGN.md) for the architecture and
[doc/LIBRARY-USAGE.md](doc/LIBRARY-USAGE.md) for API examples.

```toml
[dependencies]
strip-ansi = { package = "distill-strip-ansi", version = "0.4", default-features = false, features = ["std"] }
```

For `no_std` (requires `alloc`):

```toml
[dependencies]
strip-ansi = { package = "distill-strip-ansi", version = "0.4", default-features = false }
```

## Feature Flags

| Feature             | Default | Description                         |
| ------------------- | ------- | ----------------------------------- |
| `std`               | yes     | StripWriter and I/O traits          |
| `cli`               | yes     | Builds the `strip-ansi` binary      |
| `filter`            | yes     | Selective sequence filtering        |
| `terminal-detect`   | yes     | Auto-detect terminal capabilities   |
| `transform`         | no      | Streaming SGR color rewriting       |
| `downgrade-color`   | no      | Color depth reduction algorithms    |
| `color-palette`     | no      | 3x3 matrix color transforms         |
| `unicode-normalize` | no      | Unicode homograph normalization     |
| `toml-config`       | no      | TOML config + threat database       |
| `distill-ansi-cli`  | no      | Full-featured `distill-ansi` binary |

## MSRV

Rust 1.85+ (edition 2024).

## Contributing

See [doc/CONTRIBUTING.md](doc/CONTRIBUTING.md).

## License

[Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at
your option.

SPDX: `MIT OR Apache-2.0`
