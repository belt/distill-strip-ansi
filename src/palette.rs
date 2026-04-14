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
    [0.10889, 0.89111, -0.00000],
    [0.10889, 0.89111, 0.00000],
    [0.00447, -0.00447, 1.00000],
];

/// Deuteranopia simulation matrix (Viénot et al. 1999, linear RGB).
pub const DEUTERANOPIA_VIENOT: Matrix3 = [
    [0.29275, 0.70725, 0.00000],
    [0.29275, 0.70725, -0.00000],
    [-0.02234, 0.02234, 1.00000],
];

/// Tritanopia simulation matrix (Brettel et al. 1997, H1 half-plane, linear RGB).
///
/// Models blue-yellow confusion (S cone absence). Uses the H1
/// half-plane which covers the majority of real-world colors.
/// Source: DaltonLens project, Brettel 1997 implementation.
pub const TRITANOPIA_BRETTEL_H1: Matrix3 = [
    [1.01354, 0.14268, -0.15622],
    [-0.01181, 0.87561, 0.13619],
    [0.07707, 0.81208, 0.11085],
];

// ── sRGB Linearization via Lookup Table ─────────────────────────────

/// Precomputed sRGB byte (0-255) → linear light (0.0-1.0).
/// Eliminates `powf(2.4)` from the hot path. 2 KiB.
#[rustfmt::skip]
static SRGB_TO_LINEAR: [f64; 256] = [
    0.0, 0.0003035269835, 0.0006070539671, 0.0009105809506,
    0.001214107934, 0.001517634918, 0.001821161901, 0.002124688885,
    0.002428215868, 0.002731742852, 0.003035269835, 0.003346535764,
    0.003676507324, 0.004024717018, 0.004391442037, 0.004776953481,
    0.005181516702, 0.005605391624, 0.006048833023, 0.006512090793,
    0.006995410187, 0.007499032043, 0.008023192985, 0.008568125618,
    0.009134058702, 0.00972121732, 0.01032982303, 0.01096009401,
    0.01161224518, 0.01228648836, 0.01298303234, 0.01370208305,
    0.0144438436, 0.01520851442, 0.01599629337, 0.01680737575,
    0.01764195449, 0.01850022013, 0.01938236096, 0.02028856306,
    0.02121901038, 0.02217388479, 0.02315336618, 0.02415763245,
    0.02518685963, 0.02624122189, 0.02732089164, 0.0284260395,
    0.02955683444, 0.03071344373, 0.03189603307, 0.03310476657,
    0.03433980681, 0.03560131488, 0.0368894504, 0.0382043716,
    0.03954623528, 0.04091519691, 0.04231141062, 0.04373502926,
    0.04518620439, 0.04666508634, 0.04817182423, 0.04970656598,
    0.05126945837, 0.05286064702, 0.05448027644, 0.05612849005,
    0.05780543019, 0.05951123816, 0.06124605423, 0.06301001765,
    0.06480326669, 0.06662593864, 0.06847816984, 0.0703600957,
    0.07227185068, 0.07421356838, 0.07618538148, 0.07818742181,
    0.08021982031, 0.08228270713, 0.08437621154, 0.08650046204,
    0.08865558629, 0.09084171118, 0.09305896285, 0.09530746663,
    0.09758734714, 0.09989872825, 0.1022417331, 0.1046164841,
    0.107023103, 0.1094617108, 0.1119324278, 0.1144353738,
    0.1169706678, 0.119538428, 0.1221387722, 0.1247718176,
    0.1274376804, 0.1301364767, 0.1328683216, 0.1356333297,
    0.138431615, 0.1412632911, 0.1441284709, 0.1470272665,
    0.1499597898, 0.152926152, 0.1559264637, 0.1589608351,
    0.1620293756, 0.1651321945, 0.1682694002, 0.1714411007,
    0.1746474037, 0.177888416, 0.1811642442, 0.1844749945,
    0.1878207723, 0.1912016827, 0.1946178304, 0.1980693196,
    0.2015562538, 0.2050787364, 0.2086368701, 0.2122307574,
    0.2158605001, 0.2195261997, 0.2232279573, 0.2269658735,
    0.2307400485, 0.2345505822, 0.2383975738, 0.2422811225,
    0.2462013267, 0.2501582847, 0.2541520943, 0.2581828529,
    0.2622506575, 0.2663556048, 0.270497791, 0.2746773121,
    0.2788942635, 0.2831487404, 0.2874408377, 0.2917706498,
    0.2961382708, 0.3005437944, 0.3049873141, 0.3094689228,
    0.3139887134, 0.3185467781, 0.3231432091, 0.3277780981,
    0.3324515363, 0.337163615, 0.3419144249, 0.3467040564,
    0.3515325995, 0.3564001441, 0.3613067798, 0.3662525956,
    0.3712376805, 0.376262123, 0.3813260114, 0.3864294338,
    0.3915724777, 0.3967552307, 0.4019777798, 0.4072402119,
    0.4125426135, 0.4178850708, 0.42326767, 0.4286904966,
    0.4341536362, 0.4396571738, 0.4452011945, 0.4507857828,
    0.4564110232, 0.4620769997, 0.4677837961, 0.4735314961,
    0.4793201831, 0.4851499401, 0.4910208498, 0.4969329951,
    0.502886458, 0.5088813209, 0.5149176654, 0.5209955732,
    0.5271151257, 0.533276404, 0.539479489, 0.5457244614,
    0.5520114015, 0.5583403896, 0.5647115057, 0.5711248295,
    0.5775804404, 0.5840784179, 0.5906188409, 0.5972017884,
    0.6038273389, 0.6104955708, 0.6172065624, 0.6239603917,
    0.6307571363, 0.637596874, 0.644479682, 0.6514056374,
    0.6583748173, 0.6653872983, 0.672443157, 0.6795424696,
    0.6866853124, 0.6938717613, 0.7011018919, 0.7083757799,
    0.7156935005, 0.7230551289, 0.7304607401, 0.7379104088,
    0.7454042095, 0.7529422168, 0.7605245047, 0.7681511472,
    0.7758222183, 0.7835377915, 0.7912979403, 0.799102738,
    0.8069522577, 0.8148465722, 0.8227857544, 0.8307698768,
    0.8387990117, 0.8468732315, 0.8549926081, 0.8631572135,
    0.8713671192, 0.8796223969, 0.8879231179, 0.8962693534,
    0.9046611744, 0.9130986518, 0.9215818563, 0.9301108584,
    0.9386857285, 0.9473065367, 0.9559733532, 0.9646862479,
    0.9734452904, 0.9822505503, 0.9911020971, 1.0,
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
