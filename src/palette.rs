//! Color palette remapping via 3x3 matrix transforms.
//!
//! Operates in linear RGB (sRGB decoded). The pipeline:
//! 1. sRGB decode (linearize)
//! 2. Apply 3x3 matrix
//! 3. Clamp to [0, 1]
//! 4. sRGB encode (gamma compress)
//!
//! Palette names are neutral — no medical terminology.
//! See `doc/COLOR-TRANSFORMS.md` for design rationale.
//!
//! Gated behind the `color-palette` feature.

#![forbid(unsafe_code)]

/// 3x3 matrix type: `[[f64; 3]; 3]`, row-major.
pub type Matrix3 = [[f64; 3]; 3];

/// Identity matrix — no color change.
pub const IDENTITY_MATRIX: Matrix3 = [
    [1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 0.0, 1.0],
];

/// Protanopia simulation matrix (Viénot et al. 1999, linear RGB).
pub const PROTANOPIA_VIENOT: Matrix3 = [
    [0.10889, 0.89111, -0.00000],
    [0.10889, 0.89111,  0.00000],
    [0.00447, -0.00447, 1.00000],
];

/// Deuteranopia simulation matrix (Viénot et al. 1999, linear RGB).
pub const DEUTERANOPIA_VIENOT: Matrix3 = [
    [ 0.29275, 0.70725,  0.00000],
    [ 0.29275, 0.70725, -0.00000],
    [-0.02234, 0.02234,  1.00000],
];

// ── sRGB Linearization (IEC 61966-2-1) ─────────────────────────────

/// Decode an sRGB byte (0-255) to linear light (0.0-1.0).
#[inline]
#[must_use]
pub fn srgb_to_linear(v: u8) -> f64 {
    let c = v as f64 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Encode a linear light value (0.0-1.0) to an sRGB byte (0-255).
#[inline]
#[must_use]
pub fn linear_to_srgb(c: f64) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0 + 0.5) as u8
}

/// Apply a 3x3 matrix to a linear RGB triplet.
#[inline]
#[must_use]
pub fn apply_matrix(m: &Matrix3, rgb: &[f64; 3]) -> [f64; 3] {
    [
        m[0][0] * rgb[0] + m[0][1] * rgb[1] + m[0][2] * rgb[2],
        m[1][0] * rgb[0] + m[1][1] * rgb[1] + m[1][2] * rgb[2],
        m[2][0] * rgb[0] + m[2][1] * rgb[1] + m[2][2] * rgb[2],
    ]
}

// ── PaletteTransform ────────────────────────────────────────────────

/// A color palette transform that operates on sRGB byte values.
///
/// Wraps a 3x3 matrix applied in linear RGB space with automatic
/// sRGB decode/encode and clamping.
#[derive(Clone, Debug)]
pub struct PaletteTransform {
    matrix: Matrix3,
}

impl PaletteTransform {
    /// Create a transform from a 3x3 matrix (linear RGB space).
    #[must_use]
    pub fn from_matrix(matrix: Matrix3) -> Self {
        Self { matrix }
    }

    /// Transform an sRGB color through this palette.
    ///
    /// Decodes to linear, applies the matrix, clamps, encodes back.
    #[must_use]
    pub fn transform(&self, r: u8, g: u8, b: u8) -> (u8, u8, u8) {
        let lin = [
            srgb_to_linear(r),
            srgb_to_linear(g),
            srgb_to_linear(b),
        ];
        let out = apply_matrix(&self.matrix, &lin);
        (
            linear_to_srgb(out[0]),
            linear_to_srgb(out[1]),
            linear_to_srgb(out[2]),
        )
    }

    /// Returns `true` if this is the identity transform.
    #[must_use]
    pub fn is_identity(&self) -> bool {
        self.matrix == IDENTITY_MATRIX
    }
}

impl Default for PaletteTransform {
    /// Default is the identity transform (no color change).
    fn default() -> Self {
        Self { matrix: IDENTITY_MATRIX }
    }
}
