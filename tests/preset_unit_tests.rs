#![cfg(feature = "filter")]

use strip_ansi::{filter_strip, SeqKind, TerminalPreset};

// ── Preset → FilterConfig mapping ───────────────────────────────────

#[test]
fn dumb_strips_everything() {
    let config = TerminalPreset::Dumb.to_filter_config();
    assert!(config.is_strip_all());
    // Verify every kind is stripped.
    for kind in all_kinds() {
        assert!(config.should_strip(kind), "dumb should strip {kind:?}");
    }
}

#[test]
fn color_preserves_only_sgr() {
    let config = TerminalPreset::Color.to_filter_config();
    assert!(!config.is_strip_all());
    assert!(!config.should_strip(SeqKind::CsiSgr));
    // Other CSI sub-kinds stripped (no_strip_kind does NOT set group bit).
    assert!(config.should_strip(SeqKind::CsiCursor));
    assert!(config.should_strip(SeqKind::CsiErase));
    assert!(config.should_strip(SeqKind::CsiScroll));
    assert!(config.should_strip(SeqKind::CsiMode));
    assert!(config.should_strip(SeqKind::CsiDeviceStatus));
    assert!(config.should_strip(SeqKind::CsiWindow));
    assert!(config.should_strip(SeqKind::CsiOther));
    // Non-CSI groups stripped.
    assert!(config.should_strip(SeqKind::Osc));
    assert!(config.should_strip(SeqKind::Dcs));
    assert!(config.should_strip(SeqKind::Fe));
}

#[test]
fn vt100_preserves_sgr_cursor_erase() {
    let config = TerminalPreset::Vt100.to_filter_config();
    assert!(!config.should_strip(SeqKind::CsiSgr));
    assert!(!config.should_strip(SeqKind::CsiCursor));
    assert!(!config.should_strip(SeqKind::CsiErase));
    // Other CSI sub-kinds stripped.
    assert!(config.should_strip(SeqKind::CsiScroll));
    assert!(config.should_strip(SeqKind::CsiMode));
    assert!(config.should_strip(SeqKind::CsiWindow));
    // Non-CSI stripped.
    assert!(config.should_strip(SeqKind::Osc));
    assert!(config.should_strip(SeqKind::Dcs));
    assert!(config.should_strip(SeqKind::Fe));
}

#[test]
fn tmux_preserves_all_csi_and_fe() {
    let config = TerminalPreset::Tmux.to_filter_config();
    // All CSI sub-kinds preserved.
    for kind in csi_kinds() {
        assert!(!config.should_strip(kind), "tmux should preserve {kind:?}");
    }
    // Fe preserved.
    assert!(!config.should_strip(SeqKind::Fe));
    // OSC, DCS, and string types stripped.
    assert!(config.should_strip(SeqKind::Osc));
    assert!(config.should_strip(SeqKind::Dcs));
    assert!(config.should_strip(SeqKind::Apc));
    assert!(config.should_strip(SeqKind::Pm));
    assert!(config.should_strip(SeqKind::Sos));
}

#[test]
fn xterm_preserves_csi_osc_fe() {
    let config = TerminalPreset::Xterm.to_filter_config();
    for kind in csi_kinds() {
        assert!(!config.should_strip(kind), "xterm should preserve {kind:?}");
    }
    assert!(!config.should_strip(SeqKind::Osc));
    assert!(!config.should_strip(SeqKind::Fe));
    // DCS and string types still stripped.
    assert!(config.should_strip(SeqKind::Dcs));
    assert!(config.should_strip(SeqKind::Apc));
    assert!(config.should_strip(SeqKind::Pm));
    assert!(config.should_strip(SeqKind::Sos));
}

#[test]
fn full_preserves_everything() {
    let config = TerminalPreset::Full.to_filter_config();
    assert!(config.is_pass_all());
    for kind in all_kinds() {
        assert!(!config.should_strip(kind), "full should preserve {kind:?}");
    }
}

// ── Preset gradient: each level is a superset of the previous ───────

