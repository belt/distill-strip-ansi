//! SGR parameter parser and rewriter for color transforms.
//!
//! Parses the parameter bytes of a CSI SGR sequence (`ESC[...m`),
//! identifies color subsequences (`38;2;R;G;B`, `48;5;N`, `30-37`,
//! etc.), and rewrites them according to a target [`ColorDepth`].
//!
//! Gated behind the `transform` feature.

#![forbid(unsafe_code)]

use alloc::vec::Vec;

use crate::downgrade::ColorDepth;

/// Rewrite SGR parameters in a complete CSI SGR sequence to the
/// target color depth.
///
/// `seq` must be a complete SGR sequence: `ESC [ <params> m`.
/// Returns a new sequence with color params rewritten. Non-color
/// params (bold, italic, underline, etc.) pass through unchanged.
///
/// If no rewriting is needed (all colors already within target
/// depth), returns a clone of the input.
pub fn rewrite_sgr_params(seq: &[u8], target: ColorDepth) -> Vec<u8> {
    if target == ColorDepth::Truecolor {
        return seq.to_vec();
    }
    // Extract param bytes between ESC[ and m.
    debug_assert!(seq.len() >= 3 && seq[0] == 0x1B && seq[1] == b'[');
    let params_end = seq.len() - 1; // index of 'm'
    let param_bytes = &seq[2..params_end];

    let params = parse_params(param_bytes);
    let rewritten = rewrite_params(&params, target);
    emit_sgr(&rewritten)
}

/// A parsed SGR parameter value.
#[derive(Clone, Debug, PartialEq, Eq)]
enum SgrParam {
    /// A simple numeric param (e.g. `1` for bold, `31` for red fg).
    Simple(u16),
    /// Foreground 256-color: `38;5;N`.
    Fg256(u8),
    /// Background 256-color: `48;5;N`.
    Bg256(u8),
    /// Foreground truecolor: `38;2;R;G;B`.
    FgRgb(u8, u8, u8),
    /// Background truecolor: `48;2;R;G;B`.
    BgRgb(u8, u8, u8),
}

/// Parse semicolon-separated SGR params into structured form.
fn parse_params(bytes: &[u8]) -> Vec<SgrParam> {
    let nums = split_params(bytes);
    let mut result = Vec::new();
    let mut i = 0;
    while i < nums.len() {
        match nums[i] {
            38 if i + 1 < nums.len() && nums[i + 1] == 2 => {
                if i + 4 < nums.len() {
                    let r = nums[i + 2].min(255) as u8;
                    let g = nums[i + 3].min(255) as u8;
                    let b = nums[i + 4].min(255) as u8;
                    result.push(SgrParam::FgRgb(r, g, b));
                    i += 5;
                } else {
                    // Malformed — emit as simple params.
                    result.push(SgrParam::Simple(38));
                    i += 1;
                }
            }
            48 if i + 1 < nums.len() && nums[i + 1] == 2 => {
                if i + 4 < nums.len() {
                    let r = nums[i + 2].min(255) as u8;
                    let g = nums[i + 3].min(255) as u8;
                    let b = nums[i + 4].min(255) as u8;
                    result.push(SgrParam::BgRgb(r, g, b));
                    i += 5;
                } else {
                    result.push(SgrParam::Simple(48));
                    i += 1;
                }
            }
            38 if i + 1 < nums.len() && nums[i + 1] == 5 => {
                if i + 2 < nums.len() {
                    result.push(SgrParam::Fg256(nums[i + 2].min(255) as u8));
                    i += 3;
                } else {
                    result.push(SgrParam::Simple(38));
                    i += 1;
                }
            }
            48 if i + 1 < nums.len() && nums[i + 1] == 5 => {
                if i + 2 < nums.len() {
                    result.push(SgrParam::Bg256(nums[i + 2].min(255) as u8));
                    i += 3;
                } else {
                    result.push(SgrParam::Simple(48));
                    i += 1;
                }
            }
            n => {
                result.push(SgrParam::Simple(n));
                i += 1;
            }
        }
    }
    result
}

