# Rust ANSI Stripping Ecosystem

## Crate Comparison

| Crate          | Parser    | Stream | no_std | Filter | Sec | Size   |
| -------------- | --------- | ------ | ------ | ------ | --- | ------ |
| `distill-*`    | 1B ECMA48 | 1B     | yes    | yes    | yes | ~100KB |
| `strip-ansi-*` | `vte`     | Writer | no     | no     | no  | ~225KB |
| `fast-strip-*` | `vt-push` | no     | no     | no     | no  | ~140KB |
| `console`      | regex     | no     | no     | no     | no  | ~500KB |

## What Each Crate Actually Does

### strip-ansi-escapes (~2.7M downloads/month)

The ecosystem incumbent. Wraps the `vte` crate (Alacritty's terminal
parser). Correct and battle-tested. Returns `Vec<u8>` always — no
`Cow::Borrowed` fast path for clean input. The `Writer` adapter
provides streaming but allocates internally.

Good choice when: you already depend on `vte`, or correctness is the
only concern and performance doesn't matter.

### fast-strip-ansi (~1.4K downloads/month)

Uses `vt-push-parser` (a push-based VT-100 state machine). Claims
fastest performance. Benchmarks show ~2x faster than `strip-ansi`
0.1.0 and ~25x faster than `strip-ansi-escapes` on an M3 MacBook Pro.
No streaming API. No `no_std` support.
Likely slower than `distill-strip-ansi` on an M3 and is much slower on author's
hardware.

Good choice when: you need raw single-buffer throughput and don't
need streaming or `no_std`.

### console (~27M total downloads)

General terminal abstraction crate. `strip_ansi_codes()` uses a
compiled regex internally. Pulls in `regex`, `unicode-width`, and
other dependencies. Not a stripping specialist.

Good choice when: you already use `console` for terminal styling
and need occasional stripping as a side feature.

### distill-strip-ansi (this crate)

Custom ECMA-48 state machine (15 states in 1 byte). `memchr` SIMD
scanning for ESC bytes. `Cow::Borrowed` on clean input (zero alloc).
Streaming with 1 byte of cross-chunk state. `no_std` compatible.
Selective filtering by sequence group/sub-kind, SGR color depth,
and OSC type. Security-aware: parameter-level classification
(SgrContent bitfield, OscType enum, CsiQuery sub-kind, DCS query
detection), `sanitize` preset as auto-detect ceiling, `--unsafe`
gate for dangerous presets, `--check-threats` echoback scanning
with structured output, and external threat database support.

Good choice when: you need streaming, `no_std`, selective filtering,
zero-alloc clean-input fast paths, or security-aware ANSI processing.

## Sequence Coverage

| Sequence        | this         | strip-ansi-* | fast-strip-* |
| --------------- | ------------ | ------------ | ------------ |
| CSI (`ESC [`)   | yes          | yes          | yes          |
| OSC (`ESC ]`)   | yes (BEL+ST) | yes          | yes          |
| Fe (single)     | yes          | yes          | yes          |
| DCS (`ESC P`)   | yes (param)  | yes          | yes          |
| SS2 (`ESC N`)   | yes          | yes          | yes          |
| SS3 (`ESC O`)   | yes          | yes          | yes          |
| APC (`ESC _`)   | yes          | yes          | yes          |
| PM (`ESC ^`)    | yes          | yes          | yes          |
| SOS (`ESC X`)   | yes          | yes          | yes          |
| EscIntermediate | yes          | yes          | unknown      |
| CAN/SUB abort   | yes (§5.6)   | partial      | unknown      |

All three specialist crates cover the core ECMA-48 sequences.
`distill-strip-ansi` explicitly handles CAN (0x18) and SUB (0x1A)
abort per ECMA-48 §5.6, which prevents sequence bodies from leaking
into output on malformed streams.

## Drop-In Migration

`distill-strip-ansi` provides API-compatible aliases:

```rust
// From strip-ansi-escapes:
use strip_ansi::strip_ansi_escapes;  // returns Vec<u8>

// From fast-strip-ansi:
use strip_ansi::strip_ansi_bytes;    // returns Cow<[u8]>
```

For new code, prefer `strip()` directly for `Cow` semantics.

## Security Comparison

| Feature              | this | strip-ansi-* | fast-strip-* |
| -------------------- | ---- | ------------ | ------------ |
| SGR depth classify   | yes  | no           | no           |
| OSC type classify    | yes  | no           | no           |
| CsiQuery separation  | yes  | no           | no           |
| DCS query detection  | yes  | no           | no           |
| Preset security      | yes  | no           | no           |
| Echoback detection   | yes  | no           | no           |
| Threat scanning      | yes  | no           | no           |
| External threat DB   | yes  | no           | no           |
| `--unsafe` gate      | yes  | n/a          | n/a          |

None of the other crates in this space address ANSI
security concerns. They strip everything or nothing —
no parameter-level inspection, no echoback awareness,
no threat detection.
