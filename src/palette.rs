//! Color palette remapping via 3x3 matrix transforms.
//!
//! Operates in linear RGB (sRGB decoded). The pipeline:
//! 1. sRGB decode via 256-entry lookup table (no `powf`)
//! 2. Apply 3x3 matrix
//! 3. Clamp to [0, 1]
//! 4. sRGB encode via binary search in reverse table
//!
//! Palette names are neutral — no medical terminology.
//! See `doc/COLOR-TRANSFORMS.md` for design rationale.
//!
//! Gated behind the `augment-color` feature.

#![forbid(unsafe_code)]

/// 3x3 matrix type: `[[f64; 3]; 3]`, row-major.
pub type Matrix3 = [[f64; 3]; 3];

/// Identity matrix — no color change.
pub const IDENTITY_MATRIX: Matrix3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// Protanopia simulation matrix (Viénot et al. 1999, linear RGB).
pub const PROTANOPIA_VIENOT: Matrix3 = [
    [0.108_89, 0.891_11, -0.0],
    [0.108_89, 0.891_11, 0.0],
    [0.004_47, -0.004_47, 1.0],
];

/// Deuteranopia simulation matrix (Viénot et al. 1999, linear RGB).
pub const DEUTERANOPIA_VIENOT: Matrix3 = [
    [0.292_75, 0.707_25, 0.0],
    [0.292_75, 0.707_25, -0.0],
    [-0.022_34, 0.022_34, 1.0],
];

/// Tritanopia simulation matrix (Brettel et al. 1997, H1 half-plane, linear RGB).
///
/// Models blue-yellow confusion (S cone absence). Uses the H1
/// half-plane which covers the majority of real-world colors.
/// Source: `DaltonLens` project, Brettel 1997 implementation.
pub const TRITANOPIA_BRETTEL_H1: Matrix3 = [
    [1.013_54, 0.142_68, -0.156_22],
    [-0.011_81, 0.875_61, 0.136_19],
    [0.077_07, 0.812_08, 0.110_85],
];

// ── sRGB Linearization via Lookup Table ─────────────────────────────

