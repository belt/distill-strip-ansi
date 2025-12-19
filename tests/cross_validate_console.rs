#![cfg(feature = "cross-validate")]

use proptest::prelude::*;

// Compare our strip with console::strip_ansi_codes on well-formed CSI.
// Note: console operates on &str, so we only test valid UTF-8 input.
proptest! {
    #![proptest_config(ProptestConfig { cases: 200, ..Default::default() })]
    #[test]
    fn strip_matches_console_on_csi(
        prefix in "[ -~]{0,64}",
        code in 0u8..=49,
        suffix in "[ -~]{0,64}",
    ) {
        let input = format!("{prefix}\x1b[{code}m{suffix}");

        let ours = strip_ansi::strip(input.as_bytes());
        let ours_str = String::from_utf8_lossy(&ours);
        let theirs = console::strip_ansi_codes(&input);

        prop_assert_eq!(ours_str.as_ref(), theirs.as_ref(),
            "Our strip should match console on CSI sequences");
    }
}

// Compare on clean input (no escapes).
proptest! {
    #![proptest_config(ProptestConfig { cases: 100, ..Default::default() })]
    #[test]
    fn strip_matches_console_on_clean(
        input in "[ -~]{0,256}",
    ) {
        let ours = strip_ansi::strip(input.as_bytes());
        let ours_str = String::from_utf8_lossy(&ours);
        let theirs = console::strip_ansi_codes(&input);

        prop_assert_eq!(ours_str.as_ref(), theirs.as_ref(),
            "Clean input should be identical across crates");
    }
}
