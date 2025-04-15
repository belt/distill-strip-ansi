//! Instruction-count benchmark for distill's hot paths.
//!
//! Callgrind variant of `benches/internals.rs` — deterministic
//! measurement of the pipeline stages most likely to dominate cost:
//! the `strip` entry point, the streaming API, the classifier, the
//! filter, threat scanning, color transforms, and Unicode normalization.
//!
//! Sized from the `IAI_SIZES` ladder (shared with ecosystem benches)
//! so numbers are comparable across crates and across hosts. See
//! `benches/harness/src/iai_inputs.rs` for the rationale.

use distill_bench_harness::{LARGE, MEDIUM, SMALL, TINY, XLARGE, iai_cargo, iai_input, iai_osc8};
use iai_callgrind::{library_benchmark, library_benchmark_group, main};
use std::hint::black_box;

// ── strip entrypoint ────────────────────────────────────────────────

#[library_benchmark]
#[bench::tiny(iai_input(TINY))]
#[bench::small(iai_input(SMALL))]
#[bench::medium(iai_input(MEDIUM))]
#[bench::large(iai_input(LARGE))]
#[bench::xlarge(iai_input(XLARGE))]
fn strip_dirty(input: Vec<u8>) -> Vec<u8> {
    strip_ansi::strip(black_box(&input)).into_owned()
}

#[library_benchmark]
#[bench::cargo(iai_cargo())]
#[bench::osc8(iai_osc8())]
fn strip_fixture(input: Vec<u8>) -> Vec<u8> {
    strip_ansi::strip(black_box(&input)).into_owned()
}

// ── streaming ───────────────────────────────────────────────────────

#[library_benchmark]
#[bench::tiny(iai_input(TINY))]
#[bench::small(iai_input(SMALL))]
#[bench::medium(iai_input(MEDIUM))]
#[bench::large(iai_input(LARGE))]
#[bench::xlarge(iai_input(XLARGE))]
fn stream_dirty(input: Vec<u8>) -> Vec<u8> {
    let mut stream = strip_ansi::StripStream::new();
    let mut out = Vec::with_capacity(input.len());
    stream.push(black_box(&input), &mut out);
    stream.finish();
    out
}

// ── classifier ──────────────────────────────────────────────────────

#[library_benchmark]
#[bench::cargo(iai_cargo())]
fn classifier_cargo(input: Vec<u8>) -> u32 {
    use strip_ansi::{ClassifyingParser, SeqAction};
    let mut cp = ClassifyingParser::new();
    let mut count = 0u32;
    for &byte in black_box(&input) {
        if cp.feed(byte) == SeqAction::EndSeq {
            let _ = black_box(cp.detail());
            count += 1;
        }
    }
    count
}

// Classify-only: same loop as `classifier_cargo` minus the
// per-sequence `detail()` call. Measures the parser itself
// without the descriptor-materialisation cost.
#[library_benchmark]
#[bench::cargo(iai_cargo())]
fn classifier_cargo_no_detail(input: Vec<u8>) -> u32 {
    use strip_ansi::{ClassifyingParser, SeqAction};
    let mut cp = ClassifyingParser::new();
    let mut count = 0u32;
    for &byte in black_box(&input) {
        if cp.feed(byte) == SeqAction::EndSeq {
            count += 1;
        }
    }
    count
}

// ── filter ──────────────────────────────────────────────────────────

#[library_benchmark]
#[bench::cargo(iai_cargo())]
fn filter_sanitize_preset(input: Vec<u8>) -> Vec<u8> {
    let config = strip_ansi::TerminalPreset::Sanitize.to_filter_config();
    strip_ansi::filter_strip(black_box(&input), &config).into_owned()
}

// Selective SGR filtering — strip everything except SGR, and mask
// SGR content to basic colors. Exercises the `SgrContent::BASIC`
// bitmask hot path distinct from the all-strip fast path.
#[library_benchmark]
#[bench::cargo(iai_cargo())]
fn filter_sgr_mask(input: Vec<u8>) -> Vec<u8> {
    use strip_ansi::{FilterConfig, SeqKind, SgrContent, filter_strip};
    let config = FilterConfig::strip_all()
        .no_strip_kind(SeqKind::CsiSgr)
        .with_sgr_mask(SgrContent::BASIC);
    filter_strip(black_box(&input), &config).into_owned()
}

