use proptest::prelude::*;

/// Generate arbitrary bytes excluding 0x1B (ESC).
fn arb_clean_bytes() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec((0u8..=0x1A).prop_union(0x1Cu8..=0xFF), 0..4096)
}

/// Generate a well-formed CSI sequence.
fn arb_ansi_csi() -> impl Strategy<Value = Vec<u8>> {
    (0u8..50, prop::collection::vec(0x30u8..=0x3F, 0..4), 0x40u8..=0x7E).prop_map(
        |(code, params, final_byte)| {
            let mut seq = vec![0x1B, b'['];
            seq.extend_from_slice(code.to_string().as_bytes());
            for p in params {
                seq.push(b';');
                seq.push(p);
            }
            seq.push(final_byte);
            seq
        },
    )
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

/// Generate an arbitrary well-formed ANSI sequence.
fn arb_ansi_sequence() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        arb_ansi_csi(),
        arb_ansi_osc(),
        // SS2
        any::<u8>().prop_map(|b| vec![0x1B, b'N', b]),
        // SS3
        any::<u8>().prop_map(|b| vec![0x1B, b'O', b]),
        // Fe (excluding CSI '[', OSC ']', DCS 'P', SS2 'N', SS3 'O',
        // APC '_', PM '^', SOS 'X' which have multi-byte bodies)
        (0x40u8..=0x5F)
            .prop_filter("exclude multi-byte Fe introducers", |&b| {
                !matches!(b, b'[' | b']' | b'P' | b'N' | b'O' | b'_' | b'^' | b'X')
            })
            .prop_map(|b| vec![0x1B, b]),
    ]
}

// P1: Idempotency — strip(strip(x)) == strip(x)
proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]
    #[test]
    fn p1_strip_idempotent(input in prop::collection::vec(any::<u8>(), 0..4096)) {
        let stripped = strip_ansi::strip(&input);
        let double_stripped = strip_ansi::strip(&stripped);
        prop_assert_eq!(&*stripped, &*double_stripped);
    }
}

// P2: Preservation — strip(x) is a subsequence of x
proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]
    #[test]
    fn p2_strip_preserves_content(input in prop::collection::vec(any::<u8>(), 0..4096)) {
        let stripped = strip_ansi::strip(&input);
        // Every byte in stripped must appear in input in order.
        let mut it = input.iter();
        for &b in stripped.iter() {
            prop_assert!(it.any(|&x| x == b),
                "stripped byte 0x{:02X} not found in input subsequence", b);
        }
    }
}

// P3: Never grows — strip(x).len() <= x.len()
proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]
    #[test]
    fn p3_strip_never_grows(input in prop::collection::vec(any::<u8>(), 0..4096)) {
        let stripped = strip_ansi::strip(&input);
        prop_assert!(stripped.len() <= input.len());
    }
}

// P4: Clean identity — no ESC => strip(x) == x
proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]
    #[test]
    fn p4_clean_identity(input in arb_clean_bytes()) {
        let stripped = strip_ansi::strip(&input);
        prop_assert_eq!(&*stripped, &*input);
    }
}

// Legacy alias for backward compat with existing proptest regressions
proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]
    #[test]
    fn strip_idempotent(input in prop::collection::vec(any::<u8>(), 0..4096)) {
        let stripped = strip_ansi::strip(&input);
        let double_stripped = strip_ansi::strip(&stripped);
        prop_assert_eq!(&*stripped, &*double_stripped);
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]
    #[test]
    fn passthrough_identity(s in "[ -~]{0,1024}") {
        let input = s.as_bytes();
        let stripped = strip_ansi::strip(input);
        prop_assert_eq!(&*stripped, input);
    }
}

// Property: Check mode detects well-formed ANSI sequences
proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]
    #[test]
    fn check_mode_correctness(
        prefix in prop::collection::vec(0x20..=0x7Eu8, 0..64),
        sgr_code in 0u8..=49,
        suffix in prop::collection::vec(0x20..=0x7Eu8, 0..64),
    ) {
        // Build input with a well-formed CSI SGR sequence.
        let mut input = prefix.clone();
        input.extend_from_slice(b"\x1b[");
        input.extend_from_slice(sgr_code.to_string().as_bytes());
        input.push(b'm');
        input.extend_from_slice(&suffix);

        let output = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("strip-ansi"))
            .arg("--check")
            .write_stdin(input)
            .timeout(std::time::Duration::from_secs(5))
            .output()?;

        prop_assert!(output.stdout.is_empty(),
            "stdout should always be empty in check mode");
        prop_assert!(!output.status.success(),
            "exit code should be 1 when ANSI sequences are present");
    }
}

