use insta::assert_snapshot;

fn strip_to_string(input: &[u8]) -> String {
    String::from_utf8_lossy(&strip_ansi::strip(input)).into_owned()
}

fn stream_to_string(input: &[u8]) -> String {
    let mut stream = strip_ansi::StripStream::new();
    let mut out = Vec::new();
    stream.push(input, &mut out);
    stream.finish();
    String::from_utf8_lossy(&out).into_owned()
}

#[test]
fn snapshot_cargo_test_output() {
    let input = b"\x1b[0m\x1b[1m\x1b[32m   Compiling\x1b[0m strip-ansi v0.2.0 (/home/user/project)\n\
\x1b[0m\x1b[1m\x1b[32m    Finished\x1b[0m `test` profile [unoptimized + debuginfo] target(s) in 2.34s\n\
\x1b[0m\x1b[1m\x1b[32m     Running\x1b[0m unittests src/lib.rs (target/debug/deps/strip_ansi-abc123)\n\
\n\
running 5 tests\n\
test parser::tests::ground_emit ... \x1b[32mok\x1b[0m\n\
test parser::tests::csi_skip ... \x1b[32mok\x1b[0m\n\
test parser::tests::osc_skip ... \x1b[32mok\x1b[0m\n\
test strip::tests::clean ... \x1b[32mok\x1b[0m\n\
test strip::tests::mixed ... \x1b[32mok\x1b[0m\n\
\n\
test result: \x1b[32mok\x1b[0m. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n";
    assert_snapshot!(strip_to_string(input));
}

#[test]
fn snapshot_cargo_error_output() {
    let input = b"\x1b[0m\x1b[1m\x1b[38;5;9merror[E0308]\x1b[0m\x1b[0m\x1b[1m: mismatched types\x1b[0m\n\
 \x1b[0m\x1b[1m\x1b[38;5;12m--> \x1b[0msrc/main.rs:10:5\n\
  \x1b[0m\x1b[1m\x1b[38;5;12m|\x1b[0m\n\
\x1b[0m\x1b[1m\x1b[38;5;12m10\x1b[0m \x1b[0m\x1b[1m\x1b[38;5;12m|\x1b[0m     \x1b[0m\x1b[1m\x1b[38;5;9m42\x1b[0m\n\
  \x1b[0m\x1b[1m\x1b[38;5;12m|\x1b[0m     \x1b[0m\x1b[1m\x1b[38;5;9m^^\x1b[0m \x1b[0m\x1b[1m\x1b[38;5;9mexpected `&str`, found integer\x1b[0m\n";
    assert_snapshot!(strip_to_string(input));
}

#[test]
fn snapshot_docker_build_output() {
    let input = b"#1 [internal] load build definition from Dockerfile\n\
#1 transferring dockerfile: 523B done\n\
#1 DONE 0.0s\n\
\n\
#2 [internal] load metadata for docker.io/library/rust:1.75-slim\n\
#2 DONE 1.2s\n\
\n\
#3 [1/4] FROM docker.io/library/rust:1.75-slim@sha256:abc123\n\
#3 DONE 0.0s\n\
\n\
\x1b[1m#4 [2/4] COPY Cargo.toml Cargo.lock ./\x1b[0m\n\
#4 DONE 0.1s\n\
\n\
\x1b[1m#5 [3/4] RUN cargo build --release\x1b[0m\n\
#5 0.543 \x1b[0m\x1b[1m\x1b[32m   Compiling\x1b[0m libc v0.2.150\n\
#5 2.123 \x1b[0m\x1b[1m\x1b[32m   Compiling\x1b[0m memchr v2.7.1\n\
#5 5.678 \x1b[0m\x1b[1m\x1b[32m    Finished\x1b[0m `release` profile [optimized] target(s) in 5.67s\n\
#5 DONE 6.0s\n";
    assert_snapshot!(strip_to_string(input));
}

#[test]
fn snapshot_osc8_hyperlinks() {
    // Cargo with hyperlinks enabled (CARGO_TERM_HYPERLINKS=1)
    let input = b"\x1b[0m\x1b[1m\x1b[38;5;9merror[E0433]\x1b[0m\x1b[0m\x1b[1m: failed to resolve: use of undeclared crate\x1b[0m\n\
 \x1b[0m\x1b[1m\x1b[38;5;12m--> \x1b[0m\x1b]8;;file:///home/user/src/main.rs#5\x07src/main.rs:5:5\x1b]8;;\x07\n\
  \x1b[0m\x1b[1m\x1b[38;5;12m|\x1b[0m\n\
\x1b[0m\x1b[1m\x1b[38;5;12m5\x1b[0m \x1b[0m\x1b[1m\x1b[38;5;12m|\x1b[0m use \x1b]8;;https://docs.rs/serde\x07\x1b[0m\x1b[1m\x1b[38;5;9mserde\x1b[0m\x1b]8;;\x07::Deserialize;\n";
    assert_snapshot!(strip_to_string(input));
}

#[test]
fn snapshot_window_title_sequences() {
    // tmux/screen window title setting
    let input = b"\x1b]0;user@host:~/project\x07$ cargo test\n\
\x1b]2;Running tests...\x07\
running 3 tests\n\
test a ... ok\n\
test b ... ok\n\
test c ... ok\n";
    assert_snapshot!(strip_to_string(input));
}

#[test]
fn snapshot_256_color_output() {
    let input = b"\x1b[38;5;196mRed\x1b[0m \x1b[38;5;46mGreen\x1b[0m \x1b[38;5;21mBlue\x1b[0m\n\
\x1b[48;5;226m\x1b[38;5;0mBlack on Yellow\x1b[0m\n\
\x1b[38;2;255;128;0mOrange (24-bit)\x1b[0m\n";
    assert_snapshot!(strip_to_string(input));
}

#[test]
fn snapshot_streaming_cargo_output() {
    // Same cargo output but through streaming API
    let input = b"\x1b[0m\x1b[1m\x1b[32m   Compiling\x1b[0m strip-ansi v0.2.0\n\
test result: \x1b[32mok\x1b[0m. 5 passed; 0 failed\n";
    assert_snapshot!(stream_to_string(input));
}
