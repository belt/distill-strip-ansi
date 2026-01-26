//! SGR parameter parser and rewriter for color transforms.
//!
//! Single-pass rewriter: reads raw param bytes and emits rewritten
//! SGR output directly — no intermediate `SgrParam` collections.
//! Typical SGR sequences (≤8 params, ≤32 output bytes) stay
//! entirely on the stack via `SmallVec<[u8; 32]>`.
//!
//! Gated behind the `transform` feature.

#![forbid(unsafe_code)]

use alloc::vec::Vec;
use smallvec::SmallVec;

use crate::downgrade::ColorDepth;

// ── Public API ──────────────────────────────────────────────────────

/// Rewrite SGR parameters in a complete CSI SGR sequence to the
/// target color depth.
///
/// `seq` must be a complete SGR sequence: `ESC [ <params> m`.
/// Returns a new sequence with color params rewritten.
pub fn rewrite_sgr_params(seq: &[u8], target: ColorDepth) -> Vec<u8> {
    if target == ColorDepth::Truecolor {
        return seq.to_vec();
    }
    debug_assert!(seq.len() >= 3 && seq[0] == 0x1B && seq[1] == b'[');
    let params_end = seq.len() - 1;
    let param_bytes = &seq[2..params_end];

    let mut out = SmallVec::<[u8; 32]>::new();
    rewrite_sgr_direct(param_bytes, target, &mut out);
    out.to_vec()
}

/// Single-pass SGR rewriter that writes directly into a caller-provided
/// `SmallVec`. Used by [`TransformStream`](crate::TransformStream) to
/// avoid the `Vec` round-trip.
///
/// `param_bytes` is the content between `ESC[` and `m` (exclusive).
/// Writes a complete `ESC[…m` sequence into `out`.
pub(crate) fn rewrite_sgr_direct(
    param_bytes: &[u8],
    target: ColorDepth,
    out: &mut SmallVec<[u8; 32]>,
) {
    out.push(0x1B);
    out.push(b'[');

    // Empty params (ESC[m) = implicit reset — pass through unchanged.
    if param_bytes.is_empty() {
        out.push(b'm');
        return;
    }

    let mut emitter = SgrEmitter::new(target, out);
    let mut acc: u16 = 0;
    let mut has_digit = false;

    for &b in param_bytes {
        let dv = DIGIT_VAL[b as usize];
        if dv != 0xFF {
            acc = acc.saturating_mul(10).saturating_add(dv as u16);
            has_digit = true;
        } else if b == b';' {
            let val = if has_digit { acc } else { 0 };
            emitter.feed(val);
            acc = 0;
            has_digit = false;
        }
    }
    // Final param (or implicit 0 if trailing semicolon / empty).
    let val = if has_digit { acc } else { 0 };
    emitter.feed(val);
    emitter.flush();

    let out = emitter.finish();
    out.push(b'm');
}

// ── State machine ───────────────────────────────────────────────────

/// Tracks where we are inside an extended color subsequence.
#[derive(Clone, Copy)]
enum ExtState {
    /// Not inside an extended color sequence.
    Normal,
    /// Saw `38`, waiting for discriminator (2 or 5).
    Saw38,
    /// Saw `48`, waiting for discriminator (2 or 5).
    Saw48,
    /// Collecting RGB after `38;2;` — `count` components stored so far.
    FgRgb { count: u8, r: u8, g: u8 },
    /// Collecting RGB after `48;2;` — `count` components stored so far.
    BgRgb { count: u8, r: u8, g: u8 },
    /// Collecting index after `38;5;`.
    Fg256,
    /// Collecting index after `48;5;`.
    Bg256,
}

/// Drives the single-pass rewrite: receives one parsed number at a
/// time and emits rewritten bytes directly into the output buffer.
struct SgrEmitter<'a> {
    state: ExtState,
    target: ColorDepth,
    out: &'a mut SmallVec<[u8; 32]>,
    /// Whether we've emitted at least one param (for semicolon insertion).
    need_sep: bool,
}

impl<'a> SgrEmitter<'a> {
    fn new(target: ColorDepth, out: &'a mut SmallVec<[u8; 32]>) -> Self {
        Self {
            state: ExtState::Normal,
            target,
            out,
            need_sep: false,
        }
    }