// ── threat scan ─────────────────────────────────────────────────────

#[library_benchmark]
#[bench::clean_cargo(iai_cargo())]
fn threat_scan(input: Vec<u8>) -> u32 {
    use strip_ansi::{ClassifyingParser, SeqAction, SeqKind};
    let mut cp = ClassifyingParser::new();
    let mut threats = 0u32;
    for &byte in black_box(&input) {
        if cp.feed(byte) == SeqAction::EndSeq {
            let d = cp.detail();
            if matches!(d.kind, SeqKind::Dcs | SeqKind::CsiQuery)
                || (d.kind == SeqKind::Osc && d.osc_number == 50)
            {
                threats += 1;
            }
        }
    }
    threats
}

// Threat-scan on input that actually contains embedded threats
// (DCS, CsiQuery, OSC 50). The hit rate changes branch-predictor
// behaviour versus `threat_scan` on clean cargo output, so this
// is a separate data point.
#[library_benchmark]
#[bench::dirty_cargo(threat_input())]
fn threat_scan_dirty(input: Vec<u8>) -> u32 {
    use strip_ansi::{ClassifyingParser, SeqAction, SeqKind};
    let mut cp = ClassifyingParser::new();
    let mut threats = 0u32;
    for &byte in black_box(&input) {
        if cp.feed(byte) == SeqAction::EndSeq {
            let d = cp.detail();
            if matches!(d.kind, SeqKind::Dcs | SeqKind::CsiQuery)
                || (d.kind == SeqKind::Osc && d.osc_number == 50)
            {
                threats += 1;
            }
        }
    }
    threats
}

fn threat_input() -> Vec<u8> {
    // Mirrors the `threat_input` fixture in `benches/internals.rs`.
    let mut v = Vec::new();
    for _ in 0..100 {
        v.extend_from_slice(b"\x1b[0m\x1b[1m\x1b[32m   Compiling\x1b[0m memchr v2.7.1\n");
    }
    v.extend_from_slice(b"\x1b[21t");
    v.extend_from_slice(b"\x1b]50;?\x07");
    v.extend_from_slice(b"\x1bP$qm\x1b\\");
    v
}

// ── color transform ─────────────────────────────────────────────────

fn truecolor_input() -> Vec<u8> {
    let mut v = Vec::new();
    for i in 0..100u8 {
        v.extend_from_slice(
            format!(
                "\x1b[38;2;{};{};{}m   Compiling\x1b[0m crate v0.{}.0\n",
                i,
                255 - i,
                128,
                i
            )
            .as_bytes(),
        );
    }
    v
}

/// Byte-identical to `color256_input` in `benches/internals.rs` —
/// lines tagged with 256-color SGR (`\x1b[38;5;N m`) so the 256→*
/// downgrade paths have real palette indices to map.
fn color256_input() -> Vec<u8> {
    let mut v = Vec::new();
    for i in 0..100u8 {
        v.extend_from_slice(
            format!("\x1b[38;5;{i}m   Compiling\x1b[0m crate v0.{i}.0\n").as_bytes(),
        );
    }
    v
}

/// Byte-identical to `basic_input = real_world_cargo()` — cargo-
/// style lines with basic ANSI colors (bright green Compiling,
/// reset). Covers the `basic→mono` and `passthrough` transform
/// paths in iai, aligning row-for-row with criterion's output.
fn basic_input() -> Vec<u8> {
    let mut v = Vec::new();
    for _ in 0..100 {
        v.extend_from_slice(b"\x1b[0m\x1b[1m\x1b[32m   Compiling\x1b[0m memchr v2.7.1\n");
    }
    v
}

// Per-depth transform benches — one `#[library_benchmark]` per
// source/depth pair so criterion rows and iai rows line up 1:1.
// The internals.rs criterion bench uses a `bench_xform!` macro
// over the same pairs; iai-callgrind's proc-macro won't digest
// a declarative macro wrapper, hence the repetition.

