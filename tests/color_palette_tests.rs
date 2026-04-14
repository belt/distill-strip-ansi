//! Unit tests for the `augment-color` feature.
//!
//! Tests the palette remapping system described in
//! `doc/COLOR-TRANSFORMS.md`. Covers:
//! - sRGB linearization / encoding round-trip
//! - 3x3 matrix transform application
//! - CVD simulation matrices (Viénot 1999)
//! - Palette identity transform
//! - Palette + depth reduction composition
//!
//! These tests define the contract for `src/palette.rs`.

#![cfg(feature = "augment-color")]

use strip_ansi::palette::{
    DEUTERANOPIA_VIENOT, IDENTITY_MATRIX, PROTANOPIA_VIENOT, PaletteTransform,
    TRITANOPIA_BRETTEL_H1, apply_matrix, linear_to_srgb, srgb_to_linear,
};

// ── sRGB Linearization ──────────────────────────────────────────────

#[test]
fn srgb_linear_round_trip_black() {
    let linear = srgb_to_linear(0);
    assert_eq!(linear, 0.0);
    assert_eq!(linear_to_srgb(linear), 0);
}

#[test]
fn srgb_linear_round_trip_white() {
    let linear = srgb_to_linear(255);
    assert!(
        (linear - 1.0).abs() < 1e-4,
        "white should linearize to ~1.0"
    );
    assert_eq!(linear_to_srgb(linear), 255);
}

#[test]
fn srgb_linear_round_trip_all_values() {
    for v in 0..=255u8 {
        let linear = srgb_to_linear(v);
        let back = linear_to_srgb(linear);
        assert!(
            (v as i16 - back as i16).unsigned_abs() <= 1,
            "round-trip failed for {v}: got {back}"
        );
    }
}

#[test]
fn srgb_linear_monotonic() {
    let mut prev = 0.0f64;
    for v in 0..=255u8 {
        let linear = srgb_to_linear(v);
        assert!(
            linear >= prev,
            "srgb_to_linear not monotonic at {v}: {linear} < {prev}"
        );
        prev = linear;
    }
}

#[test]
fn srgb_linear_midpoint_below_half() {
    // sRGB 128 should linearize to well below 0.5 (gamma curve).
    let linear = srgb_to_linear(128);
    assert!(
        linear < 0.5,
        "sRGB 128 should linearize below 0.5, got {linear}"
    );
    assert!(
        linear > 0.1,
        "sRGB 128 should linearize above 0.1, got {linear}"
    );
}

// ── 3x3 Matrix Application ─────────────────────────────────────────

#[test]
fn identity_matrix_preserves_color() {
    let rgb = [0.5, 0.3, 0.8];
    let result = apply_matrix(&IDENTITY_MATRIX, &rgb);
    for i in 0..3 {
        assert!(
            (result[i] - rgb[i]).abs() < 1e-10,
            "identity should preserve channel {i}"
        );
    }
}

#[test]
fn identity_matrix_preserves_black() {
    let result = apply_matrix(&IDENTITY_MATRIX, &[0.0, 0.0, 0.0]);
    assert_eq!(result, [0.0, 0.0, 0.0]);
}

#[test]
fn identity_matrix_preserves_white() {
    let result = apply_matrix(&IDENTITY_MATRIX, &[1.0, 1.0, 1.0]);
    for (i, &val) in result.iter().enumerate() {
        assert!(
            (val - 1.0).abs() < 1e-10,
            "identity should preserve white channel {i}"
        );
    }
}

// ── CVD Simulation Matrices ─────────────────────────────────────────

#[test]
fn protanopia_collapses_red_green() {
    // Under protanopia, pure red and a specific green should
    // produce similar outputs (they lie on a confusion line).
    let red = [1.0, 0.0, 0.0];
    let sim_red = apply_matrix(&PROTANOPIA_VIENOT, &red);
    // Red channel should be significantly reduced.
    assert!(
        sim_red[0] < 0.5,
        "protanopia should reduce red perception, got R={:.3}",
        sim_red[0]
    );
}

