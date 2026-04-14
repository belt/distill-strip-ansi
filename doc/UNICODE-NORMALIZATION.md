# Unicode Normalization

Unicode character normalization for terminal output. Maps
compatibility forms, styled variants, and legacy encoding
artifacts to their canonical equivalents.

Related docs:
[ANSI-REFERENCE.md](ANSI-REFERENCE.md) (escape sequence taxonomy),
[COLOR-TRANSFORMS.md](COLOR-TRANSFORMS.md) (SGR rewriting),
[DESIGN.md](DESIGN.md) (parser/filter architecture).

## Principle

Normalize to the simplest encoding that preserves semantic
meaning. `Ａ` (fullwidth) and `A` (ASCII) mean the same thing.
`𝐀` (math bold) and `A` mean the same thing outside mathematical
notation. The compatibility forms exist for round-trip fidelity
with legacy encodings, not because they carry distinct semantics.

This is not NFKC normalization. NFKC does too much (ligature
decomposition, superscript flattening in contexts where position
matters). This is targeted: only map characters where the source
and target are semantically identical in a terminal context.

## Two-Layer Architecture

Same pattern as threat scanning (`threat_db.rs`):

1. **Built-in** (~247 chars): compiled into the binary. Security-
   motivated mappings that defend against homograph attacks and
   visual deception. No filesystem dependency. Immutable.

2. **TOML files** (6 files, ~1800+ chars): shipped in
   `etc/unicode-mappings/`. Canonicalization mappings for niche
   audiences. Opt-in via `--unicode-map`. Additive to builtins.
   Duplicate `type_name` rejected with warning to stderr.

### Why Two Layers

A security feature should not fail because someone deleted a
data file. The built-in set catches the high-frequency homograph
vectors. The TOML files complete the Unicode standard for each
block.

A Unicode block can span both layers. The builtin contains the
security-critical slice. The TOML file contains the remainder
of the same block for full canonicalization. For example:

- `math_latin_bold` (builtin): bold A-Z, a-z (52 chars)
- `math-latin.toml` (TOML): italic, script, fraktur, sans-serif,
  monospace, double-struck, bold-italic, styled digits (~884 chars)

Together they cover the full Mathematical Alphanumeric Symbols
block. Separately, the builtin handles the most-abused subset
without filesystem dependency.

Some builtins cover their entire block (fullwidth ASCII,
superscript/subscript, Latin ligatures) because the block is
small and entirely security-relevant. These have no companion
TOML file.

A Unicode block can span both layers. The security-relevant
subset is compiled in. The remainder ships as TOML.

## Built-in Mappings

Enabled by default. Disabled with `--no-unicode-map` (security-
tagged sets require `--unsafe`).

Each builtin is the security-relevant subset of a Unicode block.
The companion TOML file (if any) completes the block.

| Builtin | Chars | Block coverage | TOML companion |
| ------- | ----- | -------------- | -------------- |
| fullwidth_ascii | 101 | entire block | — |
| math_latin_bold | 52 | bold only | math-latin.toml |
| latin_ligatures | 7 | entire block | — |
| enclosed_circled_letters | 52 | letters only | enclosed-alphanumerics.toml |
| superscript_subscript | ~42 | entire block | — |

### Fullwidth ASCII (101 chars)

U+FF01–FF5E to U+0021–007E (94 chars, constant offset)
plus U+FFE0–FFE6 to individual targets (7 chars).

Fullwidth forms of ASCII punctuation, digits, letters, and
currency symbols. The most common normalization need and the
primary homograph defense vector. `Ａdmin` becomes `Admin`.

Direction: narrowing (2-col to 1-col).

### Fullwidth Symbols (7 chars)

U+FFE0–FFE6 to individual targets.

| Source | Target | Name |
| ------ | ------ | ---- |
| ￠ FFE0 | ¢ 00A2 | cent sign |
| ￡ FFE1 | £ 00A3 | pound sign |
| ￢ FFE2 | ¬ 00AC | not sign |
| ￣ FFE3 | ¯ 00AF | macron |
| ￤ FFE4 | ¦ 00A6 | broken bar |
| ￥ FFE5 | ¥ 00A5 | yen sign |
| ￦ FFE6 | ₩ 20A9 | won sign |

Direction: narrowing.

### Math Latin Bold (52 chars)

U+1D400–1D419 to A–Z, U+1D41A–1D433 to a–z.

The most commonly abused styled alphabet for social media
"fancy text" generators and spam filter evasion. `𝐇𝐞𝐥𝐥𝐨`
becomes `Hello`.