#[test]
fn preset_gradient_is_monotonic() {
    // The gradient is: dumb ⊂ color ⊂ vt100 ⊂ tmux ⊂ sanitize ⊂ xterm ⊂ full
    //
    // Note: tmux ⊂ sanitize is NOT a strict subset at the kind level.
    // Tmux preserves ALL CSI (including CsiQuery, CsiDeviceStatus) via
    // group bit, while Sanitize strips those dangerous kinds. Sanitize
    // adds OSC sub-type preservation that Tmux lacks. The gradient is
    // conceptual — Sanitize is the security-aware replacement.
    //
    // We test the strict subset pairs and the sanitize ⊂ xterm ⊂ full chain.
    let strict_pairs: &[(TerminalPreset, TerminalPreset)] = &[
        (TerminalPreset::Dumb, TerminalPreset::Color),
        (TerminalPreset::Color, TerminalPreset::Vt100),
        (TerminalPreset::Vt100, TerminalPreset::Tmux),
        // tmux → sanitize: not strict (sanitize strips CsiQuery/CsiDeviceStatus)
        (TerminalPreset::Sanitize, TerminalPreset::Xterm),
        (TerminalPreset::Xterm, TerminalPreset::Full),
    ];
    for &(narrower_preset, wider_preset) in strict_pairs {
        let narrower = narrower_preset.to_filter_config();
        let wider = wider_preset.to_filter_config();
        for kind in all_kinds() {
            if !narrower.should_strip(kind) {
                assert!(
                    !wider.should_strip(kind),
                    "{:?} preserves {kind:?} but {:?} strips it — \
                     gradient violated",
                    narrower_preset, wider_preset,
                );
            }
        }
    }
}

// ── Preset name parsing ─────────────────────────────────────────────

#[test]
fn from_name_standard_names() {
    assert_eq!(TerminalPreset::from_name("dumb"), Some(TerminalPreset::Dumb));
    assert_eq!(TerminalPreset::from_name("color"), Some(TerminalPreset::Color));
    assert_eq!(TerminalPreset::from_name("vt100"), Some(TerminalPreset::Vt100));
    assert_eq!(TerminalPreset::from_name("tmux"), Some(TerminalPreset::Tmux));
    assert_eq!(TerminalPreset::from_name("sanitize"), Some(TerminalPreset::Sanitize));
    assert_eq!(TerminalPreset::from_name("xterm"), Some(TerminalPreset::Xterm));
    assert_eq!(TerminalPreset::from_name("full"), Some(TerminalPreset::Full));
}

#[test]
fn from_name_aliases() {
    assert_eq!(TerminalPreset::from_name("pipe"), Some(TerminalPreset::Dumb));
    assert_eq!(TerminalPreset::from_name("ci"), Some(TerminalPreset::Color));
    assert_eq!(TerminalPreset::from_name("pager"), Some(TerminalPreset::Color));
    assert_eq!(TerminalPreset::from_name("screen"), Some(TerminalPreset::Tmux));
    assert_eq!(TerminalPreset::from_name("safe"), Some(TerminalPreset::Sanitize));
    assert_eq!(TerminalPreset::from_name("modern"), Some(TerminalPreset::Full));
}

#[test]
fn from_name_case_insensitive() {
    assert_eq!(TerminalPreset::from_name("DUMB"), Some(TerminalPreset::Dumb));
    assert_eq!(TerminalPreset::from_name("Color"), Some(TerminalPreset::Color));
    assert_eq!(TerminalPreset::from_name("VT100"), Some(TerminalPreset::Vt100));
    assert_eq!(TerminalPreset::from_name("XTERM"), Some(TerminalPreset::Xterm));
}

#[test]
fn from_name_unknown_returns_none() {
    assert_eq!(TerminalPreset::from_name("bogus"), None);
    assert_eq!(TerminalPreset::from_name(""), None);
    assert_eq!(TerminalPreset::from_name("vt220"), None);
}