/// Split param bytes on `;` into numeric values.
fn split_params(bytes: &[u8]) -> Vec<u16> {
    if bytes.is_empty() {
        return Vec::new();
    }
    let mut result = Vec::new();
    let mut acc: u16 = 0;
    let mut has_digit = false;
    for &b in bytes {
        if b == b';' {
            result.push(if has_digit { acc } else { 0 });
            acc = 0;
            has_digit = false;
        } else if b.is_ascii_digit() {
            acc = acc.saturating_mul(10).saturating_add((b - b'0') as u16);
            has_digit = true;
        }
    }
    result.push(if has_digit { acc } else { 0 });
    result
}

/// Returns true if a simple param code is a color-setting code.
fn is_color_param(code: u16) -> bool {
    matches!(code, 30..=37 | 38 | 39 | 40..=47 | 48 | 49 | 90..=97 | 100..=107)
}

/// Rewrite parsed params to the target depth.
fn rewrite_params(params: &[SgrParam], target: ColorDepth) -> Vec<SgrParam> {
    use crate::downgrade::{nearest_256, nearest_16, nearest_greyscale};

    let mut out = Vec::with_capacity(params.len());
    for p in params {
        match (p, target) {
            // ── Mono: strip all color params ────────────────────
            (SgrParam::FgRgb(..) | SgrParam::BgRgb(..)
             | SgrParam::Fg256(_) | SgrParam::Bg256(_), ColorDepth::Mono) => {
                // Drop color param entirely.
            }
            (SgrParam::Simple(code), ColorDepth::Mono) if is_color_param(*code) => {
                // Drop basic color params.
            }

            // ── Greyscale: convert to greyscale ramp index ──────
            (SgrParam::FgRgb(r, g, b), ColorDepth::Greyscale) => {
                out.push(SgrParam::Fg256(nearest_greyscale(*r, *g, *b)));
            }
            (SgrParam::BgRgb(r, g, b), ColorDepth::Greyscale) => {
                out.push(SgrParam::Bg256(nearest_greyscale(*r, *g, *b)));
            }
            (SgrParam::Fg256(idx), ColorDepth::Greyscale) => {
                let (r, g, b) = idx_to_rgb(*idx);
                out.push(SgrParam::Fg256(nearest_greyscale(r, g, b)));
            }
            (SgrParam::Bg256(idx), ColorDepth::Greyscale) => {
                let (r, g, b) = idx_to_rgb(*idx);
                out.push(SgrParam::Bg256(nearest_greyscale(r, g, b)));
            }
            (SgrParam::Simple(code), ColorDepth::Greyscale) if is_fg_basic(*code) => {
                let (r, g, b) = basic_to_rgb(*code);
                out.push(SgrParam::Fg256(nearest_greyscale(r, g, b)));
            }
            (SgrParam::Simple(code), ColorDepth::Greyscale) if is_bg_basic(*code) => {
                let (r, g, b) = basic_to_rgb(*code);
                out.push(SgrParam::Bg256(nearest_greyscale(r, g, b)));
            }

            // ── Color16: downgrade to basic ANSI ────────────────
            (SgrParam::FgRgb(r, g, b), ColorDepth::Color16) => {
                let idx = nearest_256(*r, *g, *b);
                let basic = nearest_16(idx);
                out.push(SgrParam::Simple(basic_idx_to_fg(basic)));
            }
            (SgrParam::BgRgb(r, g, b), ColorDepth::Color16) => {
                let idx = nearest_256(*r, *g, *b);
                let basic = nearest_16(idx);
                out.push(SgrParam::Simple(basic_idx_to_bg(basic)));
            }
            (SgrParam::Fg256(idx), ColorDepth::Color16) => {
                let basic = nearest_16(*idx);
                out.push(SgrParam::Simple(basic_idx_to_fg(basic)));
            }
            (SgrParam::Bg256(idx), ColorDepth::Color16) => {
                let basic = nearest_16(*idx);
                out.push(SgrParam::Simple(basic_idx_to_bg(basic)));
            }
            // Basic colors already at 16 — pass through.

            // ── Color256: downgrade truecolor only ──────────────
            (SgrParam::FgRgb(r, g, b), ColorDepth::Color256) => {
                out.push(SgrParam::Fg256(nearest_256(*r, *g, *b)));
            }
            (SgrParam::BgRgb(r, g, b), ColorDepth::Color256) => {
                out.push(SgrParam::Bg256(nearest_256(*r, *g, *b)));
            }
            // 256-color and basic already within target — pass through.

            // ── Default: pass through ───────────────────────────
            (param, _) => {
                out.push(param.clone());
            }
        }
    }
    out
}

