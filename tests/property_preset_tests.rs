#![cfg(feature = "filter")]

use proptest::prelude::*;
use strip_ansi::{TerminalPreset, filter_strip};

// ── Generators ──────────────────────────────────────────────────────

/// Generate a well-formed CSI sequence: ESC [ params final_byte
fn arb_ansi_csi() -> impl Strategy<Value = Vec<u8>> {
    (
        0u8..50,
        prop::collection::vec(0x30u8..=0x3F, 0..4),
        0x40u8..=0x7E,
    )
        .prop_map(|(code, params, final_byte)| {
            let mut seq = vec![0x1B, b'['];
            seq.extend_from_slice(code.to_string().as_bytes());
            for p in params {
                seq.push(b';');
                seq.push(p);
            }
            seq.push(final_byte);
            seq
        })
}

/// Generate a well-formed OSC sequence (BEL terminated).
fn arb_ansi_osc() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(0x20u8..=0x7E, 0..32).prop_map(|body| {
        let mut seq = vec![0x1B, b']'];
        seq.extend_from_slice(&body);
        seq.push(0x07);
        seq
    })
}

/// Generate a well-formed DCS sequence: ESC P body ESC \
fn arb_ansi_dcs() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(0x20u8..=0x7E, 0..32).prop_map(|body| {
        let mut seq = vec![0x1B, b'P'];
        seq.extend_from_slice(&body);
        seq.push(0x1B);
        seq.push(b'\\');
        seq
    })
}

/// Generate a well-formed APC sequence: ESC _ body ESC \
fn arb_ansi_apc() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(0x20u8..=0x7E, 0..32).prop_map(|body| {
        let mut seq = vec![0x1B, b'_'];
        seq.extend_from_slice(&body);
        seq.push(0x1B);
        seq.push(b'\\');
        seq
    })
}

/// Generate a well-formed PM sequence: ESC ^ body ESC \
fn arb_ansi_pm() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(0x20u8..=0x7E, 0..32).prop_map(|body| {
        let mut seq = vec![0x1B, b'^'];
        seq.extend_from_slice(&body);
        seq.push(0x1B);
        seq.push(b'\\');
        seq
    })
}

/// Generate a well-formed SOS sequence: ESC X body ESC \
fn arb_ansi_sos() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(0x20u8..=0x7E, 0..32).prop_map(|body| {
        let mut seq = vec![0x1B, b'X'];
        seq.extend_from_slice(&body);
        seq.push(0x1B);
        seq.push(b'\\');
        seq
    })
}

/// Generate an arbitrary well-formed ANSI sequence.
fn arb_ansi_sequence() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        arb_ansi_csi(),
        arb_ansi_osc(),
        arb_ansi_dcs(),
        arb_ansi_apc(),
        arb_ansi_pm(),
        arb_ansi_sos(),
        // SS2: ESC N <one byte>
        any::<u8>().prop_map(|b| vec![0x1B, b'N', b]),
        // SS3: ESC O <one byte>
        any::<u8>().prop_map(|b| vec![0x1B, b'O', b]),
        // Fe: ESC + single final byte (excluding multi-byte introducers)
        (0x40u8..=0x5F)
            .prop_filter("exclude multi-byte Fe introducers", |&b| {
                !matches!(b, b'[' | b']' | b'P' | b'N' | b'O' | b'_' | b'^' | b'X')
            })
            .prop_map(|b| vec![0x1B, b]),
    ]
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Classify a sequence and check if a preset preserves it using
/// the full detail-aware strip decision.
fn preset_preserves_seq(preset: TerminalPreset, seq: &[u8]) -> bool {
    let config = preset.to_filter_config();
    let result = filter_strip(seq, &config);
    // If the result contains ESC, the sequence was preserved.
    result.contains(&0x1B)
}

// ── P5: Auto-detect never exceeds sanitize ──────────────────────────
// **Validates: Requirements AC-4.4**

#[test]
fn p5_detect_preset_never_exceeds_sanitize() {
    // Since detect_preset() depends on the runtime environment,
    // we verify the return value is within the safe set.
    let preset = strip_ansi::detect_preset();
    assert!(
        matches!(
            preset,
            TerminalPreset::Dumb
                | TerminalPreset::Color
                | TerminalPreset::Vt100
                | TerminalPreset::Sanitize
        ),
        "detect_preset() returned {:?}, which exceeds sanitize ceiling",
        preset
    );
}

#[test]
fn p5_detect_preset_untrusted_never_exceeds_sanitize() {
    let preset = strip_ansi::detect_preset_untrusted();
    assert!(
        matches!(
            preset,
            TerminalPreset::Dumb
                | TerminalPreset::Color
                | TerminalPreset::Vt100
                | TerminalPreset::Sanitize
        ),
        "detect_preset_untrusted() returned {:?}, which exceeds sanitize ceiling",
        preset
    );
}

