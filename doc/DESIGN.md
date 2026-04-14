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

| Input pattern         | Return type                  | Allocs |
| --------------------- | ---------------------------- | ------ |
| No ESC bytes          | `Cow::Borrowed(input)`       | 0      |
| Only trailing escapes | `Cow::Borrowed(&input[..n])` | 0      |
| Only leading escapes  | `Cow::Borrowed(&input[n..])` | 0      |
| Interleaved           | `Cow::Owned(Vec)`            | 1      |

`strip_in_place` uses gap compaction via `copy_within` — no second buffer.

## Streaming

`StripStream` carries 1 byte of state across chunk boundaries.
`strip_slices()` returns an iterator of borrowed `&[u8]` slices —
zero intermediate copies. Incomplete escapes at chunk boundaries
are skipped (never retroactively emitted).

`FilterStream` (feature `filter`) adds per-sequence-type preservation
using `ClassifyingParser` (12 bytes). The classifier extends `Parser`
with parameter-level inspection:

```text
ClassifyingParser layout (12 bytes):
  parser       : Parser      1B  (1-byte state machine)
  kind         : SeqKind     1B  (sequence sub-kind)
  flags        : u8          1B  (packed: in_seq, seen_semicolon,
                                  osc_number_finalized, osc_accumulating,
                                  dcs_is_query, DCS body phase)
  sgr_content  : SgrContent  1B  (SGR color depth bits)
  param_value  : u16         2B  (shared param accumulator)
  param_state  : ParamState  1B  (SGR FSM state)
  osc_type     : OscType     1B  (OSC sub-classification)
  first_param  : u16         2B  (first CSI parameter)
  osc_number   : u16         2B  (raw OSC number)
```

`param_value` is shared between CSI and OSC states (mutually
exclusive). `first_param` captures the first finalized CSI
parameter on `;` or at EndSeq for single-param sequences.
`flags` packs six booleans and a 2-bit DCS phase counter into
a single byte — previously these were separate fields.

`ClassifyingParser::detail() -> SeqDetail` bundles all classifier
outputs at EndSeq:

```rust
pub struct SeqDetail {
    pub kind: SeqKind,
    pub sgr_content: SgrContent,
    pub osc_type: OscType,
    pub osc_number: u16,
    pub first_param: u16,
    pub dcs_is_query: bool,
}
```

## Filter System

`FilterConfig` uses a 16-bit group bitfield + `SmallVec<[SeqKind; 4]>`
for sub-kind overrides + optional `SgrContent` mask + optional
`SmallVec<[OscType; 2]>` for OSC sub-type filtering.

Extended strip decision (via `should_strip_detail`):

```text
1. strip-all mode → strip
2. SGR + sgr_mask set → strip when (sgr_content ∩ mask) = ∅
3. OSC group + osc_preserve set → strip when osc_type ∉ list
4. fallthrough → should_strip(kind)
```

Fast-path: when `sgr_preserve_mask` and `osc_preserve` are both
empty, degrades to `should_strip(kind)` with zero added cost.

9 sequence groups, 17 sequence kinds (8 CSI sub-kinds + CsiQuery,
classified by final byte + first_param). The classifier wraps
`Parser` without changing the underlying state machine.

## Dependencies

| Crate      | Role                           | Size  |
| ---------- | ------------------------------ | ----- |
| `memchr`   | SIMD ESC byte scanning         | ~70KB |
| `smallvec` | Inline sub-kind store (filter) | ~30KB |

No transitive dependencies beyond these. `no_std` compatible
(requires `alloc`).

## Performance

Benchmarked on Intel Core i7-9750H @ 2.60GHz with
`cargo bench --all-features`.
Input: 4.4KB simulated cargo output (~20% escape sequences).

### Two-Binary Model

Transform features live in a separate binary (`distill-ansi`)
that shares the `strip_ansi` library crate with `strip-ansi`.
No code duplication, no bloat in the stripping binary.

```text
strip-ansi       stripping + filtering + security
distill-ansi     color transforms + unicode normalization
strip_ansi       shared lib (parser, classifier, filter,
                 strip, stream, writer, + transform modules)
```

Source layout for transform modules:

```text
src/
  sgr_rewrite.rs        SGR param parser/rewriter (feature: transform)
  downgrade.rs          color depth reduction     (feature: downgrade-color)
  palette.rs            palette transforms        (feature: augment-color)
  transform_stream.rs   streaming transform API   (feature: transform)
  unicode_map.rs        homograph normalization   (feature: unicode-normalize)
  distill_ansi_main.rs  distill-ansi entry point
```

### Transform Pipeline

At `EndSeq` for CsiSgr sequences with color content:

```text
1. Is palette or depth transform configured?
   NO  → existing strip/preserve path (unchanged)
   YES → continue
2. Re-parse SGR params from seq_buf
3. For each color param:
   a. If palette set: apply 3x3 matrix in linear RGB
   b. If depth reduction needed: downgrade
4. Emit rewritten sequence to output
```

When both `--palette` and `--color-depth` are specified:

```text
Input SGR → extract color → palette transform → depth reduce → emit
```

`TransformStream` provides the streaming API with
`TransformSlice` (`Borrowed(&[u8])` / `Owned(SmallVec)`)
for zero-copy passthrough of non-SGR content.

See [COLOR-TRANSFORMS.md](COLOR-TRANSFORMS.md) for the full
color science reference (algorithms, CVD matrices, palettes).

### Classifier Overhead (3B → 12B)

| Benchmark         | Input | Throughput   |
| ----------------- | ----- | ------------ |
| raw classify      | cargo | ~7.1 TiB/s ¹ |
| classify + detail | cargo | ~82 MiB/s    |
| classify + detail | osc8  | ~68 MiB/s    |

¹ Per-byte cost dominated by branch prediction, not
struct size. The 12-byte classifier fits in a single
cache line.

### Filter Decision

| Config          | Input | Throughput | vs baseline |
| --------------- | ----- | ---------- | ----------- |
| kind only       | cargo | ~99 MiB/s  | baseline    |
| + SGR mask      | cargo | ~95 MiB/s  | ~5% slower  |
| + OSC preserve  | osc8  | ~68 MiB/s  | n/a ²       |
| sanitize preset | cargo | ~98 MiB/s  | ~1% slower  |

² Different input (OSC-heavy vs SGR-heavy).

Fast-path verified: when `sgr_preserve_mask` and
`osc_preserve` are empty, `should_strip_detail`
compiles to the same path as `should_strip(kind)`.

### Threat Scanning

| Benchmark  | Input        | Throughput |
| ---------- | ------------ | ---------- |
| scan+match | with threats | ~92 MiB/s  |
| scan+match | clean cargo  | ~95 MiB/s  |

Threat scanning adds ~5% overhead vs baseline
filtering. Clean input (no threats) is essentially
free — the match is a few comparisons at EndSeq.

### Reproduce

```sh
cargo bench --all-features -- classifier
cargo bench --all-features -- filter_detail
cargo bench --all-features -- check_threats
```