/// Emit rewritten params as a complete SGR sequence.
fn emit_sgr(params: &[SgrParam]) -> Vec<u8> {
    let mut out = Vec::with_capacity(16);
    out.push(0x1B);
    out.push(b'[');
    for (i, p) in params.iter().enumerate() {
        if i > 0 {
            out.push(b';');
        }
        match p {
            SgrParam::Simple(n) => write_num(&mut out, *n),
            SgrParam::Fg256(idx) => {
                out.extend_from_slice(b"38;5;");
                write_num(&mut out, *idx as u16);
            }
            SgrParam::Bg256(idx) => {
                out.extend_from_slice(b"48;5;");
                write_num(&mut out, *idx as u16);
            }
            SgrParam::FgRgb(r, g, b) => {
                out.extend_from_slice(b"38;2;");
                write_num(&mut out, *r as u16);
                out.push(b';');
                write_num(&mut out, *g as u16);
                out.push(b';');
                write_num(&mut out, *b as u16);
            }
            SgrParam::BgRgb(r, g, b) => {
                out.extend_from_slice(b"48;2;");
                write_num(&mut out, *r as u16);
                out.push(b';');
                write_num(&mut out, *g as u16);
                out.push(b';');
                write_num(&mut out, *b as u16);
            }
        }
    }
    out.push(b'm');
    out
}

/// Write a u16 as ASCII decimal digits.
fn write_num(out: &mut Vec<u8>, n: u16) {
    if n == 0 {
        out.push(b'0');
        return;
    }
    let mut buf = [0u8; 5];
    let mut pos = buf.len();
    let mut val = n;
    while val > 0 {
        pos -= 1;
        buf[pos] = b'0' + (val % 10) as u8;
        val /= 10;
    }
    out.extend_from_slice(&buf[pos..]);
}

// ── Helpers ─────────────────────────────────────────────────────────

fn is_fg_basic(code: u16) -> bool {
    matches!(code, 30..=37 | 90..=97)
}

fn is_bg_basic(code: u16) -> bool {
    matches!(code, 40..=47 | 100..=107)
}

/// Convert a 256-color index to RGB.
fn idx_to_rgb(idx: u8) -> (u8, u8, u8) {
    match idx {
        0..=15 => basic_idx_to_rgb(idx),
        16..=231 => crate::downgrade::cube_to_rgb(idx),
        232..=255 => {
            let v = crate::downgrade::grey_index_to_value(idx);
            (v, v, v)
        }
    }
}

/// Convert a basic color index (0-15) to RGB (xterm defaults).
fn basic_idx_to_rgb(idx: u8) -> (u8, u8, u8) {
    crate::downgrade::BASIC_COLORS[idx as usize]
}

/// Convert a basic SGR fg/bg code to RGB.
fn basic_to_rgb(code: u16) -> (u8, u8, u8) {
    let idx = match code {
        30..=37 => (code - 30) as u8,
        40..=47 => (code - 40) as u8,
        90..=97 => (code - 90 + 8) as u8,
        100..=107 => (code - 100 + 8) as u8,
        _ => 0,
    };
    basic_idx_to_rgb(idx)
}

/// Convert a basic color index (0-15) to an SGR foreground code.
fn basic_idx_to_fg(idx: u8) -> u16 {
    if idx < 8 { 30 + idx as u16 } else { 90 + (idx - 8) as u16 }
}

/// Convert a basic color index (0-15) to an SGR background code.
fn basic_idx_to_bg(idx: u8) -> u16 {
    if idx < 8 { 40 + idx as u16 } else { 100 + (idx - 8) as u16 }
}
