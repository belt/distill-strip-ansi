# Library Usage

API examples by feature. Architecture details in
[DESIGN.md](DESIGN.md). Feature flags in the
[README](../README.md#feature-flags).

## Strip All (default)

```toml
[dependencies]
strip-ansi = { package = "distill-strip-ansi", version = "0.5", default-features = false, features = ["std"] }
```

```rust
use strip_ansi::{strip, strip_str, StripStream};

// One-shot — returns Cow<[u8]> (zero-alloc when no ESC)
let clean = strip(b"\x1b[31mhello\x1b[0m");

// Streaming — 1 byte of cross-chunk state
let mut stream = StripStream::new();
let mut out = Vec::new();
stream.push(chunk, &mut out);
```

## Selective Filtering

```toml
[dependencies]
strip-ansi = { package = "distill-strip-ansi", version = "0.5", default-features = false, features = ["filter"] }
```

```rust
use strip_ansi::{filter_strip, FilterConfig, FilterStream,
    TerminalPreset, SeqGroup, SeqKind};

// Preset-based: preserve what the terminal can handle
let config = TerminalPreset::Sanitize.to_filter_config();
let clean = filter_strip(input, &config);

// Custom: preserve only SGR (colors) and OSC hyperlinks
use strip_ansi::classifier::OscType;
let config = FilterConfig::strip_all()
    .no_strip_kind(SeqKind::CsiSgr)
    .no_strip_osc_type(OscType::Hyperlink);

// Streaming filter
let mut stream = FilterStream::new();
for slice in stream.filter_slices(chunk, &config) {
    output.extend_from_slice(slice);
}
```

## Terminal Detection

```toml
[dependencies]
strip-ansi = { package = "distill-strip-ansi", version = "0.5", default-features = false, features = ["terminal-detect"] }
```

```rust
use strip_ansi::{detect_preset_untrusted, detect_sgr_mask_untrusted};

// Auto-detect from trusted signals only (isatty + TERM)
let preset = detect_preset_untrusted(); // caps at Sanitize
let config = preset.to_filter_config();

// SGR color depth from TERM alone
if let Some(mask) = detect_sgr_mask_untrusted() {
    // mask: BASIC, BASIC|EXTENDED, or BASIC|EXTENDED|TRUECOLOR
}
```

## Color Transforms

See [COLOR-TRANSFORMS.md](COLOR-TRANSFORMS.md) for the full
design (depth reduction, palette remapping, CVD simulation).

```toml
[dependencies]
strip-ansi = { package = "distill-strip-ansi", version = "0.5", default-features = false, features = ["transform"] }
```

```rust
use strip_ansi::{TransformStream, TransformConfig};
use strip_ansi::downgrade::ColorDepth;

// Rewrite truecolor → 256-color (preserves styles)
let config = TransformConfig::new(ColorDepth::Color256);
let mut stream = TransformStream::new();
let mut out = Vec::new();
stream.push(chunk, &config, &mut out);
```

## Threat Detection

See [SECURITY.md](SECURITY.md) for the threat model.

```toml
[dependencies]
strip-ansi = { package = "distill-strip-ansi", version = "0.5", default-features = false, features = ["toml-config"] }
```

```rust
use strip_ansi::{ThreatDb, ClassifyingParser, SeqAction};

let db = ThreatDb::builtin(); // 6 built-in echoback patterns
// Or: ThreatDb::from_file("threats.toml")?

let mut cp = ClassifyingParser::new();
for &byte in input {
    if cp.feed(byte) == SeqAction::EndSeq {
        if let Some(threat) = db.classify(&cp.detail()) {
            eprintln!("threat: {} ({})", threat.type_name,
                threat.cve.as_deref().unwrap_or("no CVE"));
        }
    }
}
```

## `no_std`

Requires `alloc`. Omit `std` feature:

```toml
[dependencies]
strip-ansi = { package = "distill-strip-ansi", version = "0.5", default-features = false }
```

All core types (`strip`, `Parser`, `StripStream`,
`ClassifyingParser`, `FilterConfig`) work without `std`.
`StripWriter` and I/O traits require `std`.
