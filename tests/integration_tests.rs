use assert_cmd::Command;
use std::fs;

const FIXTURES_DIR: &str = "tests/fixtures";

fn get_fixture_path(name: &str) -> String {
    format!("{}/{}", FIXTURES_DIR, name)
}

fn read_fixture(name: &str) -> String {
    let path = get_fixture_path(name);
    fs::read_to_string(&path).expect(&format!("Failed to read fixture: {}", name))
}

#[test]
fn fixture_ansi_sgr() {
    let mut cmd = Command::cargo_bin("strip-ansi").unwrap();
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
    let mut cmd = Command::cargo_bin("strip-ansi").unwrap();
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
    let mut cmd = Command::cargo_bin("strip-ansi").unwrap();
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
    let mut cmd = Command::cargo_bin("strip-ansi").unwrap();

    cmd.arg("--help");

    let output = cmd.output().unwrap();

    assert!(output.status.success());
    let stdout_str = String::from_utf8_lossy(&output.stdout);
    assert!(stdout_str.contains("strip-ansi"));
}

#[test]
fn version_flag() {
    let mut cmd = Command::cargo_bin("strip-ansi").unwrap();

    cmd.arg("--version");

    let output = cmd.output().unwrap();

    assert!(output.status.success());
    let stdout_str = String::from_utf8_lossy(&output.stdout);
    assert!(stdout_str.contains("strip-ansi"));
}

#[test]
fn unknown_flag() {
    let mut cmd = Command::cargo_bin("strip-ansi").unwrap();

    cmd.arg("--bogus");

    let output = cmd.output().unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr_str = String::from_utf8_lossy(&output.stderr);
    assert!(stderr_str.contains("error:"));
}

#[test]
fn check_with_ansi() {
    let mut cmd = Command::cargo_bin("strip-ansi").unwrap();
    cmd.arg("--check").write_stdin("\x1b[31mred\x1b[0m");

    let output = cmd.output().unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr_str = String::from_utf8_lossy(&output.stderr);
    assert!(stderr_str.contains("ANSI escape sequences detected"));
}

#[test]
fn check_with_clean() {
    let mut cmd = Command::cargo_bin("strip-ansi").unwrap();
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

    let child = StdCommand::new("sh")
        .arg("-c")
        .arg("echo '$1' | cargo run --quiet --bin strip-ansi -- | head -n 1")
        .env("RUSTUP_HOME", "/Users/paulbelt/.rustup")
        .env("CARGO_HOME", "/Users/paulbelt/.cargo")
        .env("CARGO_TARGET_DIR", "/Users/paulbelt/work/distill/target")
        .current_dir("/Users/paulbelt/work/distill/strip-ansi")
        .arg("dummy")
        .arg(&raw)
        .spawn()
        .expect("Failed to spawn child process");

    let output = child.wait_with_output().expect("Failed to read output");

    assert_eq!(output.status.code(), Some(0));
    let stderr_str = String::from_utf8_lossy(&output.stderr);
    assert!(stderr_str.is_empty() || !stderr_str.contains("BrokenPipe"));
}