// P6: Arbitrary bytes never panic
proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]
    #[test]
    fn arbitrary_bytes_no_panic(input in prop::collection::vec(any::<u8>(), 0..4096)) {
        let _stripped = strip_ansi::strip(&input);
        let _contains = strip_ansi::contains_ansi(&input);
        let mut buf = input.clone();
        let _ = strip_ansi::strip_in_place(&mut buf);
        let mut out = Vec::new();
        strip_ansi::strip_into(&input, &mut out);
        let mut stream = strip_ansi::StripStream::new();
        for slice in stream.strip_slices(&input) {
            let _ = slice;
        }
    }
}

// P7: Known stripped — prefix + well-formed ANSI + suffix → prefix + suffix
proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]
    #[test]
    fn p7_known_stripped(
        prefix in arb_clean_bytes().prop_map(|v| {
            v.into_iter().take(64).collect::<Vec<u8>>()
        }),
        seq in arb_ansi_sequence(),
        suffix in arb_clean_bytes().prop_map(|v| {
            v.into_iter().take(64).collect::<Vec<u8>>()
        }),
    ) {
        let mut input = prefix.clone();
        input.extend_from_slice(&seq);
        input.extend_from_slice(&suffix);

        let stripped = strip_ansi::strip(&input);
        let mut expected = prefix.clone();
        expected.extend_from_slice(&suffix);
        prop_assert_eq!(&*stripped, &*expected);
    }
}

// P9: strip_into equivalence — strip_into(x) == strip(x)
proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]
    #[test]
    fn p9_strip_into_eq(input in prop::collection::vec(any::<u8>(), 0..4096)) {
        let expected = strip_ansi::strip(&input);
        let mut out = Vec::new();
        strip_ansi::strip_into(&input, &mut out);
        prop_assert_eq!(&out, &*expected);
    }
}

// P10: strip_str equivalence — strip_str bytes == strip bytes
proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]
    #[test]
    fn p10_strip_str_eq(input in "\\PC{0,2048}") {
        let stripped_bytes = strip_ansi::strip(input.as_bytes());
        let stripped_str = strip_ansi::strip_str(&input);
        prop_assert_eq!(stripped_str.as_bytes(), &*stripped_bytes);
    }
}

#[test]
fn empty_input_produces_empty() {
    let mut cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("strip-ansi"));
    cmd.write_stdin("");

    let output = cmd.output().unwrap();

    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert!(output.status.success());
}

#[test]
fn check_stderr_diagnostic() {
    let mut cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("strip-ansi"));
    cmd.arg("--check")
        .write_stdin("\x1b[31mred\x1b[0m");

    let output = cmd.output().unwrap();

    assert!(output.stdout.is_empty());
    assert!(output.status.code() == Some(1));
    let stderr_str = String::from_utf8_lossy(&output.stderr);
    assert!(stderr_str.contains("strip-ansi: ANSI escape sequences detected"));
}

#[test]
fn broken_pipe_exit_zero() {
    use std::process::Command;

    let bin = assert_cmd::cargo::cargo_bin!("strip-ansi");
    let bin_str = bin.display();
    let child = Command::new("sh")
        .arg("-c")
        .arg(format!("echo test | '{bin_str}' > /dev/null"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .spawn()
        .expect("Failed to spawn child process");

    let output = child.wait_with_output().expect("Failed to read output");
    assert!(output.status.code() == Some(0));
}

#[test]
fn broken_pipe_no_panic_stderr() {
    use std::process::Command;

    let bin = assert_cmd::cargo::cargo_bin!("strip-ansi");
    let bin_str = bin.display();
    let child = Command::new("sh")
        .arg("-c")
        .arg(format!("echo test | '{bin_str}' > /dev/null"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .spawn()
        .expect("Failed to spawn child process");

    let output = child.wait_with_output().expect("Failed to read output");
    let stderr_str = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr_str.contains("BrokenPipe"));
    assert!(!stderr_str.contains("panicked"));
}