/// Precomputed sRGB byte (0-255) → linear light (0.0-1.0).
/// Eliminates `powf(2.4)` from the hot path. 2 KiB.
#[rustfmt::skip]
static SRGB_TO_LINEAR: [f64; 256] = [
    0.0, 0.000_303_526_983_5, 0.000_607_053_967_1, 0.000_910_580_950_6,
    0.001_214_107_934, 0.001_517_634_918, 0.001_821_161_901, 0.002_124_688_885,
    0.002_428_215_868, 0.002_731_742_852, 0.003_035_269_835, 0.003_346_535_764,
    0.003_676_507_324, 0.004_024_717_018, 0.004_391_442_037, 0.004_776_953_481,
    0.005_181_516_702, 0.005_605_391_624, 0.006_048_833_023, 0.006_512_090_793,
    0.006_995_410_187, 0.007_499_032_043, 0.008_023_192_985, 0.008_568_125_618,
    0.009_134_058_702, 0.009_721_217_32, 0.010_329_823_03, 0.010_960_094_01,
    0.011_612_245_18, 0.012_286_488_36, 0.012_983_032_34, 0.013_702_083_05,
    0.014_443_843_6, 0.015_208_514_42, 0.015_996_293_37, 0.016_807_375_75,
    0.017_641_954_49, 0.018_500_220_13, 0.019_382_360_96, 0.020_288_563_06,
    0.021_219_010_38, 0.022_173_884_79, 0.023_153_366_18, 0.024_157_632_45,
    0.025_186_859_63, 0.026_241_221_89, 0.027_320_891_64, 0.028_426_039_5,
    0.029_556_834_44, 0.030_713_443_73, 0.031_896_033_07, 0.033_104_766_57,
    0.034_339_806_81, 0.035_601_314_88, 0.036_889_450_4, 0.038_204_371_6,
    0.039_546_235_28, 0.040_915_196_91, 0.042_311_410_62, 0.043_735_029_26,
    0.045_186_204_39, 0.046_665_086_34, 0.048_171_824_23, 0.049_706_565_98,
    0.051_269_458_37, 0.052_860_647_02, 0.054_480_276_44, 0.056_128_490_05,
    0.057_805_430_19, 0.059_511_238_16, 0.061_246_054_23, 0.063_010_017_65,
    0.064_803_266_69, 0.066_625_938_64, 0.068_478_169_84, 0.070_360_095_7,
    0.072_271_850_68, 0.074_213_568_38, 0.076_185_381_48, 0.078_187_421_81,
    0.080_219_820_31, 0.082_282_707_13, 0.084_376_211_54, 0.086_500_462_04,
    0.088_655_586_29, 0.090_841_711_18, 0.093_058_962_85, 0.095_307_466_63,
    0.097_587_347_14, 0.099_898_728_25, 0.102_241_733_1, 0.104_616_484_1,
    0.107_023_103, 0.109_461_710_8, 0.111_932_427_8, 0.114_435_373_8,
    0.116_970_667_8, 0.119_538_428, 0.122_138_772_2, 0.124_771_817_6,
    0.127_437_680_4, 0.130_136_476_7, 0.132_868_321_6, 0.135_633_329_7,
    0.138_431_615, 0.141_263_291_1, 0.144_128_470_9, 0.147_027_266_5,
    0.149_959_789_8, 0.152_926_152, 0.155_926_463_7, 0.158_960_835_1,
    0.162_029_375_6, 0.165_132_194_5, 0.168_269_400_2, 0.171_441_100_7,
    0.174_647_403_7, 0.177_888_416, 0.181_164_244_2, 0.184_474_994_5,
    0.187_820_772_3, 0.191_201_682_7, 0.194_617_830_4, 0.198_069_319_6,
    0.201_556_253_8, 0.205_078_736_4, 0.208_636_870_1, 0.212_230_757_4,
    0.215_860_500_1, 0.219_526_199_7, 0.223_227_957_3, 0.226_965_873_5,
    0.230_740_048_5, 0.234_550_582_2, 0.238_397_573_8, 0.242_281_122_5,
    0.246_201_326_7, 0.250_158_284_7, 0.254_152_094_3, 0.258_182_852_9,
    0.262_250_657_5, 0.266_355_604_8, 0.270_497_791, 0.274_677_312_1,
    0.278_894_263_5, 0.283_148_740_4, 0.287_440_837_7, 0.291_770_649_8,
    0.296_138_270_8, 0.300_543_794_4, 0.304_987_314_1, 0.309_468_922_8,
    0.313_988_713_4, 0.318_546_778_1, 0.323_143_209_1, 0.327_778_098_1,
    0.332_451_536_3, 0.337_163_615, 0.341_914_424_9, 0.346_704_056_4,
    0.351_532_599_5, 0.356_400_144_1, 0.361_306_779_8, 0.366_252_595_6,
    0.371_237_680_5, 0.376_262_123, 0.381_326_011_4, 0.386_429_433_8,
    0.391_572_477_7, 0.396_755_230_7, 0.401_977_779_8, 0.407_240_211_9,
    0.412_542_613_5, 0.417_885_070_8, 0.423_267_67, 0.428_690_496_6,
    0.434_153_636_2, 0.439_657_173_8, 0.445_201_194_5, 0.450_785_782_8,
    0.456_411_023_2, 0.462_076_999_7, 0.467_783_796_1, 0.473_531_496_1,
    0.479_320_183_1, 0.485_149_940_1, 0.491_020_849_8, 0.496_932_995_1,
    0.502_886_458, 0.508_881_320_9, 0.514_917_665_4, 0.520_995_573_2,
    0.527_115_125_7, 0.533_276_404, 0.539_479_489, 0.545_724_461_4,
    0.552_011_401_5, 0.558_340_389_6, 0.564_711_505_7, 0.571_124_829_5,
    0.577_580_440_4, 0.584_078_417_9, 0.590_618_840_9, 0.597_201_788_4,
    0.603_827_338_9, 0.610_495_570_8, 0.617_206_562_4, 0.623_960_391_7,
    0.630_757_136_3, 0.637_596_874, 0.644_479_682, 0.651_405_637_4,
    0.658_374_817_3, 0.665_387_298_3, 0.672_443_157, 0.679_542_469_6,
    0.686_685_312_4, 0.693_871_761_3, 0.701_101_891_9, 0.708_375_779_9,
    0.715_693_500_5, 0.723_055_128_9, 0.730_460_740_1, 0.737_910_408_8,
    0.745_404_209_5, 0.752_942_216_8, 0.760_524_504_7, 0.768_151_147_2,
    0.775_822_218_3, 0.783_537_791_5, 0.791_297_940_3, 0.799_102_738,
    0.806_952_257_7, 0.814_846_572_2, 0.822_785_754_4, 0.830_769_876_8,
    0.838_799_011_7, 0.846_873_231_5, 0.854_992_608_1, 0.863_157_213_5,
    0.871_367_119_2, 0.879_622_396_9, 0.887_923_117_9, 0.896_269_353_4,
    0.904_661_174_4, 0.913_098_651_8, 0.921_581_856_3, 0.930_110_858_4,
    0.938_685_728_5, 0.947_306_536_7, 0.955_973_353_2, 0.964_686_247_9,
    0.973_445_290_4, 0.982_250_550_3, 0.991_102_097_1, 1.0,
];

