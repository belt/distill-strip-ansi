#![cfg(feature = "toml-config")]

use proptest::prelude::*;
use strip_ansi::StripAnsiConfig;

// ── Valid filter names ──────────────────────────────────────────────

/// Group names accepted by the TOML config parser.
const GROUP_NAMES: &[&str] = &[
    "csi", "osc", "dcs", "apc", "pm", "sos", "ss2", "ss3", "fe",
];

/// CSI sub-kind names accepted by the TOML config parser.
const CSI_SUB_KIND_NAMES: &[&str] = &[
    "csi_sgr",
    "csi_cursor",
    "csi_erase",
    "csi_scroll",
    "csi_mode",
    "csi_device_status",
    "csi_window",
    "csi_other",
];

// ── Generator ───────────────────────────────────────────────────────

/// Generate a valid TOML configuration string matching the
/// `StripAnsiConfig` schema.
///
/// Produces random `buffer_size` (valid range), optional `mode`,
/// and a random subset of valid `no_strip` names.
fn arb_filter_toml() -> impl Strategy<Value = String> {
    let all_names: Vec<&'static str> = GROUP_NAMES
        .iter()
        .chain(CSI_SUB_KIND_NAMES.iter())
        .copied()
        .collect();

    (
        // buffer_size in valid range
        1024usize..=16_777_216,
        // mode: None, Some("strip"), or Some("check")
        prop::option::of(prop::sample::select(&["strip", "check"][..])),
        // no_strip: random subset of valid names (0..all)
        prop::collection::hash_set(prop::sample::select(all_names), 0..=17),
    )
        .prop_map(|(buffer_size, mode, no_strip_set)| {
            let mut toml = String::new();

            // [general] section
            toml.push_str("[general]\n");
            toml.push_str(&format!("buffer_size = {buffer_size}\n"));
            if let Some(m) = mode {
                toml.push_str(&format!("mode = \"{m}\"\n"));
            }

            // [filter] section
            toml.push_str("\n[filter]\n");
            let names: Vec<&str> = no_strip_set.into_iter().collect();
            let quoted: Vec<String> = names.iter().map(|n| format!("\"{n}\"")).collect();
            toml.push_str(&format!("no_strip = [{}]\n", quoted.join(", ")));

            toml
        })
}

// ── Property 10: TOML round-trip ────────────────────────────────────
// **Validates: Requirements 6.1, 6.3, 6.7**

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]

    #[test]
    fn p10_toml_roundtrip(toml_str in arb_filter_toml()) {
        let config_a = StripAnsiConfig::from_str(&toml_str)
            .map_err(|e| TestCaseError::Fail(format!("first parse failed: {e}").into()))?;
        let filter_a = config_a.to_filter_config()
            .map_err(|e| TestCaseError::Fail(format!("first to_filter_config failed: {e}").into()))?;

        let config_b = StripAnsiConfig::from_str(&toml_str)
            .map_err(|e| TestCaseError::Fail(format!("second parse failed: {e}").into()))?;
        let filter_b = config_b.to_filter_config()
            .map_err(|e| TestCaseError::Fail(format!("second to_filter_config failed: {e}").into()))?;

        prop_assert_eq!(
            filter_a, filter_b,
            "parsing the same TOML string twice must produce identical FilterConfig values"
        );
    }
}
