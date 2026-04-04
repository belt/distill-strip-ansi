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

// ── --preset ──

#[test]
fn preset_dumb_matches_default_strip_all() {
    // --preset dumb should produce identical output to no flags
    // (when piped, auto-detect also selects dumb).
    let input = "\x1b[31mred\x1b[0m \x1b]8;;https://example.com\x07link\x1b]8;;\x07\n";

    let default_output = cmd()
        .write_stdin(input)
        .output()
        .unwrap();

    let dumb_output = cmd()
        .arg("--preset")
        .arg("dumb")
        .write_stdin(input)
        .output()
        .unwrap();

    assert!(default_output.status.success());
    assert!(dumb_output.status.success());
    assert_eq!(default_output.stdout, dumb_output.stdout);
    assert_eq!(
        String::from_utf8_lossy(&dumb_output.stdout),
        "red link\n",
    );
}

#[test]
fn preset_color_preserves_sgr() {
    let input = "\x1b[31mred\x1b[0m\n";

    let output = cmd()
        .arg("--preset")
        .arg("color")
        .write_stdin(input)
        .output()
        .unwrap();

    assert!(output.status.success());
    // SGR sequences should be preserved.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\x1b[31m"), "color preset should preserve SGR");
    assert!(stdout.contains("red"));
}

#[test]
fn preset_color_strips_cursor() {
    // Color preset preserves only CsiSgr. Cursor movement is stripped.
    let input = "\x1b[5Amoved\n";

    let output = cmd()
        .arg("--preset")
        .arg("color")
        .write_stdin(input)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "moved\n");
}

#[test]
fn preset_alias_pipe_eq_dumb() {
    let input = "\x1b[1mbold\x1b[0m\n";

    let dumb = cmd()
        .arg("--preset").arg("dumb")
        .write_stdin(input)
        .output().unwrap();

    let pipe = cmd()
        .arg("--preset").arg("pipe")
        .write_stdin(input)
        .output().unwrap();

    assert_eq!(dumb.stdout, pipe.stdout);
}

#[test]
fn preset_alias_ci_eq_color() {
    let input = "\x1b[31mred\x1b[0m\n";

    let color = cmd()
        .arg("--preset").arg("color")
        .write_stdin(input)
        .output().unwrap();

    let ci = cmd()
        .arg("--preset").arg("ci")
        .write_stdin(input)
        .output().unwrap();

    assert_eq!(color.stdout, ci.stdout);
}

#[test]
fn preset_unknown_name_exits_error() {
    let output = cmd()
        .arg("--preset")
        .arg("bogus")
        .write_stdin("test")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown preset"));
    assert!(stderr.contains("bogus"));
}

#[test]
fn preset_case_insensitive() {
    let input = "\x1b[31mred\x1b[0m\n";

    let lower = cmd()
        .arg("--preset").arg("color")
        .write_stdin(input)
        .output().unwrap();

    let upper = cmd()
        .arg("--preset").arg("COLOR")
        .write_stdin(input)
        .output().unwrap();

    assert!(upper.status.success());
    assert_eq!(lower.stdout, upper.stdout);
}

#[test]
fn preset_full_preserves_everything() {
    let input = "\x1b[31mred\x1b[0m \x1b]8;;https://x.com\x07link\x1b]8;;\x07\n";

    let output = cmd()
        .arg("--preset")
        .arg("full")
        .arg("--unsafe")
        .write_stdin(input)
        .output()
        .unwrap();

    assert!(output.status.success());
    // Full preset should pass input through unchanged.
    assert_eq!(String::from_utf8_lossy(&output.stdout), input);
}

#[test]
fn preset_with_no_strip_overlay() {
    // Start from color, verify --no-strip-osc adds OSC preservation.
    let input = "\x1b[31mred\x1b[0m \x1b]0;title\x07\n";

    let without_overlay = cmd()
        .arg("--preset").arg("color")
        .write_stdin(input)
        .output()
        .unwrap();

    let with_overlay = cmd()
        .arg("--preset").arg("color")
        .arg("--no-strip-osc")
        .write_stdin(input)
        .output()
        .unwrap();

    assert!(without_overlay.status.success());
    assert!(with_overlay.status.success());

    let without_str = String::from_utf8_lossy(&without_overlay.stdout);
    let with_str = String::from_utf8_lossy(&with_overlay.stdout);

    // Without overlay: OSC stripped.
    assert!(!without_str.contains("\x1b]0;"));
    // With overlay: OSC preserved.
    assert!(with_str.contains("\x1b]0;"));
}

