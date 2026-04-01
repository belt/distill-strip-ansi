# Design

## Architecture

```text
Input bytes ──► memchr(0x1B) ──► Parser::feed() ──► Action::Emit / Skip
                  │                   │
                  │ SIMD scan         │ 1-byte state machine
                  │ (no ESC? done)    │ 15 states, repr(u8)
                  ▼                   ▼
              Cow::Borrowed       Cow::Owned
              (zero alloc)        (only when needed)
```

Two-phase pipeline: `memchr` locates ESC bytes using SIMD intrinsics,
then a 1-byte state machine classifies each escape sequence byte-by-byte.
Clean regions between escapes are bulk-copied via `extend_from_slice`.

## Parser

15-state ECMA-48 state machine in a single `u8`. Covers:

- CSI (`ESC [` … final byte)
- OSC (`ESC ]` … BEL or ST)
- DCS (`ESC P` … ST), with param/passthrough sub-states
- APC/PM/SOS (collapsed into shared `StringPassthrough`)
- SS2 (`ESC N`), SS3 (`ESC O`)
- Fe (single-byte `ESC` + 0x40–0x5F)
- `EscIntermediate` for multi-byte escape sequences
- CAN/SUB abort handling per ECMA-48 §5.6

The `StEsc` re-entry loop handles `ESC` inside string states
(OSC/DCS/APC/PM/SOS) — runs at most 2 iterations, no recursion.

Compile-time guarantees: `size_of::<Parser>() == 1`, `Send + Sync`.

## Allocation Strategy

| Input pattern          | Return type                  | Allocations |
| ---------------------- | ---------------------------- | ----------- |
| No ESC bytes           | `Cow::Borrowed(input)`       | 0           |
| Only trailing escapes  | `Cow::Borrowed(&input[..n])` | 0           |
| Only leading escapes   | `Cow::Borrowed(&input[n..])` | 0           |
| Interleaved            | `Cow::Owned(Vec)`            | 1           |

`strip_in_place` uses gap compaction via `copy_within` — no second buffer.

## Streaming

`StripStream` carries 1 byte of state across chunk boundaries.
`strip_slices()` returns an iterator of borrowed `&[u8]` slices —
zero intermediate copies. Incomplete escapes at chunk boundaries
are skipped (never retroactively emitted).

`FilterStream` (feature `filter`) adds per-sequence-type preservation
using `ClassifyingParser` (3 bytes: Parser + SeqKind + bool).

## Filter System

`FilterConfig` uses a 16-bit group bitfield + `SmallVec<[SeqKind; 4]>`
for sub-kind overrides. `should_strip()` is O(1) for group checks.

9 sequence groups, 16 sequence kinds (8 CSI sub-kinds classified by
final byte). The classifier wraps `Parser` without changing the
underlying state machine.

## Dependencies

| Crate      | Role                             | Size  |
| ---------- | -------------------------------- | ----- |
| `memchr`   | SIMD ESC byte scanning           | ~70KB |
| `smallvec` | Inline sub-kind storage (filter) | ~30KB |

No transitive dependencies beyond these. `no_std` compatible
(requires `alloc`).