#[test]
fn name_roundtrips() {
    let presets = [
        TerminalPreset::Dumb,
        TerminalPreset::Color,
        TerminalPreset::Vt100,
        TerminalPreset::Tmux,
        TerminalPreset::Sanitize,
        TerminalPreset::Xterm,
        TerminalPreset::Full,
    ];
    for preset in presets {
        let name = preset.name();
        let parsed = TerminalPreset::from_name(name);
        assert_eq!(parsed, Some(preset), "roundtrip failed for {name}");
    }
}

#[test]
fn all_names_are_parseable() {
    for name in TerminalPreset::ALL_NAMES {
        assert!(
            TerminalPreset::from_name(name).is_some(),
            "ALL_NAMES contains unparseable entry: {name}",
        );
    }
}

// ── Echoback security properties ────────────────────────────────────

#[test]
fn dumb_strips_all_echoback_vectors() {
    // Only dumb (strip_all) guarantees all echoback vectors stripped.
    let echoback_kinds = [SeqKind::Dcs, SeqKind::Osc, SeqKind::CsiWindow];
    let config = TerminalPreset::Dumb.to_filter_config();
    for kind in echoback_kinds {
        assert!(
            config.should_strip(kind),
            "dumb should strip echoback vector {kind:?}",
        );
    }
}

#[test]
fn color_and_vt100_strip_all_echoback_vectors() {
    // With the no_strip_kind fix, color/vt100 now correctly strip
    // CsiWindow (title report) alongside DCS and OSC.
    let echoback_kinds = [SeqKind::Dcs, SeqKind::Osc, SeqKind::CsiWindow];
    for preset in [TerminalPreset::Color, TerminalPreset::Vt100] {
        let config = preset.to_filter_config();
        for kind in echoback_kinds {
            assert!(
                config.should_strip(kind),
                "{preset:?} should strip echoback vector {kind:?}",
            );
        }
    }
}

#[test]
fn tmux_strips_dcs_and_osc_echoback() {
    let config = TerminalPreset::Tmux.to_filter_config();
    assert!(config.should_strip(SeqKind::Dcs), "tmux should strip DCS (DECRQSS)");
    assert!(config.should_strip(SeqKind::Osc), "tmux should strip OSC (font query)");
    // CsiWindow is preserved (part of all-CSI group) — documented risk.
    assert!(!config.should_strip(SeqKind::CsiWindow));
}

#[test]
fn xterm_strips_dcs_but_preserves_osc() {
    let config = TerminalPreset::Xterm.to_filter_config();
    assert!(config.should_strip(SeqKind::Dcs), "xterm should strip DCS");
    // OSC preserved (includes dangerous OSC 50 — Phase 2 gap).
    assert!(!config.should_strip(SeqKind::Osc));
}

// ── Helpers ─────────────────────────────────────────────────────────

fn csi_kinds() -> Vec<SeqKind> {
    vec![
        SeqKind::CsiSgr,
        SeqKind::CsiCursor,
        SeqKind::CsiErase,
        SeqKind::CsiScroll,
        SeqKind::CsiMode,
        SeqKind::CsiDeviceStatus,
        SeqKind::CsiWindow,
        SeqKind::CsiQuery,
        SeqKind::CsiOther,
    ]
}

fn all_kinds() -> Vec<SeqKind> {
    vec![
        SeqKind::CsiSgr,
        SeqKind::CsiCursor,
        SeqKind::CsiErase,
        SeqKind::CsiScroll,
        SeqKind::CsiMode,
        SeqKind::CsiDeviceStatus,
        SeqKind::CsiWindow,
        SeqKind::CsiQuery,
        SeqKind::CsiOther,
        SeqKind::Osc,
        SeqKind::Dcs,
        SeqKind::Apc,
        SeqKind::Pm,
        SeqKind::Sos,
        SeqKind::Ss2,
        SeqKind::Ss3,
        SeqKind::Fe,
    ]
}

// ── Sanitize preset tests ───────────────────────────────────────────

