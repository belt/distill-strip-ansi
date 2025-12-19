#![cfg(feature = "cross-validate")]

use proptest::prelude::*;

// Compare our strip with fast-strip-ansi on well-formed CSI sequences.
proptest! {
    #![proptest_config(ProptestConfig { cases: 200, ..Default::default() })]
    #[test]
    fn strip_matches_fast_strip_on_csi(
        prefix in "[ -~]{0,64}",
        code in 0u8..=49,
        suffix in "[ -~]{0,64}",
    ) {
        let mut input = prefix.as_bytes().to_vec();
        input.extend_from_slice(b"\x1b[");
        input.extend_from_slice(code.to_string().as_bytes());
        input.push(b'm');
        input.extend_from_slice(suffix.as_bytes());

        let ours = strip_ansi::strip(&input);
        let theirs = fast_strip_ansi::strip_ansi_bytes(&input);

        prop_assert_eq!(&*ours, &*theirs,
            "Our strip should match fast-strip-ansi on CSI sequences");
    }
}

// Compare on OSC sequences (hyperlinks, titles).
proptest! {
    #![proptest_config(ProptestConfig { cases: 100, ..Default::default() })]
    #[test]
    fn strip_matches_fast_strip_on_osc(
        prefix in "[ -~]{0,32}",
        osc_num in 0u8..=20,
        payload in "[ -~]{0,64}",
        suffix in "[ -~]{0,32}",
    ) {
        let mut input = prefix.as_bytes().to_vec();
        input.extend_from_slice(b"\x1b]");
        input.extend_from_slice(osc_num.to_string().as_bytes());
        input.push(b';');
        input.extend_from_slice(payload.as_bytes());
        input.push(0x07); // BEL terminator
        input.extend_from_slice(suffix.as_bytes());

        let ours = strip_ansi::strip(&input);
        let theirs = fast_strip_ansi::strip_ansi_bytes(&input);

        prop_assert_eq!(&*ours, &*theirs,
            "Our strip should match fast-strip-ansi on OSC sequences");
    }
}

// Compare on clean input (no escapes — fast path).
proptest! {
    #![proptest_config(ProptestConfig { cases: 100, ..Default::default() })]
    #[test]
    fn strip_matches_fast_strip_on_clean(
        input in "[ -~]{0,256}",
    ) {
        let bytes = input.as_bytes();
        let ours = strip_ansi::strip(bytes);
        let theirs = fast_strip_ansi::strip_ansi_bytes(bytes);

        prop_assert_eq!(&*ours, &*theirs,
            "Clean input should be identical across crates");
    }
}
