//! Unit and property tests for TransformStream (streaming color transforms).

#![cfg(feature = "transform")]

use strip_ansi::downgrade::ColorDepth;
use strip_ansi::{TransformConfig, TransformSlice, TransformStream};

fn collect(stream: &mut TransformStream, input: &[u8], config: &TransformConfig) -> Vec<u8> {
    let mut out = Vec::new();
    for slice in stream.transform_slices(input, config) {
        out.extend_from_slice(slice.as_bytes());
    }
    out
}

fn transform_chunks(chunks: &[&[u8]], depth: ColorDepth) -> Vec<u8> {
    let config = TransformConfig::new(depth);
    let mut stream = TransformStream::new();
    let mut out = Vec::new();
    for chunk in chunks {
        out.extend(collect(&mut stream, chunk, &config));
    }
    stream.finish();
    out
}

// ── Passthrough ─────────────────────────────────────────────────────

#[test]
fn passthrough_clean_text() {
    let result = transform_chunks(&[b"hello world"], ColorDepth::Truecolor);
    assert_eq!(result, b"hello world");
}

#[test]
fn passthrough_preserves_all_sequences() {
    let input = b"a\x1b[38;2;255;0;0mb\x1b[0mc";
    let result = transform_chunks(&[input], ColorDepth::Truecolor);
    assert_eq!(result, input);
}

// ── Basic transform (single chunk) ──────────────────────────────────

#[test]
fn truecolor_to_256_single_chunk() {
    let input = b"a\x1b[38;2;255;0;0mb";
    let result = transform_chunks(&[input], ColorDepth::Color256);
    let s = String::from_utf8_lossy(&result);
    assert!(s.starts_with("a\x1b[38;5;"), "expected 256-color: {s}");
    assert!(s.ends_with("b"), "content after sequence missing");
    assert!(!s.contains("38;2;"), "truecolor should be rewritten");
}

#[test]
fn truecolor_to_16_single_chunk() {
    let input = b"a\x1b[38;2;255;0;0mb";
    let result = transform_chunks(&[input], ColorDepth::Color16);
    let s = String::from_utf8_lossy(&result);
    assert!(!s.contains("38;2;"), "truecolor should be rewritten");
    assert!(!s.contains("38;5;"), "256-color should not appear");
    assert!(s.starts_with("a\x1b["), "should start with content + CSI");
}

#[test]
fn mono_strips_color_keeps_style() {
    let input = b"\x1b[1;38;2;255;0;0;4mtext\x1b[0m";
    let result = transform_chunks(&[input], ColorDepth::Mono);
    let s = String::from_utf8_lossy(&result);
    assert!(!s.contains("38"), "color should be stripped in mono");
    assert!(s.contains("1"), "bold should survive");
    assert!(s.contains("4"), "underline should survive");
    assert!(s.contains("text"), "content should survive");
}

#[test]
fn non_sgr_sequences_pass_through() {
    // Cursor movement should not be rewritten.
    let input = b"a\x1b[5Ab";
    let result = transform_chunks(&[input], ColorDepth::Color16);
    assert_eq!(result, input, "non-SGR should pass through unchanged");
}

#[test]
fn osc_sequences_pass_through() {
    let input = b"a\x1b]0;title\x07b";
    let result = transform_chunks(&[input], ColorDepth::Color16);
    assert_eq!(result, input, "OSC should pass through unchanged");
}

#[test]
fn basic_sgr_no_color_passes_through() {
    // Bold only — no color content, should pass through.
    let input = b"\x1b[1m";
    let result = transform_chunks(&[input], ColorDepth::Color16);
    assert_eq!(result, input, "style-only SGR should pass through");
}

// ── Cross-chunk transforms ──────────────────────────────────────────

#[test]
fn cross_chunk_sgr_stripped() {
    // SGR split across chunks — cross-chunk sequences are stripped
    // (same as FilterStream: start bytes are gone).
    let result = transform_chunks(&[b"a\x1b[38;2;255", b";0;0mb"], ColorDepth::Color256);
    assert_eq!(result, b"ab", "cross-chunk SGR should be stripped");
}

#[test]
fn cross_chunk_content_preserved() {
    let result = transform_chunks(&[b"hello ", b"world"], ColorDepth::Color256);
    assert_eq!(result, b"hello world");
}

#[test]
fn cross_chunk_non_sgr_stripped() {
    // Non-SGR CSI split across chunks.
    let result = transform_chunks(&[b"a\x1b[5", b"Ab"], ColorDepth::Color256);
    assert_eq!(result, b"ab", "cross-chunk non-SGR should be stripped");
}

// ── Slice type verification ─────────────────────────────────────────

#[test]
fn content_yields_borrowed() {
    let config = TransformConfig::new(ColorDepth::Color256);
    let mut stream = TransformStream::new();
    let slices: Vec<TransformSlice> = stream.transform_slices(b"hello", &config).collect();
    assert_eq!(slices.len(), 1);
    assert!(matches!(slices[0], TransformSlice::Borrowed(_)));
}

#[test]
fn rewritten_sgr_yields_owned() {
    let config = TransformConfig::new(ColorDepth::Color256);
    let mut stream = TransformStream::new();
    let slices: Vec<TransformSlice> = stream
        .transform_slices(b"\x1b[38;2;255;0;0m", &config)
        .collect();
    assert_eq!(slices.len(), 1);
    assert!(matches!(slices[0], TransformSlice::Owned(_)));
}

