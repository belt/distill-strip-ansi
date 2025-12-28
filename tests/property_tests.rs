use assert_cmd::Command;
use proptest::prelude::*;

// Feature: strip-ansi, Property 1: Stripping removes all ANSI and preserves non-ANSI content
proptest! {
    #[test]
    fn strip_removes_all_ansi(input in prop::collection::vec(any::<u8>(), 0..4096)) {
        let input_str = String::from_utf8_lossy(&input);

        let stripped = console::strip_ansi_codes(&input_str);

        let stripped_bytes = stripped.as_bytes();

        prop_assert!(!stripped_bytes.contains(&0x1B),
            "Output should not contain any ESC bytes");

        let mut expected_non_ansi: Vec<u8> = Vec::new();
        let mut i = 0;
        while i < input.len() {
            if input[i] == 0x1B {
                let (seq_len, _) = parse_ansi_sequence(&input, i);
                i += seq_len;
            } else {
                expected_non_ansi.push(input[i]);
                i += 1;
            }
        }

        prop_assert_eq!(stripped_bytes, &expected_non_ansi[..],
            "Non-ANSI bytes should be preserved in order");
    }
}

// Feature: strip-ansi, Property 2: Pass-through identity
proptest! {
    #[test]
    fn passthrough_identity(input in prop::collection::vec(
        prop::num::u8::range(0x00..0x1B).union(prop::num::u8::range(0x1C..=0xFF)),
        0..4096
    )) {
        let input_str = String::from_utf8_lossy(&input);

        let stripped = console::strip_ansi_codes(&input_str);

        prop_assert_eq!(stripped.as_bytes(), input.as_slice(),
            "Input without ANSI should pass through unchanged");
    }
}

// Feature: strip-ansi, Property 3: Check mode correctness
proptest! {
    #[test]
    fn check_mode_correctness(input in prop::collection::vec(any::<u8>(), 0..4096)) {
        let has_ansi = input.contains(&0x1B);

        let mut cmd = Command::cargo_bin("strip-ansi")?;
        cmd.arg("--check")
            .write_stdin(&input)
            .assert()
            .success();

        let output = cmd.get_output();

        prop_assert!(output.stdout.is_empty(),
            "stdout should always be empty in check mode");

        if has_ansi {
            prop_assert!(!output.status.success(),
                "exit code should be 1 when ANSI sequences are present");
        } else {
            prop_assert!(output.status.success(),
                "exit code should be 0 when no ANSI sequences are present");
        }
    }
}

// Feature: strip-ansi, Property 4: Arbitrary bytes never panic
proptest! {
    #[test]
    fn arbitrary_bytes_no_panic(input in prop::collection::vec(any::<u8>(), 0..4096)) {
        let input_str = String::from_utf8_lossy(&input);

        let _stripped = console::strip_ansi_codes(&input_str);

        let _has_ansi = input.contains(&0x1B);
    }
}

fn parse_ansi_sequence(input: &[u8], start: usize) -> (usize, bool) {
    if start >= input.len() || input[start] != 0x1B {
        return (0, false);
    }

    let mut i = start + 1;

    if i < input.len() && input[i] == b'[' {
        i += 1;
        while i < input.len() && (input[i] >= b'0' && input[i] <= b'9' || input[i] == b';') {
            i += 1;
        }
        if i < input.len() && input[i] >= 0x40 && input[i] <= 0x7E {
            return (i - start + 1, true);
        }
    } else if i < input.len() && input[i] == b']' {
        i += 1;
        while i < input.len() && input[i] != 0x07 && input[i] != 0x1B {
            i += 1;
        }
        if i < input.len() {
            if input[i] == 0x07 {
                return (i - start + 1, true);
            }
            if input[i] == 0x1B && i + 1 < input.len() && input[i + 1] == b'\\' {
                return (i - start + 2, true);
            }
        }
    }

    (0, false)
}
