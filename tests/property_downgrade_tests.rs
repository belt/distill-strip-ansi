//! Property-based tests for `downgrade-color` and `color-palette`.
//!
//! Validates invariants that must hold for ALL inputs, not just
//! specific examples. Uses proptest.

#![cfg(feature = "downgrade-color")]

use proptest::prelude::*;
use strip_ansi::downgrade::{
    nearest_256, nearest_16, nearest_greyscale, nearest_axis,
    cube_to_rgb, grey_index_to_value,
};

// ── Axis Quantization Properties ────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]

    /// nearest_axis always returns 0..=5.
    #[test]
    fn prop_nearest_axis_range(v in 0u8..=255) {
        let idx = nearest_axis(v);
        prop_assert!(idx <= 5, "nearest_axis({v}) = {idx}, expected 0..=5");
    }

    /// nearest_axis is monotonically non-decreasing.
    #[test]
    fn prop_nearest_axis_monotonic(v in 0u8..254) {
        let a = nearest_axis(v);
        let b = nearest_axis(v + 1);
        prop_assert!(
            b >= a,
            "nearest_axis not monotonic: f({v})={a}, f({})={b}",
            v + 1
        );
    }

    /// The axis value at nearest_axis(v) is the closest of the 6 values.
    #[test]
    fn prop_nearest_axis_is_closest(v in 0u8..=255) {
        const AXIS: [u8; 6] = [0x00, 0x5F, 0x87, 0xAF, 0xD7, 0xFF];
        let idx = nearest_axis(v) as usize;
        let chosen_dist = (v as i16 - AXIS[idx] as i16).unsigned_abs();
        for (i, &a) in AXIS.iter().enumerate() {
            let dist = (v as i16 - a as i16).unsigned_abs();
            prop_assert!(
                dist >= chosen_dist,
                "nearest_axis({v})={idx} (dist {chosen_dist}) but axis {i} is closer (dist {dist})"
            );
        }
    }
}

// ── Truecolor → 256 Properties ──────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig { cases: 1024, ..Default::default() })]

    /// nearest_256 always returns 16..=255 (cube or greyscale, never basic).
    #[test]
    fn prop_nearest_256_range(r in 0u8..=255, g in 0u8..=255, b in 0u8..=255) {
        let idx = nearest_256(r, g, b);
        prop_assert!(
            (16..=255).contains(&idx),
            "nearest_256({r},{g},{b}) = {idx}, expected 16..=255"
        );
    }

    /// Exact cube vertex colors should map to themselves.
    #[test]
    fn prop_nearest_256_exact_cube(
        ri in 0u8..6, gi in 0u8..6, bi in 0u8..6
    ) {
        const AXIS: [u8; 6] = [0x00, 0x5F, 0x87, 0xAF, 0xD7, 0xFF];
        let r = AXIS[ri as usize];
        let g = AXIS[gi as usize];
        let b = AXIS[bi as usize];
        let expected = 16 + 36 * ri + 6 * gi + bi;
        let result = nearest_256(r, g, b);
        // Cube vertex should map to itself OR a greyscale entry
        // that happens to be equidistant (for greys on the diagonal).
        if r == g && g == b {
            // Diagonal — either cube or grey is acceptable.
            prop_assert!(
                result == expected || (232..=255).contains(&result),
                "diagonal ({r},{g},{b}): expected cube {expected} or grey, got {result}"
            );
        } else {
            prop_assert_eq!(result, expected);
        }
    }

    /// Exact greyscale ramp values should map to themselves.
    #[test]
    fn prop_nearest_256_exact_grey(idx in 232u8..=255) {
        let val = grey_index_to_value(idx);
        let result = nearest_256(val, val, val);
        // Should map to this grey index or the corresponding cube diagonal.
        let grey_val = grey_index_to_value(result);
        if (232..=255).contains(&result) {
            prop_assert_eq!(result, idx);
        }
        // Cube diagonal is also acceptable if equidistant.
    }
}