#[test]
fn protanopia_preserves_blue() {
    // Blue should be mostly preserved under protanopia.
    let blue = [0.0, 0.0, 1.0];
    let sim_blue = apply_matrix(&PROTANOPIA_VIENOT, &blue);
    assert!(
        sim_blue[2] > 0.9,
        "protanopia should mostly preserve blue, got B={:.3}",
        sim_blue[2]
    );
}

#[test]
fn deuteranopia_collapses_red_green() {
    // Under deuteranopia, red and green should produce similar outputs.
    let red = [1.0, 0.0, 0.0];
    let green = [0.0, 1.0, 0.0];
    let sim_red = apply_matrix(&DEUTERANOPIA_VIENOT, &red);
    let sim_green = apply_matrix(&DEUTERANOPIA_VIENOT, &green);
    // Both should have similar R and G values (confusion).
    let r_diff = (sim_red[0] - sim_green[0]).abs();
    let g_diff = (sim_red[1] - sim_green[1]).abs();
    // They won't be identical but should be closer than the originals.
    assert!(
        r_diff < 0.8,
        "deuteranopia should bring R channels closer: diff={r_diff:.3}"
    );
    assert!(
        g_diff < 0.8,
        "deuteranopia should bring G channels closer: diff={g_diff:.3}"
    );
}

#[test]
fn deuteranopia_preserves_blue() {
    let blue = [0.0, 0.0, 1.0];
    let sim_blue = apply_matrix(&DEUTERANOPIA_VIENOT, &blue);
    assert!(
        sim_blue[2] > 0.9,
        "deuteranopia should mostly preserve blue, got B={:.3}",
        sim_blue[2]
    );
}

#[test]
fn cvd_matrices_preserve_black() {
    for (name, matrix) in [
        ("protanopia", &PROTANOPIA_VIENOT),
        ("deuteranopia", &DEUTERANOPIA_VIENOT),
        ("tritanopia", &TRITANOPIA_BRETTEL_H1),
    ] {
        let result = apply_matrix(matrix, &[0.0, 0.0, 0.0]);
        for (i, &val) in result.iter().enumerate() {
            assert!(
                val.abs() < 1e-10,
                "{name} should preserve black, channel {i} = {val:.6}",
            );
        }
    }
}

#[test]
fn cvd_matrices_preserve_white_luminance() {
    // White should remain close to white (luminance preserved).
    for (name, matrix) in [
        ("protanopia", &PROTANOPIA_VIENOT),
        ("deuteranopia", &DEUTERANOPIA_VIENOT),
        ("tritanopia", &TRITANOPIA_BRETTEL_H1),
    ] {
        let result = apply_matrix(matrix, &[1.0, 1.0, 1.0]);
        for (i, &val) in result.iter().enumerate() {
            assert!(
                val > 0.8 && val < 1.2,
                "{name} white channel {i} = {val:.3}, expected ~1.0",
            );
        }
    }
}

#[test]
fn cvd_matrix_output_non_negative_for_primaries() {
    // Primary colors should not produce negative outputs (after clamping).
    let primaries = [
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 1.0, 0.0],
        [1.0, 0.0, 1.0],
        [0.0, 1.0, 1.0],
    ];
    for (name, matrix) in [
        ("protanopia", &PROTANOPIA_VIENOT),
        ("deuteranopia", &DEUTERANOPIA_VIENOT),
        ("tritanopia", &TRITANOPIA_BRETTEL_H1),
    ] {
        for primary in &primaries {
            let result = apply_matrix(matrix, primary);
            for (i, &val) in result.iter().enumerate() {
                assert!(
                    val >= -0.2,
                    "{name} produced negative channel {i} = {val:.4} for {primary:?}",
                );
            }
        }
    }
}

// ── PaletteTransform API ────────────────────────────────────────────

#[test]
fn palette_transform_default_is_identity() {
    let t = PaletteTransform::default();
    // Applying default transform should not change colors.
    let (r, g, b) = t.transform(128, 64, 200);
    assert_eq!((r, g, b), (128, 64, 200), "default should be identity");
}

