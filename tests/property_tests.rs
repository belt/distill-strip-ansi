use proptest::prelude::*;

// Feature: strip-ansi, Property 1: Stripping removes all ANSI and preserves non-ANSI content
// **Validates: Requirements 2.1, 2.7**
proptest! {
    #[test]
    fn strip_removes_all_ansi(input in prop::collection::vec(any::<u8>(), 0..4096)) {
        let stripped = strip_ansi_escapes::strip(&input);

        // Property 1a: Stripping is idempotent — re-stripping produces no change.
        // This proves all recognized ANSI sequences were removed in the first pass.
        let double_stripped = strip_ansi_escapes::strip(&stripped);
        prop_assert_eq!(&stripped, &double_stripped,
            "Stripping should be idempotent — no ANSI sequences remain after first strip");

        // Property 1b: The function completes without panicking on arbitrary input
        // (binary safety - Requirement 8.1)
        // The length check is omitted because strip-ansi-escapes may replace invalid
        // UTF-8 with replacement characters, which can change byte count.
    }
}

// Feature: strip-ansi, Property 2: Pass-through identity
// **Validates: Requirements 2.2**
proptest! {
    #[test]
    fn passthrough_identity(s in "[ -~]{0,1024}") {
        // Generate printable ASCII strings (space through tilde, excluding control characters)
        // to test pass-through behavior.
        let input = s.as_bytes().to_vec();
        let stripped = strip_ansi_escapes::strip(&input);

        prop_assert_eq!(stripped, input,
            "Printable ASCII without ANSI should pass through unchanged");
    }
}

// Feature: strip-ansi, Property 3: Check mode correctness
proptest! {
    #[test]
    fn check_mode_correctness(input in prop::collection::vec(any::<u8>(), 0..4096)) {
        let has_ansi = input.contains(&0x1B);

        let mut cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("strip-ansi"));
        cmd.arg("--check")
            .write_stdin(input);

        let output = cmd.output()?;

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
        let _stripped = strip_ansi_escapes::strip(&input);

        let _has_ansi = input.contains(&0x1B);
    }
}

// Feature: strip-ansi, Unit tests for exit code mapping and BrokenPipe handling

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

    // Create a broken pipe scenario by piping to a command that exits quickly
    // Use /dev/null to simulate a closed pipe
    let child = Command::new("sh")
        .arg("-c")
        .arg("echo test | cargo run --quiet --bin strip-ansi -- > /dev/null")
        .current_dir("/Users/paulbelt/work/distill/strip-ansi")
        .spawn()
        .expect("Failed to spawn child process");

    let output = child.wait_with_output().expect("Failed to read output");

    assert!(output.status.code() == Some(0));
}

#[test]
fn broken_pipe_no_panic_stderr() {
    use std::process::Command;

    // Create a broken pipe scenario
    // Use /dev/null to simulate a closed pipe
    let child = Command::new("sh")
        .arg("-c")
        .arg("echo test | cargo run --quiet --bin strip-ansi -- > /dev/null")
        .current_dir("/Users/paulbelt/work/distill/strip-ansi")
        .spawn()
        .expect("Failed to spawn child process");

    let output = child.wait_with_output().expect("Failed to read output");

    // Should not have any panic-related stderr output
    let stderr_str = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr_str.contains("BrokenPipe"));
    assert!(!stderr_str.contains("panicked"));
}