// ── --unsafe gate ──

#[test]
fn preset_xterm_without_unsafe_exits_error() {
    let output = cmd()
        .arg("--preset")
        .arg("xterm")
        .write_stdin("test")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--preset xterm preserves dangerous sequences"),
        "expected unsafe gate error, got: {stderr}"
    );
    assert!(stderr.contains("--unsafe"));
}

#[test]
fn preset_full_without_unsafe_exits_error() {
    let output = cmd()
        .arg("--preset")
        .arg("full")
        .write_stdin("test")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--preset full preserves dangerous sequences"),
        "expected unsafe gate error, got: {stderr}"
    );
    assert!(stderr.contains("--unsafe"));
}

#[test]
fn preset_xterm_with_unsafe_accepts() {
    let input = "\x1b[31mred\x1b[0m \x1b]8;;https://x.com\x07link\x1b]8;;\x07\n";

    let output = cmd()
        .arg("--preset")
        .arg("xterm")
        .arg("--unsafe")
        .write_stdin(input)
        .output()
        .unwrap();

    assert!(output.status.success());
    // Xterm preserves CSI + OSC + Fe.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\x1b[31m"), "xterm --unsafe should preserve SGR");
    assert!(stdout.contains("\x1b]8;"), "xterm --unsafe should preserve OSC hyperlinks");
}

#[test]
fn unsafe_with_safe_preset_is_noop() {
    // --unsafe with a safe preset (color) should be silently accepted.
    let input = "\x1b[31mred\x1b[0m\n";

    let output = cmd()
        .arg("--preset")
        .arg("color")
        .arg("--unsafe")
        .write_stdin(input)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\x1b[31m"), "color --unsafe should still preserve SGR");
}

#[test]
fn unsafe_without_preset_is_ignored() {
    // --unsafe without --preset should be silently ignored.
    let input = "\x1b[31mred\x1b[0m\n";

    let output = cmd()
        .arg("--unsafe")
        .write_stdin(input)
        .output()
        .unwrap();

    assert!(output.status.success());
}


// ── --check-threats ──

#[test]
fn check_threats_detects_csi_21t_exit_77() {
    // CSI 21t = title report echoback vector
    let mut cmd = cmd();
    cmd.arg("--check-threats")
        .write_stdin("hello\x1b[21tworld\n");

    let output = cmd.output().unwrap();
    assert_eq!(output.status.code(), Some(77));
    assert!(output.stdout.is_empty(), "fail mode should produce no stdout");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("csi_21t"), "should report csi_21t threat");
}

#[test]
fn check_threats_detects_csi_6n_exit_77() {
    // CSI 6n = cursor position report echoback vector
    let mut cmd = cmd();
    cmd.arg("--check-threats")
        .write_stdin("hello\x1b[6nworld\n");

    let output = cmd.output().unwrap();
    assert_eq!(output.status.code(), Some(77));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("csi_6n"), "should report csi_6n threat");
}

#[test]
fn check_threats_detects_osc_50_exit_77() {
    // OSC 50 = font query echoback vector
    let mut cmd = cmd();
    cmd.arg("--check-threats")
        .write_stdin("hello\x1b]50;?\x07world\n");

    let output = cmd.output().unwrap();
    assert_eq!(output.status.code(), Some(77));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("osc_50"), "should report osc_50 threat");
}

#[test]
fn check_threats_detects_osc_52_clipboard_exit_77() {
    // OSC 52 = clipboard access
    let mut cmd = cmd();
    cmd.arg("--check-threats")
        .write_stdin("hello\x1b]52;c;SGVsbG8=\x07world\n");

    let output = cmd.output().unwrap();
    assert_eq!(output.status.code(), Some(77));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("osc_clipboard"),
        "should report osc_clipboard threat"
    );
}

#[test]
fn check_threats_detects_dcs_decrqss_exit_77() {
    // DCS $q = DECRQSS echoback vector
    let mut cmd = cmd();
    cmd.arg("--check-threats")
        .write_stdin("hello\x1bP$q\"p\x1b\\world\n");

    let output = cmd.output().unwrap();
    assert_eq!(output.status.code(), Some(77));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("dcs_decrqss"),
        "should report dcs_decrqss threat"
    );
}

