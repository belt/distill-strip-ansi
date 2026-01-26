use proptest::prelude::*;

// Property: Stripping via our library is idempotent
proptest! {
    #![proptest_config(ProptestConfig { cases: 100, ..Default::default() })]
    #[test]
    fn strip_idempotent(input in prop::collection::vec(any::<u8>(), 0..4096)) {
        let stripped = strip_ansi::strip(&input);
        let double_stripped = strip_ansi::strip(&stripped);
        prop_assert_eq!(&*stripped, &*double_stripped,
            "Stripping should be idempotent");
    }
}

// Property: Clean ASCII passes through unchanged
proptest! {
    #![proptest_config(ProptestConfig { cases: 100, ..Default::default() })]
    #[test]
    fn passthrough_identity(s in "[ -~]{0,1024}") {
        let input = s.as_bytes();
        let stripped = strip_ansi::strip(input);
        prop_assert_eq!(&*stripped, input,
            "Printable ASCII without ANSI should pass through unchanged");
    }
}

// Property: Check mode detects well-formed ANSI sequences
proptest! {
    #![proptest_config(ProptestConfig::with_cases(8))]
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

// Property: Arbitrary bytes never panic
proptest! {
    #![proptest_config(ProptestConfig { cases: 100, ..Default::default() })]
    #[test]
    fn arbitrary_bytes_no_panic(input in prop::collection::vec(any::<u8>(), 0..4096)) {
        let _stripped = strip_ansi::strip(&input);
        let _contains = strip_ansi::contains_ansi(&input);
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

    let child = Command::new("sh")
        .arg("-c")
        .arg("echo test | cargo run --quiet --bin strip-ansi -- > /dev/null")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .spawn()
        .expect("Failed to spawn child process");

    let output = child.wait_with_output().expect("Failed to read output");
    assert!(output.status.code() == Some(0));
}

#[test]
fn broken_pipe_no_panic_stderr() {
    use std::process::Command;

    let child = Command::new("sh")
        .arg("-c")
        .arg("echo test | cargo run --quiet --bin strip-ansi -- > /dev/null")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .spawn()
        .expect("Failed to spawn child process");

    let output = child.wait_with_output().expect("Failed to read output");
    let stderr_str = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr_str.contains("BrokenPipe"));
    assert!(!stderr_str.contains("panicked"));
}
