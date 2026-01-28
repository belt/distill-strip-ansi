//! Unit tests for the `downgrade-color` feature.
//!
//! Tests the color depth reduction algorithms described in
//! `doc/COLOR-TRANSFORMS.md`. Covers:
//! - 256-color cube axis quantization
//! - Truecolor → 256 mapping
//! - 256 → 16 mapping
//! - Greyscale conversion (Rec. 709 luminance)
//! - Monochrome SGR param stripping
//! - SGR param rewriting (shared infrastructure)
//!
//! These tests define the contract for `src/downgrade.rs` and
//! `src/sgr_rewrite.rs`.

#![cfg(feature = "downgrade-color")]

use strip_ansi::downgrade::{
    cube_to_rgb, grey_index_to_value, nearest_16, nearest_256, nearest_axis, nearest_greyscale,
};
use strip_ansi::sgr_rewrite::rewrite_sgr_params;

// ── 6x6x6 Cube Axis Quantization ───────────────────────────────────

/// The six axis values per doc/COLOR-TRANSFORMS.md.
const AXIS: [u8; 6] = [0x00, 0x5F, 0x87, 0xAF, 0xD7, 0xFF];

#[test]
fn nearest_axis_exact_values() {
    for (i, &val) in AXIS.iter().enumerate() {
        assert_eq!(
            nearest_axis(val),
            i as u8,
            "exact axis value {val:#04x} should map to index {i}"
        );
    }
}

#[test]
fn nearest_axis_boundary_low() {
    // 47 is the last value mapping to axis 0
    assert_eq!(nearest_axis(47), 0);
    // 48 is the first value mapping to axis 1
    assert_eq!(nearest_axis(48), 1);
}

#[test]
fn nearest_axis_boundary_1_2() {
    assert_eq!(nearest_axis(114), 1);
    assert_eq!(nearest_axis(115), 2);
}

#[test]
fn nearest_axis_boundary_2_3() {
    assert_eq!(nearest_axis(154), 2);
    assert_eq!(nearest_axis(155), 3);
}

#[test]
fn nearest_axis_boundary_3_4() {
    assert_eq!(nearest_axis(194), 3);
    assert_eq!(nearest_axis(195), 4);
}

#[test]
fn nearest_axis_boundary_4_5() {
    assert_eq!(nearest_axis(234), 4);
    assert_eq!(nearest_axis(235), 5);
}

#[test]
fn nearest_axis_extremes() {
    assert_eq!(nearest_axis(0), 0);
    assert_eq!(nearest_axis(255), 5);
}

// ── Cube Index ↔ RGB Round-Trip ─────────────────────────────────────

#[test]
fn cube_to_rgb_corners() {
    // Index 16 = (0,0,0) → black
    assert_eq!(cube_to_rgb(16), (0x00, 0x00, 0x00));
    // Index 231 = (5,5,5) → white
    assert_eq!(cube_to_rgb(231), (0xFF, 0xFF, 0xFF));
    // Index 196 = 16 + 36*5 + 6*0 + 0 → pure red
    assert_eq!(cube_to_rgb(196), (0xFF, 0x00, 0x00));
    // Index 46 = 16 + 36*0 + 6*5 + 0 → pure green
    assert_eq!(cube_to_rgb(46), (0x00, 0xFF, 0x00));
    // Index 21 = 16 + 36*0 + 6*0 + 5 → pure blue
    assert_eq!(cube_to_rgb(21), (0x00, 0x00, 0xFF));
}

#[test]
fn cube_to_rgb_mid_value() {
    // Index 103 = 16 + 36*2 + 6*2 + 3 → (0x87, 0x87, 0xAF)
    assert_eq!(cube_to_rgb(103), (0x87, 0x87, 0xAF));
}

// ── Greyscale Ramp ──────────────────────────────────────────────────

#[test]
fn grey_index_to_value_endpoints() {
    assert_eq!(grey_index_to_value(232), 8);
    assert_eq!(grey_index_to_value(255), 238);
}

#[test]
fn grey_index_to_value_formula() {
    for idx in 232..=255u8 {
        let expected = 8 + 10 * (idx as u16 - 232);
        assert_eq!(
            grey_index_to_value(idx) as u16,
            expected,
            "grey index {idx} should have value {expected}"
        );
    }
}

// ── Truecolor → 256 ────────────────────────────────────────────────