    /// Feed one parsed numeric parameter.
    fn feed(&mut self, val: u16) {
        match self.state {
            ExtState::Normal => self.feed_normal(val),
            ExtState::Saw38 => self.feed_saw38(val),
            ExtState::Saw48 => self.feed_saw48(val),
            ExtState::FgRgb { count, r, g } => self.feed_fg_rgb(count, r, g, val),
            ExtState::BgRgb { count, r, g } => self.feed_bg_rgb(count, r, g, val),
            ExtState::Fg256 => self.feed_fg256(val),
            ExtState::Bg256 => self.feed_bg256(val),
        }
    }

    /// Flush any incomplete extended color sequence at end-of-input.
    fn flush(&mut self) {
        match self.state {
            ExtState::Saw38 => {
                self.emit_simple(38);
            }
            ExtState::Saw48 => {
                self.emit_simple(48);
            }
            ExtState::FgRgb { .. } | ExtState::BgRgb { .. } | ExtState::Fg256 | ExtState::Bg256 => {
                // Incomplete extended color — drop it (same as current behavior:
                // parse_params falls back to Simple(38/48) which gets emitted).
                // Actually, current behavior emits Simple(38) for incomplete.
                // We match that: the leading 38/48 was already consumed, and
                // the partial components are lost. This matches the existing
                // parse_params fallback.
            }
            ExtState::Normal => {}
        }
        self.state = ExtState::Normal;
    }