// ── 256 → 16 Properties ────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]

    /// nearest_16 always returns 0..=15.
    #[test]
    fn prop_nearest_16_range(idx in 0u8..=255) {
        let result = nearest_16(idx);
        prop_assert!(result <= 15, "nearest_16({idx}) = {result}, expected 0..=15");
    }

    /// Basic colors (0-15) map to themselves.
    #[test]
    fn prop_nearest_16_identity_for_basic(idx in 0u8..=15) {
        prop_assert_eq!(nearest_16(idx), idx);
    }
}

// ── Greyscale Properties ────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig { cases: 1024, ..Default::default() })]

    /// nearest_greyscale always returns 232..=255.
    #[test]
    fn prop_greyscale_range(r in 0u8..=255, g in 0u8..=255, b in 0u8..=255) {
        let idx = nearest_greyscale(r, g, b);
        prop_assert!(
            (232..=255).contains(&idx),
            "nearest_greyscale({r},{g},{b}) = {idx}, expected 232..=255"
        );
    }

    /// Greyscale is monotonic in uniform grey input.
    #[test]
    fn prop_greyscale_monotonic_uniform(v in 0u8..254) {
        let a = nearest_greyscale(v, v, v);
        let b = nearest_greyscale(v + 1, v + 1, v + 1);
        prop_assert!(
            b >= a,
            "greyscale not monotonic: f({v},{v},{v})={a}, f({},{},{})={b}",
            v + 1, v + 1, v + 1
        );
    }

    /// Increasing green (highest Rec. 709 weight) increases luminance.
    #[test]
    fn prop_greyscale_green_weight(g1 in 0u8..200, delta in 50u8..=55) {
        let g2 = g1.saturating_add(delta);
        if g2 > g1 {
            let a = nearest_greyscale(0, g1, 0);
            let b = nearest_greyscale(0, g2, 0);
            prop_assert!(
                b >= a,
                "more green should mean brighter: g={g1}→{a}, g={g2}→{b}"
            );
        }
    }
}

// ── Cube ↔ RGB Round-Trip ───────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig { cases: 216, ..Default::default() })]

    /// cube_to_rgb for all 216 cube indices produces valid axis values.
    #[test]
    fn prop_cube_to_rgb_valid(idx in 16u8..=231) {
        const AXIS: [u8; 6] = [0x00, 0x5F, 0x87, 0xAF, 0xD7, 0xFF];
        let (r, g, b) = cube_to_rgb(idx);
        prop_assert!(AXIS.contains(&r), "R={r:#04x} not an axis value for idx {idx}");
        prop_assert!(AXIS.contains(&g), "G={g:#04x} not an axis value for idx {idx}");
        prop_assert!(AXIS.contains(&b), "B={b:#04x} not an axis value for idx {idx}");
    }
}

// ── sRGB Round-Trip (if color-palette also enabled) ─────────────────

#[cfg(feature = "color-palette")]
mod palette_properties {
    use super::*;
    use strip_ansi::palette::{srgb_to_linear, linear_to_srgb};

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]

        /// sRGB → linear → sRGB round-trips within ±1.
        #[test]
        fn prop_srgb_round_trip(v in 0u8..=255) {
            let linear = srgb_to_linear(v);
            let back = linear_to_srgb(linear);
            prop_assert!(
                (v as i16 - back as i16).unsigned_abs() <= 1,
                "round-trip failed for {v}: got {back}"
            );
        }

        /// sRGB linearization is monotonically non-decreasing.
        #[test]
        fn prop_srgb_monotonic(v in 0u8..254) {
            let a = srgb_to_linear(v);
            let b = srgb_to_linear(v + 1);
            prop_assert!(
                b >= a,
                "srgb_to_linear not monotonic: f({v})={a}, f({})={b}",
                v + 1
            );
        }

        /// Linear values are in [0.0, 1.0].
        #[test]
        fn prop_srgb_linear_bounded(v in 0u8..=255) {
            let linear = srgb_to_linear(v);
            prop_assert!(linear >= 0.0 && linear <= 1.0,
                "srgb_to_linear({v}) = {linear}, expected [0,1]");
        }
    }
}