#[test]
fn truecolor_to_256_pure_black() {
    // Pure black → cube index 16 (0,0,0) or greyscale 232 (value 8).
    // Cube (0,0,0) has distance 0, so cube wins.
    assert_eq!(nearest_256(0, 0, 0), 16);
}

#[test]
fn truecolor_to_256_pure_white() {
    // Pure white (255,255,255) → cube index 231 (0xFF,0xFF,0xFF).
    // Cube distance = 0, so cube wins.
    assert_eq!(nearest_256(255, 255, 255), 231);
}

#[test]
fn truecolor_to_256_exact_cube_colors() {
    // Colors that land exactly on cube vertices should map to them.
    assert_eq!(nearest_256(0x5F, 0x00, 0x00), 52); // 16 + 36*1
    assert_eq!(nearest_256(0x00, 0x5F, 0x00), 22); // 16 + 6*1
    assert_eq!(nearest_256(0x00, 0x00, 0x5F), 17); // 16 + 1
}

#[test]
fn truecolor_to_256_exact_grey_values() {
    // Mid-grey (128,128,128) → should prefer greyscale ramp.
    // Nearest cube: (0x87,0x87,0x87) = index 145, distance = 3*(0x87-128)^2 = 3*49 = 147
    // Nearest grey: value 128 → index 232 + (128-8)/10 = 244, value 128
    //   but 8+10*(244-232) = 128, distance = 0
    let result = nearest_256(128, 128, 128);
    assert_eq!(grey_index_to_value(result), 128);
    assert!(result >= 232, "should be greyscale index");
}

#[test]
fn truecolor_to_256_saturated_prefers_cube() {
    // Saturated red (200, 0, 0) should prefer cube over greyscale.
    let result = nearest_256(200, 0, 0);
    assert!(
        (16..=231).contains(&result),
        "saturated color should map to cube"
    );
}

#[test]
fn truecolor_to_256_result_in_range() {
    // Spot-check a spread of values.
    for r in (0..=255).step_by(51) {
        for g in (0..=255).step_by(51) {
            for b in (0..=255).step_by(51) {
                let idx = nearest_256(r, g, b);
                assert!(
                    (16..=255).contains(&idx),
                    "nearest_256({r},{g},{b}) = {idx}, out of range 16..=255"
                );
            }
        }
    }
}

// ── 256 → 16 ────────────────────────────────────────────────────────

#[test]
fn index_0_15_identity() {
    for i in 0..=15u8 {
        assert_eq!(nearest_16(i), i, "basic color {i} should map to itself");
    }
}

#[test]
fn cube_red_maps_to_basic_red() {
    // Cube pure red (index 196, RGB 255,0,0) → bright red (9)
    let result = nearest_16(196);
    assert!(
        result == 1 || result == 9,
        "cube red should map to red (1) or bright red (9), got {result}"
    );
}

#[test]
fn cube_green_maps_to_basic_green() {
    // Cube pure green (index 46, RGB 0,255,0) → bright green (10)
    let result = nearest_16(46);
    assert!(
        result == 2 || result == 10,
        "cube green should map to green (2) or bright green (10), got {result}"
    );
}

#[test]
fn cube_blue_maps_to_basic_blue() {
    // Cube pure blue (index 21, RGB 0,0,255) → bright blue (12)
    let result = nearest_16(21);
    assert!(
        result == 4 || result == 12,
        "cube blue should map to blue (4) or bright blue (12), got {result}"
    );
}

#[test]
fn greyscale_dark_maps_to_black() {
    // Greyscale index 232 (value 8) → black (0) or bright black (8)
    let result = nearest_16(232);
    assert!(
        result == 0 || result == 8,
        "dark grey should map to black (0) or bright black (8), got {result}"
    );
}

#[test]
fn greyscale_light_maps_to_white() {
    // Greyscale index 255 (value 238) → white (7) or bright white (15)
    let result = nearest_16(255);
    assert!(
        result == 7 || result == 15,
        "light grey should map to white (7) or bright white (15), got {result}"
    );
}

#[test]
fn nearest_16_result_in_range() {
    for i in 0..=255u8 {
        let result = nearest_16(i);
        assert!(
            result <= 15,
            "nearest_16({i}) = {result}, out of range 0..=15"
        );
    }
}

// ── Greyscale Conversion ────────────────────────────────────────────

