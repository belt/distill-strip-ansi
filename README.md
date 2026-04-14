# distill-strip-ansi

Strip ANSI escape sequences from byte streams — fast,
correct, and security-aware.

## What It Does

Removes terminal control sequences (colors, cursor
movement, hyperlinks, window titles) from text. Handles
the full ECMA-48 spec: CSI, OSC, DCS, APC/PM/SOS,
SS2/SS3, Fe, and CAN/SUB abort.

## Why distill-strip-ansi?

- zero `unsafe`, `no_std`, fast
- SIMD-accelerated scanning via `memchr`
- Zero-alloc fast path on clean input (`Cow::Borrowed`)
- Streaming with 1 byte of cross-chunk state
- Selective filtering by sequence group, sub-kind,
  SGR color depth, and OSC type
- Color transforms: truecolor → 256 → 16 → greyscale → mono
  - tritanopia simulation for dichromats
  - extended-gamut for tetrachromats
  - legacy terminal support (without the garbage)
  - e-ink displays (without the garbage)
- Unicode homograph normalization (fullwidth, math bold,
  circled letters, superscripts — security hardening)
- Security-aware: auto-detect caps at `sanitize`,
  echoback vectors stripped by default, threat scanning
  with external database support
- Drop-in replacements for `strip-ansi-escapes` and
  `fast-strip-ansi`

See [doc/ECOSYSTEM.md](doc/ECOSYSTEM.md) for a detailed
crate comparison.

## Transform Reference

Color transforms and Unicode normalization live in the
`distill-ansi` binary. Architecture and algorithms:

- [doc/COLOR-TRANSFORMS.md](doc/COLOR-TRANSFORMS.md) —
  depth reduction, palette remapping, CVD simulation
- [doc/UNICODE-NORMALIZATION.md](doc/UNICODE-NORMALIZATION.md) —
  homograph defense, CJK canonicalization, TOML mappings
- [doc/ANSI-REFERENCE.md](doc/ANSI-REFERENCE.md) —
  ECMA-48 sequence taxonomy (SGR, OSC, CSI, DCS)

## Performance

1.1× faster than `fast-strip-ansi`, 2.4× faster than `console`,
7.4× faster than `strip-ansi-escapes` on authors hardware.
Clean input returns `Cow::Borrowed` pure `memchr` SIMD scan, zero allocation.
O(n) linear scaling across all input sizes.

How:

1. Understanding silicon-software boundaries
2. Approach, discipline, and slop
3. No regex — `console` pays regex compilation cost
4. No `vte` — `strip-ansi-escapes` runs Alacritty's
   full terminal emulator state machine (as of 10 April 2026)

See [doc/BENCHMARKS.md](doc/BENCHMARKS.md) for full
Criterion results and reproduction steps.

## Install the easy way

### Homebrew (macOS / Linux)

```sh
brew install belt/distill/distill-strip-ansi
```

See the [Homebrew tap](https://github.com/belt/homebrew-distill)
for Brewfile usage and bottle details.

### Cargo (any platform)

```sh
cargo install distill-strip-ansi
```

[crates.io](https://crates.io/crates/distill-strip-ansi)

## Short Short Version

```sh
# Strip all ANSI from cargo-build output
cargo build --color=always 2>&1 | strip-ansi

# Save clean docker build log
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
strip-ansi = { package = "distill-strip-ansi", version = "0.5", default-features = false, features = ["std"] }
```

For `no_std` (requires `alloc`):

```toml
[dependencies]
strip-ansi = { package = "distill-strip-ansi", version = "0.5", default-features = false }
```

## Feature Flags

All processing features are enabled by default. Disable
with `default-features = false` and pick what you need.
`toml-config` and `distill-ansi-cli` are opt-in.
Listed in pipeline order.

| Feature             | Def     | Description                    |
| ------------------- | ------- | ------------------------------ |
| `std`               | yes     | StripWriter, I/O traits        |
| `filter`            | yes     | Selective group/kind filtering |
| `transform`         | yes     | SGR color rewriting engine     |
| `downgrade-color`   | yes     | Truecolor→256→16→grey→mono     |
| `augment-color`     | yes     | Color vision simulation        |
| `unicode-normalize` | yes     | Homograph normalization        |
| `cli`               | yes     | `strip-ansi` binary            |
| `terminal-detect`   | via cli | Auto-detect color/hyperlinks   |
| `toml-config`       | no      | External threat DB + config    |
| `distill-ansi-cli`  | no      | Full `distill-ansi` binary     |

Built-in threat detection (`--check-threats`) works without
`toml-config`. The `toml-config` feature adds `--threat-db`
for loading additional patterns from file — useful when new
attack vectors emerge before a crate release.

## MSRV

Rust 1.85+ (edition 2024).

## Coming Soon

- Criterion 0.7 → 0.8 (MSRV 1.86, breaking API)
- MSRV 1.86
- Better ops

## Contributing

See [doc/CONTRIBUTING.md](doc/CONTRIBUTING.md).

## License

[Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at
your option.

SPDX: `MIT OR Apache-2.0`

## Copyright and Credits

Initial slop by: Paul Belt <paul.belt@users.noreply.github.com>
Word-smith: Paul Belt <paul.belt@users.noreply.github.com>
Architect: Paul Belt <paul.belt@users.noreply.github.com>
Lead: Paul Belt <paul.belt@users.noreply.github.com>
SRE: Paul Belt <paul.belt@users.noreply.github.com>
Ops: Paul Belt <paul.belt@users.noreply.github.com>
Marketing: Paul Belt <paul.belt@users.noreply.github.com>
Finance: Paul Belt <paul.belt@users.noreply.github.com>
Sanity: V/R/J, Gaa, Sherman, Luke,, y'all know who you are. Thank you!