Direction: narrowing (4-byte UTF-8 to 1-byte).

### Enclosed Circled Letters (52 chars)

Ⓐ–Ⓩ (U+24B6–24CF) to A–Z, ⓐ–ⓩ (U+24D0–24E9) to a–z.

Circled letter forms used for filter evasion and decorative
text. `Ⓗⓔⓛⓛⓞ` becomes `Hello`.

Direction: narrowing.

### Superscript and Subscript (all ~42 chars)

Superscript digits: ⁰ (U+2070), ¹ (U+00B9), ² (U+00B2),
³ (U+00B3), ⁴–⁹ (U+2074–2079) to 0–9.

Subscript digits: ₀–₉ (U+2080–2089) to 0–9.

Superscript/subscript letters and operators: ⁿ to n, ₐ to a,
⁺ to +, ₊ to +, ⁻ to -, ₋ to -, ⁼ to =, ₌ to =, ⁽ to (,
₍ to (, ⁾ to ), ₎ to ), etc.

Direction: neutral (same column width).

### Latin Ligatures (7 chars)

U+FB00–FB06 to ASCII letter pairs.

| Source | Target | Name |
| ------ | ------ | ---- |
| ﬀ FB00 | ff | Latin small ligature ff |
| ﬁ FB01 | fi | Latin small ligature fi |
| ﬂ FB02 | fl | Latin small ligature fl |
| ﬃ FB03 | ffi | Latin small ligature ffi |
| ﬄ FB04 | ffl | Latin small ligature ffl |
| ﬅ FB05 | st | Latin small ligature long s t |
| ﬆ FB06 | Latin small ligature st |

Common in copy-paste from PDFs. `ﬁle` does not match `file`
in grep, breaking search and pattern matching. Multi-codepoint
targets (fi → f + i).

Direction: neutral.

## TOML Mapping Files

Shipped in `etc/unicode-mappings/`. Loaded via `--unicode-map`.

### File Format

```toml
[metadata]
type = "math_latin"
description = "Styled Latin letters and digits → plain ASCII"
direction = "narrowing"    # narrowing | widening | neutral
tags = ["math", "canonicalize", "ascii-normalize"]

# Contiguous range with constant offset
[[ranges]]
from_start = "1D434"       # first source codepoint (hex, no U+ prefix)
from_end = "1D44D"         # last source codepoint (inclusive)
to_start = "0041"          # first target codepoint

# Individual pair (single codepoint target)
[[pairs]]
from = "FFE0"
to = "00A2"

# Individual pair (multi-codepoint target)
[[pairs]]
from = "2473"              # ⑳
to_seq = "0032 0030"       # "20" (space-separated hex codepoints)
```

#### Range Semantics

A `[[ranges]]` entry maps a contiguous block of source
codepoints to a contiguous block of target codepoints with
constant offset. The offset is implicit: `to_start - from_start`.

```
from_start = "1D434"  (source A)
from_end   = "1D44D"  (source Z)
to_start   = "0041"   (target A)
```

Every codepoint C in `[from_start..=from_end]` maps to
`C - from_start + to_start`.

#### Pair Semantics

A `[[pairs]]` entry maps a single source codepoint to either:
- `to`: a single target codepoint (hex string)
- `to_seq`: multiple target codepoints (space-separated hex)

Exactly one of `to` or `to_seq` must be present.

#### Metadata Fields

| Field | Required | Values |
| ----- | -------- | ------ |
| `type` | yes | unique identifier, snake_case |
| `description` | yes | human-readable purpose |
| `direction` | yes | `narrowing`, `widening`, `neutral` |
| `tags` | yes | array of tag strings |

### Shipped Files

| File | Chars | Direction | Tags |
| ---- | ----- | --------- | ---- |
| `math-latin.toml` | ~884 | narrowing | math, canonicalize, ascii-normalize |
| `math-greek.toml` | ~300 | neutral | math, canonicalize |
| `enclosed-alphanumerics.toml` | ~108 | narrowing | ascii-normalize, canonicalize |
| `enclosed-alphanumeric-supplement.toml` | ~80 | narrowing | ascii-normalize, canonicalize |
| `enclosed-cjk.toml` | ~256 | neutral | cjk, japanese, korean, canonicalize |
| `cjk-compatibility.toml` | ~256 | narrowing | japanese, cjk, canonicalize |
| `halfwidth-katakana.toml` | ~62 | widening | japanese, legacy-encoding, canonicalize |
| `halfwidth-hangul.toml` | ~52 | widening | korean, legacy-encoding, canonicalize |
| `cjk-compat-ideographs.toml` | ~472 | neutral | cjk, japanese, korean, canonicalize |
| `cjk-compat-ideographs-supplement.toml` | ~542 | neutral | cjk, canonicalize |
| `arabic-presentation-forms.toml` | ~600 | neutral | arabic, canonicalize |

