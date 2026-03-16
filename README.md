# distill-strip-ansi

## Why?

Stripping is a classification problem, not an interpretation problem.

## Competitive Landscape

This workspace uses `distill-strip-ansi` (crate `strip-ansi/`).
This file documents why, and when alternatives are appropriate.

`distill-strip-ansi` — Hand-coded match + memchr.
Full ECMA-48. Cow alloc, `no_std`, 1-byte parser, streaming.

`strip-ansi-escapes` — `vte` wrapper.
Full ECMA-48. Always Vec, no `no_std`, ~1KB parser, no streaming.

`console` — Regex.
CSI + partial OSC. Cow (str), no `no_std`, regex obj, no streaming.

`vte` — State machine (Perform trait).
Full VT. OSC buf, optional `no_std`, ~1KB+ parser, streaming.

`anstyle-parse` — 4KB lookup table.
Full VT. OSC buf, optional `no_std`, ~4KB parser, streaming.

`anstream` — anstyle-parse Write adapter.
Full VT. Write sink, no `no_std`, ~4KB parser, streaming.

`cansi` — Linear CSI scan.
CSI only. Vec (str), no `no_std`, no streaming.

## When to Use What

`distill-strip-ansi` — high-throughput log stripping, binary
`&[u8]` input, streaming chunks, `no_std`/WASM targets, when
you need Cow zero-alloc on clean input + full ECMA-48 coverage.

`strip-ansi-escapes` — drop-in correctness when you don't
control the parser and always-alloc is acceptable. Uses `vte`
internally. Good default for apps that strip occasionally.

`console` — already in your dep tree for terminal width/style
and you only handle UTF-8 strings with simple CSI sequences.
Regex-based; misses DCS, incomplete OSC. Not idempotent on
arbitrary bytes.

`vte` / `anstyle-parse` — building a terminal emulator or
need to interpret sequence content (params, intermediates,
OSC payloads). Overkill for stripping; the Perform trait
machinery and param buffers add ~1KB+ per parser instance.

`anstream` — Write adapter that auto-strips based on terminal
capability. Use when wrapping `io::Write` for colored CLI
output, not for processing external log streams.

`cansi` — CSI-only. Misses OSC 8 hyperlinks, DCS, APC, SS2/SS3.
Insufficient for real-world build output.

## distill-strip-ansi Design

15-state ECMA-48 machine. 1-byte `Parser` struct.

Key properties:

- `memchr` SIMD scan for ESC; state machine only on escape bytes
- `Cow::Borrowed` when no ESC in input (zero-alloc fast path)
- Full coverage: CSI, OSC (BEL + ST), DCS, APC/PM/SOS, SS2/SS3, Fe
- C1 codes (0x80-0x9F) passed through (UTF-8 safe)
- Idempotent: `strip(strip(x)) == strip(x)` for all `&[u8]`
- `StripStream` for chunked input (1-byte state across chunks)
- `strip_in_place` for zero-alloc gap compaction
- `#![forbid(unsafe_code)]`, `no_std + alloc`

## Sequence Coverage

| Crate                | CSI | OSC     | DCS | APC/PM/SOS | SS2/SS3 | Fe  |
| -------------------- | --- | ------- | --- | ---------- | ------- | --- |
| `distill-strip-ansi` | Yes | Yes     | Yes | Yes        | Yes     | Yes |
| `strip-ansi-escapes` | Yes | Yes     | Yes | Yes        | No      | Yes |
| `console`            | Yes | Partial | No  | No         | No      | No  |
| `vte`                | Yes | Yes     | Yes | Yes        | No      | Yes |
| `anstyle-parse`      | Yes | Yes     | Yes | Yes        | No      | Yes |
| `cansi`              | Yes | No      | No  | No         | No      | No  |