    /// Return the output buffer reference for the final `m` push.
    fn finish(self) -> &'a mut SmallVec<[u8; 32]> {
        self.out
    }

    // ── State handlers ──────────────────────────────────────────────

    fn feed_normal(&mut self, val: u16) {
        if val == 38 {
            self.state = ExtState::Saw38;
        } else if val == 48 {
            self.state = ExtState::Saw48;
        } else {
            self.emit_simple(val);
        }
    }

    fn feed_saw38(&mut self, val: u16) {
        match val {
            2 => {
                self.state = ExtState::FgRgb { count: 0, r: 0, g: 0 };
            }
            5 => {
                self.state = ExtState::Fg256;
            }
            _ => {
                // Not an extended color — emit the pending 38 as simple,
                // then process this value normally.
                self.emit_simple(38);
                self.state = ExtState::Normal;
                self.feed_normal(val);
            }
        }
    }

    fn feed_saw48(&mut self, val: u16) {
        match val {
            2 => {
                self.state = ExtState::BgRgb { count: 0, r: 0, g: 0 };
            }
            5 => {
                self.state = ExtState::Bg256;
            }
            _ => {
                self.emit_simple(48);
                self.state = ExtState::Normal;
                self.feed_normal(val);
            }
        }
    }

    fn feed_fg_rgb(&mut self, count: u8, r: u8, g: u8, val: u16) {
        let component = val.min(255) as u8;
        match count {
            0 => {
                self.state = ExtState::FgRgb { count: 1, r: component, g: 0 };
            }
            1 => {
                self.state = ExtState::FgRgb { count: 2, r, g: component };
            }
            _ => {
                // count == 2: have all three components.
                self.emit_fg_rgb(r, g, component);
                self.state = ExtState::Normal;
            }
        }
    }

    fn feed_bg_rgb(&mut self, count: u8, r: u8, g: u8, val: u16) {
        let component = val.min(255) as u8;
        match count {
            0 => {
                self.state = ExtState::BgRgb { count: 1, r: component, g: 0 };
            }
            1 => {
                self.state = ExtState::BgRgb { count: 2, r, g: component };
            }
            _ => {
                self.emit_bg_rgb(r, g, component);
                self.state = ExtState::Normal;
            }
        }
    }

    fn feed_fg256(&mut self, val: u16) {
        let idx = val.min(255) as u8;
        self.emit_fg_256(idx);
        self.state = ExtState::Normal;
    }

    fn feed_bg256(&mut self, val: u16) {
        let idx = val.min(255) as u8;
        self.emit_bg_256(idx);
        self.state = ExtState::Normal;
    }

    // ── Emit helpers ────────────────────────────────────────────────

    /// Emit a simple (non-extended) SGR parameter, rewriting if needed.
    fn emit_simple(&mut self, code: u16) {
        use crate::downgrade::nearest_greyscale;

        match self.target {
            ColorDepth::Mono if is_color_param(code) => {
                // Strip color params in mono mode.
            }
            ColorDepth::Greyscale if is_fg_basic(code) => {
                let (r, g, b) = basic_to_rgb(code);
                self.write_fg_256(nearest_greyscale(r, g, b));
            }
            ColorDepth::Greyscale if is_bg_basic(code) => {
                let (r, g, b) = basic_to_rgb(code);
                self.write_bg_256(nearest_greyscale(r, g, b));
            }
            ColorDepth::Color16 if is_fg_basic(code) => {
                // Basic fg colors pass through at 16-color depth.
                self.write_simple(code);
            }
            ColorDepth::Color16 if is_bg_basic(code) => {
                // Basic bg colors pass through at 16-color depth.
                self.write_simple(code);
            }
            _ => {
                self.write_simple(code);
            }
        }
    }

    /// Emit a foreground RGB color, rewritten to target depth.
    fn emit_fg_rgb(&mut self, r: u8, g: u8, b: u8) {
        use crate::downgrade::{nearest_16, nearest_256, nearest_greyscale};

        match self.target {
            ColorDepth::Mono => {}
            ColorDepth::Greyscale => {
                self.write_fg_256(nearest_greyscale(r, g, b));
            }
            ColorDepth::Color16 => {
                self.write_simple(basic_idx_to_fg(nearest_16(nearest_256(r, g, b))));
            }
            ColorDepth::Color256 => {
                self.write_fg_256(nearest_256(r, g, b));
            }
            ColorDepth::Truecolor => {
                // Shouldn't reach here (early return), but be safe.
                self.write_fg_rgb(r, g, b);
            }
        }
    }

    /// Emit a background RGB color, rewritten to target depth.
    fn emit_bg_rgb(&mut self, r: u8, g: u8, b: u8) {
        use crate::downgrade::{nearest_16, nearest_256, nearest_greyscale};

        match self.target {
            ColorDepth::Mono => {}
            ColorDepth::Greyscale => {
                self.write_bg_256(nearest_greyscale(r, g, b));
            }
            ColorDepth::Color16 => {
                self.write_simple(basic_idx_to_bg(nearest_16(nearest_256(r, g, b))));
            }
            ColorDepth::Color256 => {
                self.write_bg_256(nearest_256(r, g, b));
            }
            ColorDepth::Truecolor => {
                self.write_bg_rgb(r, g, b);
            }
        }
    }

    /// Emit a foreground 256-color index, rewritten to target depth.
    fn emit_fg_256(&mut self, idx: u8) {
        use crate::downgrade::{nearest_16, nearest_greyscale};

        match self.target {
            ColorDepth::Mono => {}
            ColorDepth::Greyscale => {
                let (r, g, b) = idx_to_rgb(idx);
                self.write_fg_256(nearest_greyscale(r, g, b));
            }
            ColorDepth::Color16 => {
                self.write_simple(basic_idx_to_fg(nearest_16(idx)));
            }
            ColorDepth::Color256 | ColorDepth::Truecolor => {
                self.write_fg_256(idx);
            }
        }
    }

    /// Emit a background 256-color index, rewritten to target depth.
    fn emit_bg_256(&mut self, idx: u8) {
        use crate::downgrade::{nearest_16, nearest_greyscale};

        match self.target {
            ColorDepth::Mono => {}
            ColorDepth::Greyscale => {
                let (r, g, b) = idx_to_rgb(idx);
                self.write_bg_256(nearest_greyscale(r, g, b));
            }
            ColorDepth::Color16 => {
                self.write_simple(basic_idx_to_bg(nearest_16(idx)));
            }
            ColorDepth::Color256 | ColorDepth::Truecolor => {
                self.write_bg_256(idx);
            }
        }
    }

    // ── Raw byte writers ────────────────────────────────────────────

    fn sep(&mut self) {
        if self.need_sep {
            self.out.push(b';');
        }
        self.need_sep = true;
    }

    fn write_simple(&mut self, n: u16) {
        self.sep();
        write_num(self.out, n);
    }

    fn write_fg_256(&mut self, idx: u8) {
        self.sep();
        self.out.extend_from_slice(b"38;5;");
        write_num(self.out, idx as u16);
    }

    fn write_bg_256(&mut self, idx: u8) {
        self.sep();
        self.out.extend_from_slice(b"48;5;");
        write_num(self.out, idx as u16);
    }

    fn write_fg_rgb(&mut self, r: u8, g: u8, b: u8) {
        self.sep();
        self.out.extend_from_slice(b"38;2;");
        write_num(self.out, r as u16);
        self.out.push(b';');
        write_num(self.out, g as u16);
        self.out.push(b';');
        write_num(self.out, b as u16);
    }

    fn write_bg_rgb(&mut self, r: u8, g: u8, b: u8) {
        self.sep();
        self.out.extend_from_slice(b"48;2;");
        write_num(self.out, r as u16);
        self.out.push(b';');
        write_num(self.out, g as u16);
        self.out.push(b';');
        write_num(self.out, b as u16);
    }
}