### Tag Taxonomy

Tags enable semantic selection via `--unicode-map @tag`.

| Tag | Selects | Audience |
| --- | ------- | -------- |
| `@security` | builtins: fullwidth-ascii, math-latin-bold, latin-ligatures | homograph defense |
| `@ascii-normalize` | @security + enclosed-circled-letters, super/sub (builtins) | log normalization, search |
| `@narrowing` | direction = narrowing or neutral | monospace alignment |
| `@widening` | direction = widening | CJK canonicalization |
| `@canonicalize` | all TOML files | text processing |
| `@japanese` | halfwidth-katakana, cjk-compat-ideographs, enclosed-cjk, cjk-compatibility | Japanese text |
| `@korean` | halfwidth-hangul, cjk-compat-ideographs, enclosed-cjk | Korean text |
| `@cjk` | all CJK-relevant files | pan-CJK |
| `@math` | math-latin, math-greek | scientific text |
| `@arabic` | arabic-presentation-forms | Arabic text |
| `@all` | everything | full normalization |

### User Extension

Users create TOML files in the same format and load them with
`--unicode-map path/to/custom.toml`. Extension files are additive.
Entries with the same `type` as a built-in or shipped file are
rejected with a warning to stderr.

## CLI

All flags are on the `distill-ansi` binary. The `strip-ansi`
binary is unaffected — it strips escape sequences, not content.

One flag pair: `--unicode-map` / `--no-unicode-map`. Both accept
the same argument types: `@tag`, `name` (shipped file without
`.toml`), or `path/to/file.toml`.

### Default Behavior

Built-in mappings tagged `@ascii-normalize` are ON by default.
No flag needed to enable them. This includes all 4 builtin sets.

The `@ascii-normalize` tag is a superset of `@security`:

```
@security        = fullwidth_ascii + math_latin_bold
@ascii-normalize = @security + enclosed_circled_letters
                             + superscript_subscript
```

### Add Mappings

```
--unicode-map @tag           # add shipped files by tag
--unicode-map name           # add specific shipped file by name
--unicode-map path/to/file   # add user extension file
--unicode-map @all           # add all shipped files
```

Multiple `--unicode-map` values combine additively with the
defaults.

### Remove Mappings

```
--no-unicode-map @tag        # remove by tag
--no-unicode-map name        # remove specific set by name
```

Subtractive. Applies to both builtins and TOML-loaded sets.

Removing `@security`-tagged builtins requires `--unsafe`:

```
--unsafe --no-unicode-map @security
--unsafe --no-unicode-map fullwidth-ascii
--unsafe --no-unicode-map math-latin-bold
```

Without `--unsafe`, attempting to remove a security-tagged
mapping exits with an error explaining the risk.

### Examples

```sh
# Default: @ascii-normalize builtins on
distill-ansi

# Disable security subset (requires --unsafe)
distill-ansi --unsafe --no-unicode-map @security

# Disable all defaults
distill-ansi --no-unicode-map @ascii-normalize

# Disable one specific security builtin (requires --unsafe)
distill-ansi --unsafe --no-unicode-map fullwidth-ascii

# Disable one non-security builtin (no --unsafe needed)
distill-ansi --no-unicode-map superscript-subscript

# Defaults + Japanese canonicalization
distill-ansi --unicode-map @japanese

# Defaults + all shipped TOML files
distill-ansi --unicode-map @all

# Defaults + user extension file
distill-ansi --unicode-map ~/my-mappings.toml

# Everything except widening transforms
distill-ansi --unicode-map @all --no-unicode-map @widening
```

## Feature Flags

```toml
[features]
unicode-normalize = ["transform"]
```

The `unicode-normalize` feature enables the built-in mapping
table and the transform logic. No new dependencies. Built-in
mappings are active by default when the feature is compiled in.

TOML file loading reuses the existing `toml-config` feature
(which provides `serde` + `toml`). When `unicode-normalize`
is enabled without `toml-config`, only built-in mappings are
available and `--unicode-map` accepts only `--no-unicode-map`
for disabling builtins.

The `distill-ansi-cli` feature implies `unicode-normalize`.

## Transform Pipeline