/// Decode an sRGB byte (0-255) to linear light (0.0-1.0).
/// Single array lookup — no branches, no `powf`.
#[inline]
#[must_use]
pub fn srgb_to_linear(v: u8) -> f64 {
    SRGB_TO_LINEAR[v as usize]
}

/// Precomputed reverse table: linear light (quantized to 12 bits) → sRGB byte.
/// Eliminates binary search from the hot path. 4 KiB.
///
/// Index: `(linear_value * 4095.0).round() as usize` for linear in [0.0, 1.0].
/// Value: nearest sRGB byte (0-255).
static LINEAR_TO_SRGB_TABLE: [u8; 4096] = {
    // Build by inverting SRGB_TO_LINEAR: for each quantized linear
    // value, find the sRGB byte whose linear value is closest.
    let mut t = [0u8; 4096];
    let mut i = 0usize;
    while i < 4096 {
        let target = i as f64 / 4095.0;
        // Linear scan — runs at compile time, not runtime.
        let mut best = 0u8;
        let mut best_dist = 2.0_f64; // > max possible distance
        let mut s = 0u16;
        while s < 256 {
            let d = SRGB_TO_LINEAR[s as usize] - target;
            let d_abs = if d < 0.0 { -d } else { d };
            if d_abs < best_dist {
                best_dist = d_abs;
                best = s as u8;
            }
            s += 1;
        }
        t[i] = best;
        i += 1;
    }
    t
};

/// Encode a linear light value (0.0-1.0) to an sRGB byte (0-255).
/// Single table lookup — no binary search, no branches.
#[inline]
#[must_use]
pub fn linear_to_srgb(c: f64) -> u8 {
    if c <= 0.0 {
        return 0;
    }
    if c >= 1.0 {
        return 255;
    }
    LINEAR_TO_SRGB_TABLE[(c * 4095.0 + 0.5) as usize]
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

    /// Const-constructor for the identity transform.
    ///
    /// Used in hot paths that need a shared static palette without
    /// the `Default::default()` runtime call. Equivalent to
    /// `PaletteTransform::default()`.
    #[must_use]
    pub const fn const_identity() -> Self {
        Self {
            matrix: IDENTITY_MATRIX,
        }
    }

    /// Transform an sRGB color through this palette.
    #[must_use]
    pub fn transform(&self, r: u8, g: u8, b: u8) -> (u8, u8, u8) {
        let lin = [srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b)];
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
    fn default() -> Self {
        Self {
            matrix: IDENTITY_MATRIX,
        }
    }
}