// ── P3: Preset gradient subset chain ────────────────────────────────
// **Validates: Requirements P3**
//
// For each adjacent pair in the gradient, verify that every sequence
// preserved by the lower preset is also preserved by the higher preset.
//
// The gradient is: dumb ⊂ color ⊂ vt100 ⊂ tmux ⊂ sanitize ⊂ xterm ⊂ full
//
// Note: tmux → sanitize is NOT a strict subset. Tmux preserves ALL CSI
// (including CsiQuery, CsiDeviceStatus) via group bit, while Sanitize
// strips those dangerous kinds but adds OSC sub-type preservation.
// We test the pairs that hold strictly.

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..Default::default() })]

    /// dumb ⊂ color: everything dumb preserves, color also preserves.
    #[test]
    fn p3_gradient_dumb_subset_color(seq in arb_ansi_sequence()) {
        if preset_preserves_seq(TerminalPreset::Dumb, &seq) {
            prop_assert!(
                preset_preserves_seq(TerminalPreset::Color, &seq),
                "dumb preserves sequence but color strips it: {:?}", seq
            );
        }
    }

    /// color ⊂ vt100
    #[test]
    fn p3_gradient_color_subset_vt100(seq in arb_ansi_sequence()) {
        if preset_preserves_seq(TerminalPreset::Color, &seq) {
            prop_assert!(
                preset_preserves_seq(TerminalPreset::Vt100, &seq),
                "color preserves sequence but vt100 strips it: {:?}", seq
            );
        }
    }

    /// vt100 ⊂ tmux
    #[test]
    fn p3_gradient_vt100_subset_tmux(seq in arb_ansi_sequence()) {
        if preset_preserves_seq(TerminalPreset::Vt100, &seq) {
            prop_assert!(
                preset_preserves_seq(TerminalPreset::Tmux, &seq),
                "vt100 preserves sequence but tmux strips it: {:?}", seq
            );
        }
    }

    /// sanitize ⊂ xterm
    #[test]
    fn p3_gradient_sanitize_subset_xterm(seq in arb_ansi_sequence()) {
        if preset_preserves_seq(TerminalPreset::Sanitize, &seq) {
            prop_assert!(
                preset_preserves_seq(TerminalPreset::Xterm, &seq),
                "sanitize preserves sequence but xterm strips it: {:?}", seq
            );
        }
    }

    /// xterm ⊂ full
    #[test]
    fn p3_gradient_xterm_subset_full(seq in arb_ansi_sequence()) {
        if preset_preserves_seq(TerminalPreset::Xterm, &seq) {
            prop_assert!(
                preset_preserves_seq(TerminalPreset::Full, &seq),
                "xterm preserves sequence but full strips it: {:?}", seq
            );
        }
    }
}

// ── P6: --unsafe required for presets above sanitize ────────────────
// **Validates: Requirements P6**
//
// For all presets above sanitize (xterm, full), the requires_unsafe()
// method must return true. For all presets at or below sanitize, it
// must return false.

/// All presets in the gradient.
fn all_presets() -> Vec<TerminalPreset> {
    vec![
        TerminalPreset::Dumb,
        TerminalPreset::Color,
        TerminalPreset::Vt100,
        TerminalPreset::Tmux,
        TerminalPreset::Sanitize,
        TerminalPreset::Xterm,
        TerminalPreset::Full,
    ]
}

/// Strategy that generates a preset above sanitize.
fn arb_unsafe_preset() -> impl Strategy<Value = TerminalPreset> {
    prop_oneof![Just(TerminalPreset::Xterm), Just(TerminalPreset::Full),]
}

/// Strategy that generates a preset at or below sanitize.
fn arb_safe_preset() -> impl Strategy<Value = TerminalPreset> {
    prop_oneof![
        Just(TerminalPreset::Dumb),
        Just(TerminalPreset::Color),
        Just(TerminalPreset::Vt100),
        Just(TerminalPreset::Tmux),
        Just(TerminalPreset::Sanitize),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 100, ..Default::default() })]

    /// Presets above sanitize require --unsafe.
    #[test]
    fn p6_unsafe_presets_require_flag(preset in arb_unsafe_preset()) {
        prop_assert!(
            preset.requires_unsafe(),
            "preset {:?} is above sanitize but requires_unsafe() returned false",
            preset
        );
    }

    /// Presets at or below sanitize do NOT require --unsafe.
    #[test]
    fn p6_safe_presets_do_not_require_flag(preset in arb_safe_preset()) {
        prop_assert!(
            !preset.requires_unsafe(),
            "preset {:?} is at or below sanitize but requires_unsafe() returned true",
            preset
        );
    }
}

/// Exhaustive check: the partition is correct.
#[test]
fn p6_requires_unsafe_partition_exhaustive() {
    for preset in all_presets() {
        let expected = matches!(preset, TerminalPreset::Xterm | TerminalPreset::Full);
        assert_eq!(
            preset.requires_unsafe(),
            expected,
            "requires_unsafe() mismatch for {:?}",
            preset
        );
    }
}