#[test]
fn sanitize_preserves_safe_csi_kinds() {
    let config = TerminalPreset::Sanitize.to_filter_config();
    assert!(!config.should_strip(SeqKind::CsiSgr));
    assert!(!config.should_strip(SeqKind::CsiCursor));
    assert!(!config.should_strip(SeqKind::CsiErase));
    assert!(!config.should_strip(SeqKind::CsiScroll));
    assert!(!config.should_strip(SeqKind::CsiMode));
    assert!(!config.should_strip(SeqKind::CsiWindow));
    assert!(!config.should_strip(SeqKind::CsiOther));
}

#[test]
fn sanitize_strips_dangerous_csi_kinds() {
    let config = TerminalPreset::Sanitize.to_filter_config();
    assert!(config.should_strip(SeqKind::CsiQuery), "sanitize must strip CsiQuery");
    assert!(config.should_strip(SeqKind::CsiDeviceStatus), "sanitize must strip CsiDeviceStatus");
}

#[test]
fn sanitize_preserves_fe() {
    let config = TerminalPreset::Sanitize.to_filter_config();
    assert!(!config.should_strip(SeqKind::Fe));
}

#[test]
fn sanitize_strips_dcs_and_string_types() {
    let config = TerminalPreset::Sanitize.to_filter_config();
    assert!(config.should_strip(SeqKind::Dcs), "sanitize must strip Dcs");
    assert!(config.should_strip(SeqKind::Apc), "sanitize must strip Apc");
    assert!(config.should_strip(SeqKind::Pm), "sanitize must strip Pm");
    assert!(config.should_strip(SeqKind::Sos), "sanitize must strip Sos");
    assert!(config.should_strip(SeqKind::Ss2), "sanitize must strip Ss2");
    assert!(config.should_strip(SeqKind::Ss3), "sanitize must strip Ss3");
}

#[test]
fn sanitize_name_and_alias() {
    assert_eq!(TerminalPreset::Sanitize.name(), "sanitize");
    assert_eq!(TerminalPreset::from_name("sanitize"), Some(TerminalPreset::Sanitize));
    assert_eq!(TerminalPreset::from_name("safe"), Some(TerminalPreset::Sanitize));
    assert_eq!(TerminalPreset::from_name("SANITIZE"), Some(TerminalPreset::Sanitize));
    assert_eq!(TerminalPreset::from_name("Safe"), Some(TerminalPreset::Sanitize));
}

// ── 7.6: Sanitize strips all echoback vectors (P4) ─────────────────

/// Build a DECRQSS sequence: ESC P $ q <body> ESC \
fn decrqss(body: &[u8]) -> Vec<u8> {
    let mut v = vec![0x1B, b'P', b'$', b'q'];
    v.extend_from_slice(body);
    v.push(0x1B);
    v.push(b'\\');
    v
}

/// Build an OSC sequence with the given number and body, BEL-terminated.
fn osc(number: u16, body: &[u8]) -> Vec<u8> {
    let mut v = vec![0x1B, b']'];
    v.extend_from_slice(number.to_string().as_bytes());
    v.push(b';');
    v.extend_from_slice(body);
    v.push(0x07);
    v
}

/// Build a CSI sequence: ESC [ <params> <final>
fn csi(params: &[u8], final_byte: u8) -> Vec<u8> {
    let mut v = vec![0x1B, b'['];
    v.extend_from_slice(params);
    v.push(final_byte);
    v
}

#[test]
fn sanitize_strips_decrqss() {
    let config = TerminalPreset::Sanitize.to_filter_config();
    let seq = decrqss(b"m");
    let result = filter_strip(&seq, &config);
    assert!(result.is_empty(), "sanitize must strip DECRQSS (DCS $q)");
}

#[test]
fn sanitize_strips_osc_50_font_query() {
    let config = TerminalPreset::Sanitize.to_filter_config();
    let seq = osc(50, b"?");
    let result = filter_strip(&seq, &config);
    assert!(result.is_empty(), "sanitize must strip OSC 50 (font query)");
}

#[test]
fn sanitize_strips_osc_52_clipboard() {
    let config = TerminalPreset::Sanitize.to_filter_config();
    let seq = osc(52, b"c;dGVzdA==");
    let result = filter_strip(&seq, &config);
    assert!(result.is_empty(), "sanitize must strip OSC 52 (clipboard)");
}