#[library_benchmark]
#[bench::truecolor_to_mono(truecolor_input())]
fn transform_to_mono(input: Vec<u8>) -> Vec<u8> {
    use strip_ansi::downgrade::ColorDepth;
    use strip_ansi::{TransformConfig, TransformStream};
    let config = TransformConfig::new(ColorDepth::Mono);
    let mut stream = TransformStream::new();
    let mut out = Vec::with_capacity(input.len());
    stream.push(black_box(&input), &config, &mut out);
    stream.finish();
    out
}

#[library_benchmark]
#[bench::truecolor_to_greyscale(truecolor_input())]
fn transform_truecolor_to_grey(input: Vec<u8>) -> Vec<u8> {
    use strip_ansi::downgrade::ColorDepth;
    use strip_ansi::{TransformConfig, TransformStream};
    let config = TransformConfig::new(ColorDepth::Greyscale);
    let mut stream = TransformStream::new();
    let mut out = Vec::with_capacity(input.len());
    stream.push(black_box(&input), &config, &mut out);
    stream.finish();
    out
}

#[library_benchmark]
#[bench::truecolor_to_16(truecolor_input())]
fn transform_truecolor_to_16(input: Vec<u8>) -> Vec<u8> {
    use strip_ansi::downgrade::ColorDepth;
    use strip_ansi::{TransformConfig, TransformStream};
    let config = TransformConfig::new(ColorDepth::Color16);
    let mut stream = TransformStream::new();
    let mut out = Vec::with_capacity(input.len());
    stream.push(black_box(&input), &config, &mut out);
    stream.finish();
    out
}

#[library_benchmark]
#[bench::truecolor_to_256(truecolor_input())]
fn transform_truecolor_to_256(input: Vec<u8>) -> Vec<u8> {
    use strip_ansi::downgrade::ColorDepth;
    use strip_ansi::{TransformConfig, TransformStream};
    let config = TransformConfig::new(ColorDepth::Color256);
    let mut stream = TransformStream::new();
    let mut out = Vec::with_capacity(input.len());
    stream.push(black_box(&input), &config, &mut out);
    stream.finish();
    out
}

#[library_benchmark]
#[bench::color256_to_16(color256_input())]
fn transform_256_to_16(input: Vec<u8>) -> Vec<u8> {
    use strip_ansi::downgrade::ColorDepth;
    use strip_ansi::{TransformConfig, TransformStream};
    let config = TransformConfig::new(ColorDepth::Color16);
    let mut stream = TransformStream::new();
    let mut out = Vec::with_capacity(input.len());
    stream.push(black_box(&input), &config, &mut out);
    stream.finish();
    out
}

#[library_benchmark]
#[bench::color256_to_greyscale(color256_input())]
fn transform_256_to_grey(input: Vec<u8>) -> Vec<u8> {
    use strip_ansi::downgrade::ColorDepth;
    use strip_ansi::{TransformConfig, TransformStream};
    let config = TransformConfig::new(ColorDepth::Greyscale);
    let mut stream = TransformStream::new();
    let mut out = Vec::with_capacity(input.len());
    stream.push(black_box(&input), &config, &mut out);
    stream.finish();
    out
}

#[library_benchmark]
#[bench::basic_to_mono(basic_input())]
fn transform_basic_to_mono(input: Vec<u8>) -> Vec<u8> {
    use strip_ansi::downgrade::ColorDepth;
    use strip_ansi::{TransformConfig, TransformStream};
    let config = TransformConfig::new(ColorDepth::Mono);
    let mut stream = TransformStream::new();
    let mut out = Vec::with_capacity(input.len());
    stream.push(black_box(&input), &config, &mut out);
    stream.finish();
    out
}

