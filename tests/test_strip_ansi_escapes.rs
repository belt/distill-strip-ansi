use proptest::prelude::*;

// Test if strip-ansi-escapes is idempotent
proptest! {
    #[test]
    fn strip_ansi_escapes_is_idempotent(input in prop::collection::vec(any::<u8>(), 0..4096)) {
        let stripped = strip_ansi_escapes::strip(&input);
        let double_stripped = strip_ansi_escapes::strip(&stripped);

        prop_assert_eq!(stripped, double_stripped,
            "strip-ansi-escapes should be idempotent");
    }
}

#[test]
fn strip_ansi_escapes_basic() {
    let input = b"\x1b[32mfoo\x1b[m bar";
    let stripped = strip_ansi_escapes::strip(input);
    assert_eq!(stripped, b"foo bar");

    // Test idempotency
    let double = strip_ansi_escapes::strip(&stripped);
    assert_eq!(stripped, double);
}

#[test]
fn strip_ansi_escapes_isolated_esc() {
    let input = b"\x1b";
    let stripped = strip_ansi_escapes::strip(input);
    let double = strip_ansi_escapes::strip(&stripped);
    assert_eq!(stripped, double, "Should be idempotent");
}

#[test]
fn strip_ansi_escapes_c1_csi() {
    // C1 CSI sequence (0x9B in UTF-8 is [194, 155])
    let input = b"\xc2\x9b31m";
    let stripped = strip_ansi_escapes::strip(input);
    let double = strip_ansi_escapes::strip(&stripped);
    assert_eq!(stripped, double, "Should be idempotent");
}