#[test]
fn greyscale_pure_black() {
    let idx = nearest_greyscale(0, 0, 0);
    assert_eq!(idx, 232, "pure black → darkest grey ramp entry");
}

#[test]
fn greyscale_pure_white() {
    let idx = nearest_greyscale(255, 255, 255);
    assert_eq!(idx, 255, "pure white → lightest grey ramp entry");
}

#[test]
fn greyscale_mid_grey() {
    // (128,128,128) → luminance ~128 → index 244
    let idx = nearest_greyscale(128, 128, 128);
    let val = grey_index_to_value(idx);
    assert!(
        (val as i16 - 128).unsigned_abs() <= 10,
        "mid grey should map near value 128, got index {idx} value {val}"
    );
}

#[test]
fn greyscale_green_has_highest_luminance_weight() {
    // Green channel has highest Rec. 709 weight (0.7152).
    // Pure green (0,255,0) should map brighter than pure red (255,0,0).
    let green_idx = nearest_greyscale(0, 255, 0);
    let red_idx = nearest_greyscale(255, 0, 0);
    assert!(
        green_idx > red_idx,
        "green (idx {green_idx}) should be brighter than red (idx {red_idx})"
    );
}

#[test]
fn greyscale_blue_has_lowest_luminance_weight() {
    // Blue channel has lowest Rec. 709 weight (0.0722).
    // Pure blue (0,0,255) should map darker than pure red (255,0,0).
    let blue_idx = nearest_greyscale(0, 0, 255);
    let red_idx = nearest_greyscale(255, 0, 0);
    assert!(
        blue_idx < red_idx,
        "blue (idx {blue_idx}) should be darker than red (idx {red_idx})"
    );
}

#[test]
fn greyscale_result_in_range() {
    for r in (0..=255).step_by(51) {
        for g in (0..=255).step_by(51) {
            for b in (0..=255).step_by(51) {
                let idx = nearest_greyscale(r, g, b);
                assert!(
                    (232..=255).contains(&idx),
                    "nearest_greyscale({r},{g},{b}) = {idx}, out of range"
                );
            }
        }
    }
}

// ── SGR Param Rewriting ─────────────────────────────────────────────

/// Helper: build an SGR sequence from raw param bytes.
fn sgr(params: &[u8]) -> Vec<u8> {
    let mut seq = vec![0x1B, b'['];
    seq.extend_from_slice(params);
    seq.push(b'm');
    seq
}

#[test]
fn rewrite_truecolor_fg_to_256() {
    // ESC[38;2;255;0;0m → ESC[38;5;Nm
    let input = sgr(b"38;2;255;0;0");
    let result = rewrite_sgr_params(&input, ColorDepth::Color256);
    // Should contain 38;5; prefix
    let s = String::from_utf8_lossy(&result);
    assert!(s.contains("38;5;"), "expected 38;5;N, got {s}");
    assert!(!s.contains("38;2;"), "truecolor should be gone");
}

#[test]
fn rewrite_truecolor_bg_to_256() {
    // ESC[48;2;0;128;255m → ESC[48;5;Nm
    let input = sgr(b"48;2;0;128;255");
    let result = rewrite_sgr_params(&input, ColorDepth::Color256);
    let s = String::from_utf8_lossy(&result);
    assert!(s.contains("48;5;"), "expected 48;5;N, got {s}");
    assert!(!s.contains("48;2;"), "truecolor should be gone");
}

#[test]
fn rewrite_256_fg_to_16() {
    // ESC[38;5;196m → ESC[91m (bright red) or ESC[31m (red)
    let input = sgr(b"38;5;196");
    let result = rewrite_sgr_params(&input, ColorDepth::Color16);
    let s = String::from_utf8_lossy(&result);
    assert!(!s.contains("38;5;"), "256-color should be gone");
    // Should be a basic fg color code (30-37 or 90-97)
    assert!(s.contains("m"), "should end with m");
}

#[test]
fn rewrite_preserves_style_params() {
    // ESC[1;38;2;255;0;0;4m → ESC[1;38;5;N;4m (bold + color + underline)
    let input = sgr(b"1;38;2;255;0;0;4");
    let result = rewrite_sgr_params(&input, ColorDepth::Color256);
    let s = String::from_utf8_lossy(&result);
    // Bold (1) and underline (4) must survive
    assert!(s.starts_with("\x1b["), "should start with CSI");
    assert!(s.ends_with("m"), "should end with m");
    let params_str = &s[2..s.len() - 1]; // strip ESC[ and m
    let params: Vec<&str> = params_str.split(';').collect();
    assert!(params.contains(&"1"), "bold param missing: {s}");
    assert!(params.contains(&"4"), "underline param missing: {s}");
}