#[test]
fn check_threats_clean_input_exit_0() {
    let mut cmd = cmd();
    cmd.arg("--check-threats")
        .write_stdin("\x1b[31mhello\x1b[0m world\n");

    let output = cmd.output().unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty(), "fail mode should produce no stdout");
}

#[test]
fn check_threats_multiple_threats_reports_all() {
    // Multiple threats in one input — should report all and exit 77
    let mut cmd = cmd();
    cmd.arg("--check-threats")
        .write_stdin("\x1b[21tfoo\x1b[6nbar\n");

    let output = cmd.output().unwrap();
    assert_eq!(output.status.code(), Some(77));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("csi_21t"), "should report csi_21t");
    assert!(stderr.contains("csi_6n"), "should report csi_6n");
}

// ── --check-threats --on-threat=strip ──

#[test]
fn check_threats_strip_mode_filters_and_reports() {
    // Strip mode: clean output to stdout, threats to stderr, exit 0
    let mut cmd = cmd();
    cmd.arg("--check-threats")
        .arg("--on-threat=strip")
        .write_stdin("\x1b[31mhello\x1b[0m \x1b[21tworld\n");

    let output = cmd.output().unwrap();
    assert!(output.status.success(), "strip mode should exit 0");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // SGR should be preserved (sanitize preserves SGR), threat stripped
    assert!(stdout.contains("hello"), "should contain text");
    assert!(stdout.contains("world"), "should contain text after threat");
    // The CSI 21t sequence itself should be stripped
    assert!(!stdout.contains("\x1b[21t"), "threat sequence should be stripped");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("csi_21t"), "should report csi_21t on stderr");
}

#[test]
fn check_threats_strip_mode_clean_input() {
    let mut cmd = cmd();
    cmd.arg("--check-threats")
        .arg("--on-threat=strip")
        .write_stdin("\x1b[31mhello\x1b[0m\n");

    let output = cmd.output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hello"));
    // No threats → no stderr output
    assert!(output.stderr.is_empty() || String::from_utf8_lossy(&output.stderr).trim().is_empty());
}

// ── --on-threat without --check-threats → error ──

#[test]
fn on_threat_without_check_threats_is_error() {
    let mut cmd = cmd();
    cmd.arg("--on-threat=strip")
        .write_stdin("test");

    let output = cmd.output().unwrap();
    assert_eq!(output.status.code(), Some(2), "clap should reject --on-threat without --check-threats");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error:"), "should show clap error");
}

// ── --check-threats conflicts with --check ──

#[test]
fn check_threats_conflicts_with_check() {
    let mut cmd = cmd();
    cmd.arg("--check")
        .arg("--check-threats")
        .write_stdin("test");

    let output = cmd.output().unwrap();
    assert_eq!(output.status.code(), Some(2), "clap should reject conflicting flags");
}

// ── Structured threat output format (Task 10.5) ──

#[test]
fn structured_threat_output_csi_21t_format() {
    // CSI 21t at start of input: line=1, pos=1, offset=0
    let mut cmd = cmd();
    cmd.arg("--check-threats")
        .write_stdin("\x1b[21t\n");

    let output = cmd.output().unwrap();
    assert_eq!(output.status.code(), Some(77));
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Verify structured format fields
    assert!(stderr.contains("[strip-ansi:threat]"), "should have prefix");
    assert!(stderr.contains("type=csi_21t"), "should have type");
    assert!(stderr.contains("line=1"), "should have line=1");
    assert!(stderr.contains("pos=1"), "should have pos=1");
    assert!(stderr.contains("offset=0"), "should have offset=0");
    assert!(stderr.contains("len=5"), "CSI 21t is 5 bytes: ESC [ 2 1 t");
    assert!(stderr.contains("cve=CVE-2003-0063"), "should have CVE");
    assert!(
        stderr.contains("ref=https://nvd.nist.gov/vuln/detail/CVE-2003-0063"),
        "should have ref URI"
    );
}

#[test]
fn structured_threat_output_osc_50_format() {
    // OSC 50 at start: line=1, pos=1, offset=0
    let mut cmd = cmd();
    cmd.arg("--check-threats")
        .write_stdin("\x1b]50;?\x07\n");

    let output = cmd.output().unwrap();
    assert_eq!(output.status.code(), Some(77));
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.contains("type=osc_50"), "should have type");
    assert!(stderr.contains("line=1"), "should have line=1");
    assert!(stderr.contains("pos=1"), "should have pos=1");
    assert!(stderr.contains("offset=0"), "should have offset=0");
    assert!(stderr.contains("cve=CVE-2022-45063"), "should have CVE");
    assert!(
        stderr.contains("ref=https://nvd.nist.gov/vuln/detail/CVE-2022-45063"),
        "should have ref URI"
    );
}