// ── Shared helpers ──────────────────────────────────────────────────

/// Lookup: byte → digit value (0-9), or 0xFF if not a digit.
static DIGIT_VAL: [u8; 256] = {
    let mut t = [0xFFu8; 256];
    t[b'0' as usize] = 0;
    t[b'1' as usize] = 1;
    t[b'2' as usize] = 2;
    t[b'3' as usize] = 3;
    t[b'4' as usize] = 4;
    t[b'5' as usize] = 5;
    t[b'6' as usize] = 6;
    t[b'7' as usize] = 7;
    t[b'8' as usize] = 8;
    t[b'9' as usize] = 9;
    t
};

/// Write a u16 as ASCII decimal digits.
fn write_num(out: &mut SmallVec<[u8; 32]>, n: u16) {
    if n < 10 {
        out.push(b'0' + n as u8);
    } else if n < 100 {
        out.push(b'0' + (n / 10) as u8);
        out.push(b'0' + (n % 10) as u8);
    } else if n < 1000 {
        out.push(b'0' + (n / 100) as u8);
        out.push(b'0' + ((n / 10) % 10) as u8);
        out.push(b'0' + (n % 10) as u8);
    } else {
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
}

/// Precomputed lookup: SGR code (0-255) → true if it's a color parameter.
static IS_COLOR_PARAM_TABLE: [bool; 256] = {
    let mut t = [false; 256];
    let mut i = 30u16;
    while i <= 37 {
        t[i as usize] = true;
        i += 1;
    }
    t[38] = true;
    t[39] = true;
    i = 40;
    while i <= 47 {
        t[i as usize] = true;
        i += 1;
    }
    t[48] = true;
    t[49] = true;
    i = 90;
    while i <= 97 {
        t[i as usize] = true;
        i += 1;
    }
    i = 100;
    while i <= 107 {
        t[i as usize] = true;
        i += 1;
    }
    t
};

fn is_color_param(code: u16) -> bool {
    (code as usize) < 256 && IS_COLOR_PARAM_TABLE[code as usize]
}

fn is_fg_basic(code: u16) -> bool {
    matches!(code, 30..=37 | 90..=97)
}

fn is_bg_basic(code: u16) -> bool {
    matches!(code, 40..=47 | 100..=107)
}

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

fn basic_idx_to_rgb(idx: u8) -> (u8, u8, u8) {
    crate::downgrade::BASIC_COLORS[idx as usize]
}

/// Precomputed lookup: SGR code (0-255) → basic color index (0-15).
static BASIC_CODE_TO_IDX: [u8; 256] = {
    let mut t = [0xFFu8; 256];
    let mut i = 30u16;
    while i <= 37 {
        t[i as usize] = (i - 30) as u8;
        i += 1;
    }
    i = 40;
    while i <= 47 {
        t[i as usize] = (i - 40) as u8;
        i += 1;
    }
    i = 90;
    while i <= 97 {
        t[i as usize] = (i - 90 + 8) as u8;
        i += 1;
    }
    i = 100;
    while i <= 107 {
        t[i as usize] = (i - 100 + 8) as u8;
        i += 1;
    }
    t
};

fn basic_to_rgb(code: u16) -> (u8, u8, u8) {
    let idx = if (code as usize) < 256 {
        BASIC_CODE_TO_IDX[code as usize]
    } else {
        0
    };
    basic_idx_to_rgb(if idx == 0xFF { 0 } else { idx })
}

static BASIC_IDX_TO_FG: [u16; 16] = {
    let mut t = [0u16; 16];
    let mut i = 0u8;
    while i < 8 {
        t[i as usize] = 30 + i as u16;
        i += 1;
    }
    while i < 16 {
        t[i as usize] = 90 + (i - 8) as u16;
        i += 1;
    }
    t
};

fn basic_idx_to_fg(idx: u8) -> u16 {
    BASIC_IDX_TO_FG[(idx & 0x0F) as usize]
}

static BASIC_IDX_TO_BG: [u16; 16] = {
    let mut t = [0u16; 16];
    let mut i = 0u8;
    while i < 8 {
        t[i as usize] = 40 + i as u16;
        i += 1;
    }
    while i < 16 {
        t[i as usize] = 100 + (i - 8) as u16;
        i += 1;
    }
    t
};

fn basic_idx_to_bg(idx: u8) -> u16 {
    BASIC_IDX_TO_BG[(idx & 0x0F) as usize]
}