Unicode normalization runs after SGR rewriting in the
`distill-ansi` transform pipeline:

```
input → ANSI parse → [palette remap] → [depth reduce]
      → [unicode normalize] → output
```

Normalization applies to content bytes only (`Action::Emit`).
Escape sequence bytes (`Action::Skip`) are never modified —
they are pure ASCII.

### Fast Path

Most terminal output contains no normalizable characters.
The fast path skips chunks with no high bytes:

- BMP fullwidth/enclosed/superscript: lead byte 0xEF (3-byte
  UTF-8) or 0xC2-0xC3 (2-byte for Latin-1 Supplement super/sub)
- SMP math symbols: lead byte 0xF0 (4-byte UTF-8)

`memchr` scans for these lead bytes, mirroring the existing
`memchr(0x1B)` pattern for ESC detection. Chunks with no
matching lead bytes pass through untouched.

### Streaming

Width normalization changes byte counts (3-byte fullwidth to
1-byte ASCII, 4-byte math to 1-byte ASCII). Borrowed `&[u8]`
slices cannot represent the output. The existing
`TransformSlice` enum handles this:

```rust
enum TransformSlice<'a> {
    Passthrough(&'a [u8]),  // no change
    Rewritten(Vec<u8>),     // modified content
}
```

### Lookup

Built-in mappings use a match on UTF-8 lead byte ranges for
O(1) dispatch to the correct mapping function. Range mappings
use arithmetic (offset). Pair mappings use a sorted array with
binary search.

TOML-loaded mappings are merged into a single sorted lookup
table at load time. Duplicate source codepoints across files
are rejected.

## Direction Semantics

Each mapping has a `direction` that describes its effect on
terminal column width:

| Direction | Column width change | Example |
| --------- | ------------------- | ------- |
| narrowing | 2-col → 1-col | Ａ → A |
| widening | 1-col → 2-col | ｱ → ア |
| neutral | same width | ⁿ → n |

Direction is metadata for filtering (`@narrowing` tag), not
enforced by the runtime. A user who loads a widening file
accepts the column width change.

## Security Considerations

### Homograph Defense

Fullwidth ASCII and math styled letters are the primary vectors
for homograph confusion in terminal output. An attacker can
craft filenames, URLs, or log messages using `Ａ` (U+FF21)
instead of `A` (U+0041) to evade pattern matching, grep, and
human visual inspection.

The built-in mappings neutralize these vectors without requiring
opt-in. When `--normalize-unicode` is enabled, homograph
characters are collapsed to their ASCII equivalents before
output reaches the user or downstream tools.

### Encoding Confusion

C1 control codes (0x80–0x9F) collide with UTF-8 lead bytes.
The existing `contains_ansi_c1()` function documents this.
Unicode normalization does not change this — it operates on
valid UTF-8 codepoints, not on raw byte interpretation.

### Round-Trip Fidelity

Normalization is lossy by design. `Ａ` and `A` become
indistinguishable after normalization. This is the intended
behavior for terminal output processing. Users who need
round-trip fidelity with legacy encodings should not enable
`--normalize-unicode`.

## References

### Unicode Standards

- UAX #15: Unicode Normalization Forms.
  [unicode.org/reports/tr15](https://unicode.org/reports/tr15/)
- UAX #11: East Asian Width.
  [unicode.org/reports/tr11](https://unicode.org/reports/tr11/)
- UnicodeData.txt: canonical decomposition mappings (field 5).
  [unicode.org/Public/UNIDATA](https://unicode.org/Public/UNIDATA/UnicodeData.txt)

### Unicode Blocks

- Halfwidth and Fullwidth Forms (U+FF00–FFEF).
  [unicode.org/charts](https://unicode.org/charts/nameslist/n_FF00.html)
- Mathematical Alphanumeric Symbols (U+1D400–1D7FF).
  [wikipedia](https://en.wikipedia.org/wiki/Mathematical_Alphanumeric_Symbols)
- Enclosed Alphanumerics (U+2460–24FF).
  [wikipedia](https://en.wikipedia.org/wiki/Enclosed_Alphanumerics)
- CJK Compatibility Ideographs (U+F900–FAD9).
  [unicode.org/charts](https://unicode.org/charts/nameslist/n_F900.html)
- Superscripts and Subscripts (U+2070–209F).
  [wikipedia](https://en.wikipedia.org/wiki/Superscripts_and_Subscripts)
- Letterlike Symbols (U+2100–214F).
  [wikipedia](https://en.wikipedia.org/wiki/Letterlike_Symbols)