#[test]
fn structured_threat_output_dcs_decrqss_format() {
    // DCS $q at start: line=1, pos=1, offset=0
    let mut cmd = cmd();
    cmd.arg("--check-threats")
        .write_stdin("\x1bP$q\"p\x1b\\\n");

    let output = cmd.output().unwrap();
    assert_eq!(output.status.code(), Some(77));
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.contains("type=dcs_decrqss"), "should have type");
    assert!(stderr.contains("line=1"), "should have line=1");
    assert!(stderr.contains("pos=1"), "should have pos=1");
    assert!(stderr.contains("offset=0"), "should have offset=0");
    assert!(stderr.contains("cve=CVE-2008-2383"), "should have CVE");
    assert!(
        stderr.contains("ref=https://nvd.nist.gov/vuln/detail/CVE-2008-2383"),
        "should have ref URI"
    );
}

#[test]
fn structured_threat_output_osc_clipboard_no_cve() {
    // OSC 52 (clipboard) has no CVE
    let mut cmd = cmd();
    cmd.arg("--check-threats")
        .write_stdin("\x1b]52;c;SGVsbG8=\x07\n");

    let output = cmd.output().unwrap();
    assert_eq!(output.status.code(), Some(77));
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.contains("type=osc_clipboard"), "should have type");
    assert!(!stderr.contains("cve="), "osc_clipboard should have no CVE");
    assert!(!stderr.contains("ref="), "osc_clipboard should have no ref");
}

#[test]
fn structured_threat_output_csi_6n_no_cve() {
    // CSI 6n has no CVE
    let mut cmd = cmd();
    cmd.arg("--check-threats")
        .write_stdin("\x1b[6n\n");

    let output = cmd.output().unwrap();
    assert_eq!(output.status.code(), Some(77));
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.contains("type=csi_6n"), "should have type");
    assert!(!stderr.contains("cve="), "csi_6n should have no CVE");
    assert!(!stderr.contains("ref="), "csi_6n should have no ref");
}

#[test]
fn structured_threat_output_line_pos_tracking() {
    // Threat on line 2, after some text on line 1
    // Line 1: "hello\n" (6 bytes: h=0, e=1, l=2, l=3, o=4, \n=5)
    // Line 2: "ab" then CSI 21t at pos=3, offset=8
    let mut cmd = cmd();
    cmd.arg("--check-threats")
        .write_stdin("hello\nab\x1b[21t\n");

    let output = cmd.output().unwrap();
    assert_eq!(output.status.code(), Some(77));
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.contains("type=csi_21t"), "should have type");
    assert!(stderr.contains("line=2"), "should be on line 2");
    assert!(stderr.contains("pos=3"), "should be at pos 3 (after 'ab')");
    assert!(stderr.contains("offset=8"), "should be at byte offset 8");
}

#[test]
fn structured_threat_output_multiple_lines() {
    // Two threats on different lines
    // Line 1: CSI 21t (offset=0, line=1, pos=1)
    // Line 2: CSI 6n (offset after line 1)
    let mut cmd = cmd();
    cmd.arg("--check-threats")
        .write_stdin("\x1b[21t\n\x1b[6n\n");

    let output = cmd.output().unwrap();
    assert_eq!(output.status.code(), Some(77));
    let stderr = String::from_utf8_lossy(&output.stderr);
    let lines: Vec<&str> = stderr.trim().lines().collect();

    assert_eq!(lines.len(), 2, "should have 2 threat lines");
    assert!(lines[0].contains("line=1"), "first threat on line 1");
    assert!(lines[1].contains("line=2"), "second threat on line 2");
}

// ── --no-threat-report (Task 10.6) ──

#[test]
fn no_threat_report_suppresses_stderr_fail_mode() {
    let mut cmd = cmd();
    cmd.arg("--check-threats")
        .arg("--no-threat-report")
        .write_stdin("\x1b[21tworld\n");

    let output = cmd.output().unwrap();
    // Exit code preserved: still 77
    assert_eq!(output.status.code(), Some(77));
    // stderr should be empty
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.trim().is_empty(),
        "stderr should be empty with --no-threat-report, got: {stderr}"
    );
}