#[test]
fn rewrite_basic_colors_unchanged_at_256() {
    // ESC[31m (basic red) should pass through when target is 256.
    let input = sgr(b"31");
    let result = rewrite_sgr_params(&input, ColorDepth::Color256);
    assert_eq!(
        result, input,
        "basic color should be unchanged at 256 depth"
    );
}

#[test]
fn rewrite_basic_colors_unchanged_at_16() {
    // ESC[31m (basic red) should pass through when target is 16.
    let input = sgr(b"31");
    let result = rewrite_sgr_params(&input, ColorDepth::Color16);
    assert_eq!(result, input, "basic color should be unchanged at 16 depth");
}

#[test]
fn rewrite_reset_preserved() {
    // ESC[0m should always pass through.
    let input = sgr(b"0");
    let result = rewrite_sgr_params(&input, ColorDepth::Color16);
    assert_eq!(result, input, "reset should be unchanged");
}

#[test]
fn rewrite_empty_sgr_preserved() {
    // ESC[m (implicit reset) should pass through.
    let input = sgr(b"");
    let result = rewrite_sgr_params(&input, ColorDepth::Color16);
    assert_eq!(result, input, "implicit reset should be unchanged");
}

#[test]
fn rewrite_to_mono_strips_all_color() {
    // ESC[1;38;2;255;0;0;4m → ESC[1;4m (bold + underline, no color)
    let input = sgr(b"1;38;2;255;0;0;4");
    let result = rewrite_sgr_params(&input, ColorDepth::Mono);
    let s = String::from_utf8_lossy(&result);
    assert!(!s.contains("38"), "fg color should be stripped in mono");
    assert!(!s.contains("31"), "basic fg should be stripped in mono");
    let params_str = &s[2..s.len() - 1];
    let params: Vec<&str> = params_str.split(';').collect();
    assert!(params.contains(&"1"), "bold should survive mono: {s}");
    assert!(params.contains(&"4"), "underline should survive mono: {s}");
}

#[test]
fn rewrite_to_mono_strips_basic_fg_bg() {
    // ESC[31;42m → ESC[m (both colors stripped, only reset remains)
    let input = sgr(b"31;42");
    let result = rewrite_sgr_params(&input, ColorDepth::Mono);
    let s = String::from_utf8_lossy(&result);
    // Should not contain any color codes
    for code in 30..=37 {
        assert!(
            !s.contains(&code.to_string()),
            "fg {code} should be stripped"
        );
    }
    for code in 40..=47 {
        assert!(
            !s.contains(&code.to_string()),
            "bg {code} should be stripped"
        );
    }
}

#[test]
fn rewrite_to_greyscale_converts_color() {
    // ESC[38;2;255;0;0m → ESC[38;5;Nm where N is in greyscale ramp
    let input = sgr(b"38;2;255;0;0");
    let result = rewrite_sgr_params(&input, ColorDepth::Greyscale);
    let s = String::from_utf8_lossy(&result);
    assert!(s.contains("38;5;"), "should use 256-color greyscale index");
    // Extract the index
    let params_str = &s[2..s.len() - 1];
    let parts: Vec<&str> = params_str.split(';').collect();
    if let Some(idx_str) = parts.get(2) {
        let idx: u8 = idx_str.parse().expect("should be numeric");
        assert!(
            (232..=255).contains(&idx),
            "greyscale index should be 232-255, got {idx}"
        );
    }
}

// ── ColorDepth enum (expected public API) ───────────────────────────

use strip_ansi::downgrade::ColorDepth;

#[test]
fn color_depth_ordering() {
    // Mono < Greyscale < Color16 < Color256 < Truecolor
    assert!((ColorDepth::Mono as u8) < (ColorDepth::Greyscale as u8));
    assert!((ColorDepth::Greyscale as u8) < (ColorDepth::Color16 as u8));
    assert!((ColorDepth::Color16 as u8) < (ColorDepth::Color256 as u8));
    assert!((ColorDepth::Color256 as u8) < (ColorDepth::Truecolor as u8));
}