#[test]
fn sanitize_strips_csi_21t_title_report() {
    let config = TerminalPreset::Sanitize.to_filter_config();
    let seq = csi(b"21", b't');
    let result = filter_strip(&seq, &config);
    assert!(result.is_empty(), "sanitize must strip CSI 21t (title report)");
}

#[test]
fn sanitize_strips_csi_6n_cursor_position() {
    let config = TerminalPreset::Sanitize.to_filter_config();
    let seq = csi(b"6", b'n');
    let result = filter_strip(&seq, &config);
    assert!(result.is_empty(), "sanitize must strip CSI 6n (cursor position report)");
}

#[test]
fn sanitize_preserves_safe_osc_types() {
    let config = TerminalPreset::Sanitize.to_filter_config();

    // OSC 0 (Title) → preserved
    let title = osc(0, b"My Window");
    assert_eq!(&*filter_strip(&title, &config), &title[..], "sanitize must preserve OSC Title");

    // OSC 8 (Hyperlink) → preserved
    let hyperlink = osc(8, b";https://example.com");
    assert_eq!(&*filter_strip(&hyperlink, &config), &hyperlink[..], "sanitize must preserve OSC Hyperlink");

    // OSC 9 (Notify) → preserved
    let notify = osc(9, b"notification");
    assert_eq!(&*filter_strip(&notify, &config), &notify[..], "sanitize must preserve OSC Notify");

    // OSC 7 (WorkingDir) → preserved
    let workdir = osc(7, b"file:///home/user");
    assert_eq!(&*filter_strip(&workdir, &config), &workdir[..], "sanitize must preserve OSC WorkingDir");

    // OSC 133 (ShellInteg) → preserved
    let shellinteg = osc(133, b"A");
    assert_eq!(&*filter_strip(&shellinteg, &config), &shellinteg[..], "sanitize must preserve OSC ShellInteg");
}

#[test]
fn sanitize_strips_all_echoback_vectors_comprehensive() {
    // P4: Every known echoback vector must be stripped by sanitize.
    let config = TerminalPreset::Sanitize.to_filter_config();

    let vectors: Vec<(&str, Vec<u8>)> = vec![
        ("DECRQSS", decrqss(b"m")),
        ("OSC 50", osc(50, b"?")),
        ("OSC 52", osc(52, b"c;dGVzdA==")),
        ("CSI 21t", csi(b"21", b't')),
        ("CSI 6n", csi(b"6", b'n')),
    ];

    for (name, seq) in &vectors {
        let result = filter_strip(seq, &config);
        assert!(
            result.is_empty(),
            "sanitize must strip echoback vector {name}, but got {:?}",
            result
        );
    }
}

// ── 7.7: Auto-detect never exceeds sanitize (P5) ───────────────────

#[test]
fn detect_preset_only_returns_safe_presets() {
    // P5: detect_preset() only returns Dumb, Color, Vt100, or Sanitize.
    // Since we can't control the environment in a unit test, we verify
    // the function's return value is within the safe set.
    let preset = strip_ansi::detect_preset();
    let safe_presets = [
        TerminalPreset::Dumb,
        TerminalPreset::Color,
        TerminalPreset::Vt100,
        TerminalPreset::Sanitize,
    ];
    assert!(
        safe_presets.contains(&preset),
        "detect_preset() returned {:?}, which exceeds sanitize ceiling",
        preset
    );
}

#[test]
fn detect_preset_untrusted_only_returns_safe_presets() {
    let preset = strip_ansi::detect_preset_untrusted();
    let safe_presets = [
        TerminalPreset::Dumb,
        TerminalPreset::Color,
        TerminalPreset::Vt100,
        TerminalPreset::Sanitize,
    ];
    assert!(
        safe_presets.contains(&preset),
        "detect_preset_untrusted() returned {:?}, which exceeds sanitize ceiling",
        preset
    );
}