#[test]
fn palette_transform_round_trip_extremes() {
    let t = PaletteTransform::default();
    assert_eq!(t.transform(0, 0, 0), (0, 0, 0));
    assert_eq!(t.transform(255, 255, 255), (255, 255, 255));
}

#[test]
fn palette_transform_clamps_output() {
    // A matrix that amplifies values should clamp to 0-255.
    let amplify: [[f64; 3]; 3] = [[2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 2.0]];
    let t = PaletteTransform::from_matrix(amplify);
    let (r, g, b) = t.transform(200, 200, 200);
    assert_eq!(r, 255, "should clamp R to 255");
    assert_eq!(g, 255, "should clamp G to 255");
    assert_eq!(b, 255, "should clamp B to 255");
}

#[test]
fn palette_transform_clamps_negative() {
    // A matrix that produces negative values should clamp to 0.
    let invert: [[f64; 3]; 3] = [[-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]];
    let t = PaletteTransform::from_matrix(invert);
    let (r, g, b) = t.transform(128, 128, 128);
    assert_eq!(r, 0, "should clamp negative R to 0");
    assert_eq!(g, 0, "should clamp negative G to 0");
    assert_eq!(b, 0, "should clamp negative B to 0");
}

// ── Tritanopia (Brettel H1) ─────────────────────────────────────────

#[test]
fn tritanopia_collapses_blue_yellow() {
    // Under tritanopia, blue and yellow should become harder to
    // distinguish (they lie on the S-cone confusion axis).
    let blue = [0.0, 0.0, 1.0];
    let sim_blue = apply_matrix(&TRITANOPIA_BRETTEL_H1, &blue);
    // Blue channel should be significantly reduced.
    assert!(
        sim_blue[2] < 0.5,
        "tritanopia should reduce blue perception, got B={:.3}",
        sim_blue[2]
    );
}

#[test]
fn tritanopia_preserves_red() {
    // Red should be mostly preserved under tritanopia (L cone intact).
    let red = [1.0, 0.0, 0.0];
    let sim_red = apply_matrix(&TRITANOPIA_BRETTEL_H1, &red);
    assert!(
        sim_red[0] > 0.9,
        "tritanopia should mostly preserve red, got R={:.3}",
        sim_red[0]
    );
}

#[test]
fn tritanopia_preserves_green() {
    // Green should be mostly preserved under tritanopia (M cone intact).
    let green = [0.0, 1.0, 0.0];
    let sim_green = apply_matrix(&TRITANOPIA_BRETTEL_H1, &green);
    assert!(
        sim_green[1] > 0.8,
        "tritanopia should mostly preserve green, got G={:.3}",
        sim_green[1]
    );
}

#[test]
fn tritanopia_palette_transform_modifies_blue() {
    // End-to-end: PaletteTransform with tritanopia matrix should
    // visibly shift pure blue.
    let t = PaletteTransform::from_matrix(TRITANOPIA_BRETTEL_H1);
    let (r, g, b) = t.transform(0, 0, 255);
    // Blue should be reduced, red/green should gain some energy.
    assert!(b < 200, "tritanopia should reduce blue output, got B={b}");
    assert!(
        r > 0 || g > 0,
        "tritanopia should redistribute blue energy, got R={r} G={g}"
    );
}

#[test]
fn tritanopia_palette_transform_preserves_grey() {
    // Neutral grey should be approximately preserved.
    let t = PaletteTransform::from_matrix(TRITANOPIA_BRETTEL_H1);
    let (r, g, b) = t.transform(128, 128, 128);
    assert!(
        (r as i16 - 128).unsigned_abs() <= 15,
        "grey R should be near 128, got {r}"
    );
    assert!(
        (g as i16 - 128).unsigned_abs() <= 15,
        "grey G should be near 128, got {g}"
    );
    assert!(
        (b as i16 - 128).unsigned_abs() <= 15,
        "grey B should be near 128, got {b}"
    );
}