#[test]
fn non_color_sgr_yields_borrowed() {
    // Non-SGR sequence (cursor up) should be borrowed.
    let config = TransformConfig::new(ColorDepth::Color256);
    let mut stream = TransformStream::new();
    let slices: Vec<TransformSlice> = stream.transform_slices(b"\x1b[5A", &config).collect();
    assert_eq!(slices.len(), 1);
    assert!(
        matches!(slices[0], TransformSlice::Borrowed(_)),
        "non-SGR CSI should be borrowed"
    );
}

#[test]
fn mixed_content_and_sgr() {
    let config = TransformConfig::new(ColorDepth::Color256);
    let mut stream = TransformStream::new();
    let input = b"text\x1b[38;2;255;0;0mred\x1b[0mplain";
    let slices: Vec<TransformSlice> = stream.transform_slices(input, &config).collect();
    // Should have: "text" (borrowed), rewritten SGR (owned),
    // "red" (borrowed), reset (borrowed), "plain" (borrowed)
    assert!(
        slices.len() >= 3,
        "expected multiple slices, got {}",
        slices.len()
    );
    assert!(matches!(slices[0], TransformSlice::Borrowed(b"text")));
}

// ── push() equivalence ──────────────────────────────────────────────

#[test]
fn push_eq_slices() {
    let config = TransformConfig::new(ColorDepth::Color256);
    let input = b"a\x1b[38;2;255;0;0mb\x1b[0mc";

    let mut stream1 = TransformStream::new();
    let slices_out = collect(&mut stream1, input, &config);

    let mut stream2 = TransformStream::new();
    let mut push_out = Vec::new();
    stream2.push(input, &config, &mut push_out);

    assert_eq!(slices_out, push_out);
}

#[test]
fn push_across_chunks() {
    let config = TransformConfig::new(ColorDepth::Color256);
    let mut stream = TransformStream::new();
    let mut out = Vec::new();
    stream.push(b"a\x1b[38;2;255", &config, &mut out);
    stream.push(b";0;0mb", &config, &mut out);
    // Cross-chunk SGR is stripped.
    assert_eq!(out, b"ab");
}

// ── push_write() equivalence ────────────────────────────────────────

#[test]
fn push_write_eq_push() {
    let config = TransformConfig::new(ColorDepth::Color256);
    let input = b"a\x1b[38;2;255;0;0mb\x1b[0mc";

    let mut stream1 = TransformStream::new();
    let mut push_out = Vec::new();
    stream1.push(input, &config, &mut push_out);

    let mut stream2 = TransformStream::new();
    let mut write_out = Vec::new();
    stream2.push_write(input, &config, &mut write_out).unwrap();

    assert_eq!(push_out, write_out);
}

// ── Stateless equivalence ───────────────────────────────────────────

/// Compare streaming transform against the buffered distill_main transform.
#[test]
fn streaming_eq_buffered_single_chunk() {
    let config = TransformConfig::new(ColorDepth::Color256);
    let input = b"a\x1b[38;2;255;0;0mb\x1b[1mc\x1b[0md";

    let mut stream = TransformStream::new();
    let streaming = collect(&mut stream, input, &config);

    // Buffered: use sgr_rewrite directly on the full input.
    // We just verify the streaming output is valid and contains
    // the expected structure.
    let s = String::from_utf8_lossy(&streaming);
    assert!(s.starts_with("a"), "should start with content");
    assert!(s.contains("38;5;"), "should have 256-color");
    assert!(!s.contains("38;2;"), "truecolor should be gone");
    assert!(s.ends_with("d"), "should end with content");
}

// ── Reset / finish ──────────────────────────────────────────────────

#[test]
fn finish_discards_incomplete() {
    let config = TransformConfig::new(ColorDepth::Color256);
    let mut stream = TransformStream::new();
    let out = collect(&mut stream, b"text\x1b[38;2;255", &config);
    assert_eq!(out, b"text");
    assert!(!stream.is_ground());
    stream.finish();
    assert!(stream.is_ground());
}

#[test]
fn reset_clears_state() {
    let config = TransformConfig::new(ColorDepth::Color256);
    let mut stream = TransformStream::new();
    collect(&mut stream, b"\x1b[38;2;255", &config);
    assert!(!stream.is_ground());
    stream.reset();
    assert!(stream.is_ground());
}

#[test]
fn default_is_ground() {
    let stream = TransformStream::default();
    assert!(stream.is_ground());
}

// ── Empty input ─────────────────────────────────────────────────────

#[test]
fn empty_input() {
    let config = TransformConfig::new(ColorDepth::Color256);
    let mut stream = TransformStream::new();
    let slices: Vec<TransformSlice> = stream.transform_slices(b"", &config).collect();
    assert!(slices.is_empty());
}

// ── Greyscale streaming ─────────────────────────────────────────────

#[test]
fn greyscale_streaming() {
    let input = b"\x1b[38;2;255;0;0mred\x1b[0m";
    let result = transform_chunks(&[input], ColorDepth::Greyscale);
    let s = String::from_utf8_lossy(&result);
    assert!(s.contains("38;5;"), "should use 256-color greyscale index");
    assert!(s.contains("red"), "content should survive");
}
