//! Color depth reduction algorithms.
//!
//! Implements the conversion paths described in
//! `doc/COLOR-TRANSFORMS.md`:
//! - Truecolor → 256 (nearest cube or greyscale)
//! - Truecolor → 16 (nearest basic ANSI)
//! - 256 → 16 (lookup)
//! - Any → greyscale (Rec. 709 luminance)
//! - Any → mono (strip color params)
//!
//! Gated behind the `downgrade-color` feature.

#![forbid(unsafe_code)]

/// Target color depth for downgrading.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ColorDepth {
    /// Strip all color, keep styles.
    Mono = 0,
    /// Map to 24-shade greyscale ramp (232-255).
    Greyscale = 1,
    /// Map to basic 16 ANSI colors.
    Color16 = 2,
    /// Map to 256-color palette.
    Color256 = 3,
    /// No downgrade (pass through).
    Truecolor = 4,
}

/// The six axis values of the 6x6x6 color cube.
const AXIS: [u8; 6] = [0x00, 0x5F, 0x87, 0xAF, 0xD7, 0xFF];

/// Xterm default RGB values for the 16 basic ANSI colors.
pub const BASIC_COLORS: [(u8, u8, u8); 16] = [
    (0, 0, 0),       // 0  black
    (128, 0, 0),     // 1  red
    (0, 128, 0),     // 2  green
    (128, 128, 0),   // 3  yellow
    (0, 0, 128),     // 4  blue
    (128, 0, 128),   // 5  magenta
    (0, 128, 128),   // 6  cyan
    (192, 192, 192), // 7  white
    (128, 128, 128), // 8  bright black
    (255, 0, 0),     // 9  bright red
    (0, 255, 0),     // 10 bright green
    (255, 255, 0),   // 11 bright yellow
    (0, 0, 255),     // 12 bright blue
    (255, 0, 255),   // 13 bright magenta
    (0, 255, 255),   // 14 bright cyan
    (255, 255, 255), // 15 bright white
];

/// Precomputed lookup: byte value (0-255) → nearest 6x6x6 cube axis index (0-5).
/// Eliminates branching from the hot path. 256 bytes.
#[rustfmt::skip]
static NEAREST_AXIS_TABLE: [u8; 256] = {
    let mut t = [0u8; 256];
    // 0..=47 → 0, 48..=114 → 1, 115..=154 → 2,
    // 155..=194 → 3, 195..=234 → 4, 235..=255 → 5
    let mut i = 0;
    while i < 256 {
        t[i] = if i <= 47 { 0 }
            else if i <= 114 { 1 }
            else if i <= 154 { 2 }
            else if i <= 194 { 3 }
            else if i <= 234 { 4 }
            else { 5 };
        i += 1;
    }
    t
};

/// Quantize a 0-255 value to the nearest 6x6x6 cube axis index (0-5).
/// Single array lookup — no branches.
#[inline]
#[must_use]
pub fn nearest_axis(v: u8) -> u8 {
    NEAREST_AXIS_TABLE[v as usize]
}

/// Convert a 256-color cube index (16-231) to its RGB components.
#[inline]
#[must_use]
pub fn cube_to_rgb(idx: u8) -> (u8, u8, u8) {
    let i = idx - 16;
    let r = AXIS[(i / 36) as usize];
    let g = AXIS[((i % 36) / 6) as usize];
    let b = AXIS[(i % 6) as usize];
    (r, g, b)
}

/// Convert a greyscale ramp index (232-255) to its grey value.
#[inline]
#[must_use]
pub fn grey_index_to_value(idx: u8) -> u8 {
    (8 + 10 * (idx as u16 - 232)) as u8
}

/// Map an RGB truecolor value to the nearest 256-color index (16-255).
///
/// Compares the nearest cube vertex and nearest greyscale ramp entry,
/// returning whichever has smaller squared Euclidean distance.
#[must_use]
pub fn nearest_256(r: u8, g: u8, b: u8) -> u8 {
    // Cube candidate.
    let ri = nearest_axis(r);
    let gi = nearest_axis(g);
    let bi = nearest_axis(b);
    let cube_idx = 16 + 36 * ri + 6 * gi + bi;
    let cr = AXIS[ri as usize] as i32;
    let cg = AXIS[gi as usize] as i32;
    let cb = AXIS[bi as usize] as i32;
    let d_cube = sq(r as i32 - cr) + sq(g as i32 - cg) + sq(b as i32 - cb);

    // Greyscale candidate: simple average for ramp placement.
    let avg = ((r as u16 + g as u16 + b as u16) / 3) as i32;
    let gi_raw = (avg - 8 + 5) / 10; // +5 for rounding
    let gi_clamped = gi_raw.clamp(0, 23) as u8;
    let grey_idx = 232 + gi_clamped;
    let gv = 8 + 10 * gi_clamped as i32;
    let d_grey = sq(r as i32 - gv) + sq(g as i32 - gv) + sq(b as i32 - gv);

    if d_grey < d_cube { grey_idx } else { cube_idx }
}

/// Precomputed lookup: 256-color index → nearest basic 16-color index.
/// Eliminates cube_to_rgb + nearest_basic computation from the hot path.
static NEAREST_16_TABLE: [u8; 256] = {
    let mut t = [0u8; 256];
    // 0..=15: identity
    let mut i = 0u16;
    while i < 16 {
        t[i as usize] = i as u8;
        i += 1;
    }
    // 16..=231: cube indices — compute nearest basic at compile time
    while i < 232 {
        let ci = (i - 16) as u8;
        let r = AXIS[(ci / 36) as usize];
        let g = AXIS[((ci % 36) / 6) as usize];
        let b = AXIS[(ci % 6) as usize];
        t[i as usize] = nearest_basic_const(r, g, b);
        i += 1;
    }
    // 232..=255: greyscale ramp
    while i < 256 {
        let v = (8 + 10 * (i - 232)) as u8;
        t[i as usize] = nearest_basic_const(v, v, v);
        i += 1;
    }
    t
};

/// Const-evaluable nearest basic color (0-15) by squared Euclidean distance.
const fn nearest_basic_const(r: u8, g: u8, b: u8) -> u8 {
    let mut best = 0u8;
    let mut best_dist = i32::MAX;
    let mut i = 0;
    while i < 16 {
        let (br, bg, bb) = BASIC_COLORS[i];
        let d = sq(r as i32 - br as i32) + sq(g as i32 - bg as i32) + sq(b as i32 - bb as i32);
        if d < best_dist {
            best_dist = d;
            best = i as u8;
        }
        i += 1;
    }
    best
}

/// Map a 256-color index to the nearest basic 16-color index.
/// Single array lookup — no branches, no computation.
#[must_use]
pub fn nearest_16(idx: u8) -> u8 {
    NEAREST_16_TABLE[idx as usize]
}

/// Map an RGB truecolor value to the nearest greyscale ramp index (232-255).
///
/// Uses Rec. 709 luminance weighting.
#[must_use]
pub fn nearest_greyscale(r: u8, g: u8, b: u8) -> u8 {
    // Y = 0.2126*R + 0.7152*G + 0.0722*B
    // Use fixed-point: multiply by 10000, divide at end.
    let y = (2126 * r as u32 + 7152 * g as u32 + 722 * b as u32 + 5000) / 10000;
    let y = y as i32;
    let gi_raw = (y - 8 + 5) / 10; // +5 for rounding
    let gi_clamped = gi_raw.clamp(0, 23) as u8;
    232 + gi_clamped
}

#[inline]
const fn sq(x: i32) -> i32 {
    x * x
}