// Transform in pass-through mode (`ColorDepth::Truecolor`) — same
// code path as a real transform, but no color remapping happens.
// Measures the fixed overhead of the transform stream framing
// distinct from any palette math. Matches criterion's
// `passthrough` which uses basic_input, not truecolor_input.
#[library_benchmark]
#[bench::passthrough(basic_input())]
fn transform_passthrough(input: Vec<u8>) -> Vec<u8> {
    use strip_ansi::downgrade::ColorDepth;
    use strip_ansi::{TransformConfig, TransformStream};
    let config = TransformConfig::new(ColorDepth::Truecolor);
    let mut stream = TransformStream::new();
    let mut out = Vec::with_capacity(input.len());
    stream.push(black_box(&input), &config, &mut out);
    stream.finish();
    out
}

// ── palette augmentation ────────────────────────────────────────────

#[library_benchmark]
fn augment_protanopia_256() -> u64 {
    use strip_ansi::palette::{PROTANOPIA_VIENOT, PaletteTransform};
    let proto = PaletteTransform::from_matrix(PROTANOPIA_VIENOT);
    let rgb: Vec<(u8, u8, u8)> = (0..256u16)
        .map(|i| (i as u8, (255 - i) as u8, 128))
        .collect();
    let mut acc: u64 = 0;
    for &(r, g, b) in black_box(&rgb) {
        let (tr, tg, tb) = proto.transform(r, g, b);
        acc = acc.wrapping_add(u64::from(tr) + u64::from(tg) + u64::from(tb));
    }
    acc
}

#[library_benchmark]
fn augment_deuteranopia_256() -> u64 {
    use strip_ansi::palette::{DEUTERANOPIA_VIENOT, PaletteTransform};
    let deuter = PaletteTransform::from_matrix(DEUTERANOPIA_VIENOT);
    let rgb: Vec<(u8, u8, u8)> = (0..256u16)
        .map(|i| (i as u8, (255 - i) as u8, 128))
        .collect();
    let mut acc: u64 = 0;
    for &(r, g, b) in black_box(&rgb) {
        let (tr, tg, tb) = deuter.transform(r, g, b);
        acc = acc.wrapping_add(u64::from(tr) + u64::from(tg) + u64::from(tb));
    }
    acc
}

// sRGB ↔ linear roundtrip over all 256 8-bit values — exercises
// the gamma LUT hot paths used by every palette transform.
#[library_benchmark]
fn augment_srgb_roundtrip_256() -> u64 {
    use strip_ansi::palette::{linear_to_srgb, srgb_to_linear};
    let mut acc: u64 = 0;
    for i in 0..=255u8 {
        let lin = srgb_to_linear(black_box(i));
        let back = linear_to_srgb(lin);
        acc = acc.wrapping_add(u64::from(back));
    }
    acc
}

// ── Unicode normalize ───────────────────────────────────────────────

#[library_benchmark]
#[bench::cargo(iai_cargo())]
fn unicode_normalize_cargo(input: Vec<u8>) -> Vec<u8> {
    use strip_ansi::unicode_map::UnicodeMap;
    let map = UnicodeMap::builtin();
    let s = std::str::from_utf8(black_box(&input)).unwrap();
    let mut out = Vec::with_capacity(input.len());
    let mut char_buf = Vec::new();
    for ch in s.chars() {
        char_buf.clear();
        if map.lookup_into(ch, &mut char_buf) {
            for &tc in &char_buf {
                let mut enc = [0u8; 4];
                out.extend_from_slice(tc.encode_utf8(&mut enc).as_bytes());
            }
        } else {
            let mut enc = [0u8; 4];
            out.extend_from_slice(ch.encode_utf8(&mut enc).as_bytes());
        }
    }
    out
}

library_benchmark_group!(
    name = internals_iai;
    benchmarks =
        strip_dirty,
        strip_fixture,
        stream_dirty,
        classifier_cargo,
        classifier_cargo_no_detail,
        filter_sanitize_preset,
        filter_sgr_mask,
        threat_scan,
        threat_scan_dirty,
        transform_passthrough,
        transform_to_mono,
        transform_truecolor_to_grey,
        transform_truecolor_to_16,
        transform_truecolor_to_256,
        transform_256_to_16,
        transform_256_to_grey,
        transform_basic_to_mono,
        augment_protanopia_256,
        augment_deuteranopia_256,
        augment_srgb_roundtrip_256,
        unicode_normalize_cargo
);

main!(library_benchmark_groups = internals_iai);