#[test]
fn no_threat_report_suppresses_stderr_strip_mode() {
    let mut cmd = cmd();
    cmd.arg("--check-threats")
        .arg("--on-threat=strip")
        .arg("--no-threat-report")
        .write_stdin("\x1b[31mhello\x1b[0m \x1b[21tworld\n");

    let output = cmd.output().unwrap();
    // Exit code preserved: 0 in strip mode
    assert!(output.status.success(), "strip mode should exit 0");
    // stdout should still have clean output
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hello"), "should contain text");
    assert!(stdout.contains("world"), "should contain text after threat");
    // stderr should be empty
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.trim().is_empty(),
        "stderr should be empty with --no-threat-report, got: {stderr}"
    );
}

#[test]
fn no_threat_report_clean_input_still_exit_0() {
    let mut cmd = cmd();
    cmd.arg("--check-threats")
        .arg("--no-threat-report")
        .write_stdin("\x1b[31mhello\x1b[0m\n");

    let output = cmd.output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

#[test]
fn no_threat_report_requires_check_threats() {
    let mut cmd = cmd();
    cmd.arg("--no-threat-report")
        .write_stdin("test");

    let output = cmd.output().unwrap();
    assert_eq!(
        output.status.code(),
        Some(2),
        "clap should reject --no-threat-report without --check-threats"
    );
}

// ── --threat-db integration (Task 11.5.13) ──

#[test]
fn threat_db_loads_file_and_uses_for_reporting() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("threat-db.toml");
    std::fs::write(
        &db_path,
        r#"
[[threats]]
type = "custom_osc_999"
cve = "CVE-2024-99999"
description = "Custom OSC 999 threat"
ref = "https://example.com/advisory"

[threats.match]
kind = "Osc"
osc_number = 999
"#,
    )
    .unwrap();

    // Feed an OSC 999 sequence: ESC ] 9 9 9 ; data BEL
    let mut cmd = cmd();
    cmd.arg("--check-threats")
        .arg("--threat-db")
        .arg(db_path.to_str().unwrap())
        .write_stdin("hello\x1b]999;data\x07world\n");

    let output = cmd.output().unwrap();
    assert_eq!(output.status.code(), Some(77), "should exit 77 for custom threat");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("custom_osc_999"),
        "should report custom threat type, got: {stderr}"
    );
    assert!(
        stderr.contains("CVE-2024-99999"),
        "should report custom CVE, got: {stderr}"
    );
    assert!(
        stderr.contains("ref=https://example.com/advisory"),
        "should report custom ref URI, got: {stderr}"
    );
}

#[test]
fn threat_db_builtins_still_detected() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("threat-db.toml");
    // Empty external DB — builtins should still work.
    std::fs::write(&db_path, "").unwrap();

    let mut cmd = cmd();
    cmd.arg("--check-threats")
        .arg("--threat-db")
        .arg(db_path.to_str().unwrap())
        .write_stdin("\x1b[21t\n");

    let output = cmd.output().unwrap();
    assert_eq!(output.status.code(), Some(77));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("csi_21t"),
        "builtins should still be detected with --threat-db, got: {stderr}"
    );
}

#[test]
fn threat_db_nonexistent_file_exits_error() {
    let mut cmd = cmd();
    cmd.arg("--check-threats")
        .arg("--threat-db")
        .arg("/nonexistent/threat-db.toml")
        .write_stdin("test");

    let output = cmd.output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--threat-db"),
        "should mention --threat-db in error, got: {stderr}"
    );
}

#[test]
fn threat_db_duplicate_type_warns_and_uses_builtin() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("threat-db.toml");
    std::fs::write(
        &db_path,
        r#"
[[threats]]
type = "csi_21t"
description = "Trying to override builtin"

[threats.match]
kind = "CsiQuery"
first_param = 21
"#,
    )
    .unwrap();

    let mut cmd = cmd();
    cmd.arg("--check-threats")
        .arg("--threat-db")
        .arg(db_path.to_str().unwrap())
        .write_stdin("\x1b[21t\n");

    let output = cmd.output().unwrap();
    assert_eq!(output.status.code(), Some(77));
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should warn about duplicate.
    assert!(
        stderr.contains("rejecting duplicate"),
        "should warn about duplicate type, got: {stderr}"
    );
    // Should still detect the threat via builtin.
    assert!(
        stderr.contains("csi_21t"),
        "should still detect via builtin, got: {stderr}"
    );
}
