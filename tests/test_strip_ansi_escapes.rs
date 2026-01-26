use proptest::prelude::*;

// Compare our strip with strip-ansi-escapes on well-formed CSI sequences
proptest! {
    #![proptest_config(ProptestConfig { cases: 100, ..Default::default() })]
    #[test]
    fn strip_matches_ecosystem_on_csi(
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
        let theirs = strip_ansi_escapes::strip(&input);

        prop_assert_eq!(&*ours, &*theirs,
            "Our strip should match strip-ansi-escapes on CSI sequences");
    }
}

#[test]
fn strip_basic() {
    let input = b"\x1b[32mfoo\x1b[m bar";
    let stripped = strip_ansi::strip(input);
    assert_eq!(&*stripped, b"foo bar");

    // Idempotent
    let double = strip_ansi::strip(&stripped);
    assert_eq!(&*stripped, &*double);
}

#[test]
fn strip_isolated_esc() {
    let input = b"\x1b";
    let stripped = strip_ansi::strip(input);
    let double = strip_ansi::strip(&stripped);
    assert_eq!(&*stripped, &*double, "Should be idempotent");
}

#[test]
fn strip_c1_passthrough() {
    // C1 bytes (0x80-0x9F) should pass through as content.
    let input = b"\xc2\x9b31m";
    let stripped = strip_ansi::strip(input);
    // C1 bytes are content, not escape introducers.
    assert_eq!(&*stripped, input, "C1 bytes should pass through");
}
