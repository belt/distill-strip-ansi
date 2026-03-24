use assert_cmd::cargo;
use assert_cmd::Command;
use std::fs;

const FIXTURES_DIR: &str = "tests/fixtures";

fn get_fixture_path(name: &str) -> String {
    format!("{}/{}", FIXTURES_DIR, name)
}

fn read_fixture(name: &str) -> String {
    let path = get_fixture_path(name);
    fs::read_to_string(&path).unwrap_or_else(|_| panic!("Failed to read fixture: {}", name))
}

fn cmd() -> Command {
    Command::new(cargo::cargo_bin!("strip-ansi"))
}

#[test]
fn fixture_ansi_sgr() {
    let mut cmd = cmd();
    let raw = read_fixture("ansi-sgr.raw.txt");
    let expected = read_fixture("ansi-sgr.expected.txt");

    cmd.write_stdin(raw);

    let output = cmd.output().unwrap();

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout_str, expected,
        "SGR fixture: stripped output should match expected"
    );
    assert!(output.status.success());
}

#[test]
fn fixture_ansi_mixed() {
    let mut cmd = cmd();
    let raw = read_fixture("ansi-mixed.raw.txt");
    let expected = read_fixture("ansi-mixed.expected.txt");

    cmd.write_stdin(raw);

    let output = cmd.output().unwrap();

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout_str, expected,
        "Mixed fixture: stripped output should match expected"
    );
    assert!(output.status.success());
}

#[test]
fn fixture_plain_text() {
    let mut cmd = cmd();
    let raw = read_fixture("plain-text.raw.txt");
    let expected = read_fixture("plain-text.expected.txt");

    cmd.write_stdin(raw);

    let output = cmd.output().unwrap();

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout_str, expected,
        "Plain text fixture: should pass through unchanged"
    );
    assert!(output.status.success());
}

#[test]
fn help_flag() {
    let mut cmd = cmd();

    cmd.arg("--help");

    let output = cmd.output().unwrap();

    assert!(output.status.success());
    let stdout_str = String::from_utf8_lossy(&output.stdout);
    assert!(stdout_str.contains("strip-ansi"));
}

#[test]
fn version_flag() {
    let mut cmd = cmd();

    cmd.arg("--version");

    let output = cmd.output().unwrap();

    assert!(output.status.success());
    let stdout_str = String::from_utf8_lossy(&output.stdout);
    assert!(stdout_str.contains("strip-ansi"));
}

#[test]
fn unknown_flag() {
    let mut cmd = cmd();

    cmd.arg("--bogus");

    let output = cmd.output().unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr_str = String::from_utf8_lossy(&output.stderr);
    assert!(stderr_str.contains("error:"));
}

#[test]
fn check_with_ansi() {
    let mut cmd = cmd();
    cmd.arg("--check").write_stdin("\x1b[31mred\x1b[0m");

    let output = cmd.output().unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr_str = String::from_utf8_lossy(&output.stderr);
    assert!(stderr_str.contains("ANSI escape sequences detected"));
}

#[test]
fn check_with_clean() {
    let mut cmd = cmd();
    cmd.arg("--check").write_stdin("plain text");

    let output = cmd.output().unwrap();

    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn sigpipe_handling() {
    use std::process::Command as StdCommand;

    let raw = read_fixture("ansi-sgr.raw.txt");
    let bin = cargo::cargo_bin!("strip-ansi");
    let bin_str = bin.display();

    let child = StdCommand::new("sh")
        .arg("-c")
        .arg(format!("echo '$1' | '{bin_str}' | head -n 1"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .arg("dummy")
        .arg(&raw)
        .spawn()
        .expect("Failed to spawn child process");

    let output = child.wait_with_output().expect("Failed to read output");

    assert_eq!(output.status.code(), Some(0));
    let stderr_str = String::from_utf8_lossy(&output.stderr);
    assert!(stderr_str.is_empty() || !stderr_str.contains("BrokenPipe"));
}

// ── --head / -n ──

#[test]
fn head_limits_output_lines() {
    let mut cmd = cmd();
    cmd.arg("--head").arg("2").write_stdin(
        "\x1b[31mline1\x1b[0m\n\x1b[32mline2\x1b[0m\n\x1b[33mline3\x1b[0m\n",
    );

    let output = cmd.output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "line1\nline2\n");
}

#[test]
fn head_short_flag() {
    let mut cmd = cmd();
    cmd.arg("-n").arg("1").write_stdin("aaa\nbbb\nccc\n");

    let output = cmd.output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "aaa\n");
}

#[test]
fn head_more_than_input() {
    let mut cmd = cmd();
    cmd.arg("--head").arg("100").write_stdin("only\n");

    let output = cmd.output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "only\n");
}

#[test]
fn head_zero_produces_empty() {
    let mut cmd = cmd();
    cmd.arg("--head").arg("0").write_stdin("stuff\n");

    let output = cmd.output().unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
}

// ── --count / -c ──

#[test]
fn count_reports_stripped_bytes() {
    let mut cmd = cmd();
    // \x1b[31m = 5 bytes, \x1b[0m = 4 bytes → 9 stripped
    cmd.arg("--count").write_stdin("\x1b[31mhello\x1b[0m\n");

    let output = cmd.output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hello\n");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(stderr.trim(), "9");
}

#[test]
fn count_zero_on_clean_input() {
    let mut cmd = cmd();
    cmd.arg("-c").write_stdin("clean\n");

    let output = cmd.output().unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(stderr.trim(), "0");
}

// ── --output / -o ──

#[test]
fn output_to_file() {
    let dir = tempfile::tempdir().unwrap();
    let out_path = dir.path().join("out.txt");

    let mut cmd = cmd();
    cmd.arg("-o")
        .arg(out_path.to_str().unwrap())
        .write_stdin("\x1b[1mbold\x1b[0m\n");

    let output = cmd.output().unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty(), "stdout should be empty with -o");
    assert_eq!(fs::read_to_string(&out_path).unwrap(), "bold\n");
}

// ── --max-size ──

#[test]
fn max_size_caps_input() {
    let mut cmd = cmd();
    // 10 bytes of input, cap at 5 → only first 5 bytes processed
    cmd.arg("--max-size")
        .arg("5")
        .write_stdin("abcdefghij");

    let output = cmd.output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "abcde");
}

#[test]
fn max_size_with_check() {
    let mut cmd = cmd();
    // ANSI at byte 6+, cap at 5 → check sees only clean bytes
    cmd.arg("--check")
        .arg("--max-size")
        .arg("5")
        .write_stdin("hello\x1b[31mred\x1b[0m");

    let output = cmd.output().unwrap();
    assert!(output.status.success());
}

// ── combined options ──

#[test]
fn head_and_count_combined() {
    let mut cmd = cmd();
    cmd.arg("--head")
        .arg("1")
        .arg("--count")
        .write_stdin("\x1b[31mfirst\x1b[0m\n\x1b[32msecond\x1b[0m\n");

    let output = cmd.output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "first\n");
    // Count is reported even with --head
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.is_empty());
}

#[test]
fn output_and_head_combined() {
    let dir = tempfile::tempdir().unwrap();
    let out_path = dir.path().join("head_out.txt");

    let mut cmd = cmd();
    cmd.arg("-o")
        .arg(out_path.to_str().unwrap())
        .arg("--head")
        .arg("1")
        .write_stdin("line1\nline2\nline3\n");

    let output = cmd.output().unwrap();
    assert!(output.status.success());
    assert_eq!(fs::read_to_string(&out_path).unwrap(), "line1\n");
}
