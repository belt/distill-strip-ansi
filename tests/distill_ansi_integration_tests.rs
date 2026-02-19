//! Integration tests for the `distill-ansi` binary.
//!
//! Tests the CLI flags for Unicode normalization:
//! --unicode-map, --no-unicode-map, --unsafe gate.

#![cfg(feature = "distill-ansi-cli")]

use assert_cmd::Command;
use assert_cmd::cargo;

fn cmd() -> Command {
    Command::new(cargo::cargo_bin!("distill-ansi"))
}

// ── Default behavior (builtins on) ──────────────────────────────────

#[test]
fn default_normalizes_fullwidth_ascii() {
    let mut c = cmd();
    c.write_stdin("Ａdmin\n");
    c.assert().success().stdout("Admin\n");
}

#[test]
fn default_normalizes_math_bold() {
    let mut c = cmd();
    c.write_stdin("𝐇𝐞𝐥𝐥𝐨\n");
    c.assert().success().stdout("Hello\n");
}

#[test]
fn default_normalizes_circled_letters() {
    let mut c = cmd();
    c.write_stdin("Ⓣⓔⓢⓣ\n");
    c.assert().success().stdout("Test\n");
}

#[test]
fn default_normalizes_superscript_digits() {
    let mut c = cmd();
    c.write_stdin("x²\n");
    c.assert().success().stdout("x2\n");
}

#[test]
fn default_normalizes_latin_ligatures() {
    let mut c = cmd();
    c.write_stdin("ﬁle\n");
    c.assert().success().stdout("file\n");
}

#[test]
fn default_preserves_plain_ascii() {
    let mut c = cmd();
    c.write_stdin("Hello, World! 123\n");
    c.assert().success().stdout("Hello, World! 123\n");
}

#[test]
fn default_preserves_standard_unicode() {
    let mut c = cmd();
    // Standard CJK, katakana, emoji — not mapped by builtins
    c.write_stdin("漢字 カタカナ\n");
    c.assert().success().stdout("漢字 カタカナ\n");
}

// ── Fixture file end-to-end ─────────────────────────────────────────

#[test]
fn fixture_unicode_normalize() {
    let raw = std::fs::read_to_string("tests/fixtures/unicode-normalize.raw.txt").unwrap();
    let expected =
        std::fs::read_to_string("tests/fixtures/unicode-normalize.expected.txt").unwrap();

    let mut c = cmd();
    c.write_stdin(raw);
    c.assert().success().stdout(expected);
}

// ── --no-unicode-map disables builtins ──────────────────────────────

#[test]
fn no_unicode_map_ascii_normalize_disables_all_builtins() {
    let mut c = cmd();
    c.arg("--no-unicode-map").arg("@ascii-normalize");
    c.write_stdin("Ａdmin\n");
    // Fullwidth A should pass through unchanged
    c.assert().success().stdout("Ａdmin\n");
}

#[test]
fn no_unicode_map_specific_builtin() {
    let mut c = cmd();
    c.arg("--no-unicode-map").arg("superscript-subscript");
    c.write_stdin("x²\n");
    // Superscript 2 should pass through, but fullwidth still normalizes
    c.assert().success().stdout("x²\n");
}

#[test]
fn no_unicode_map_non_security_no_unsafe_needed() {
    let mut c = cmd();
    c.arg("--no-unicode-map").arg("enclosed-circled-letters");
    c.write_stdin("Ⓣⓔⓢⓣ\n");
    // Circled letters pass through, no --unsafe needed
    c.assert().success().stdout("Ⓣⓔⓢⓣ\n");
}

// ── --no-unicode-map @security (no --unsafe needed) ─────────────────

#[test]
fn no_unicode_map_security_disables_security_builtins() {
    let mut c = cmd();
    c.arg("--no-unicode-map").arg("@security");
    c.write_stdin("Ａdmin\n");
    // Fullwidth A passes through (security disabled)
    // But circled letters still normalize (non-security builtin)
    c.assert().success().stdout("Ａdmin\n");
}

#[test]
fn no_unicode_map_fullwidth_ascii_no_unsafe_needed() {
    let mut c = cmd();
    c.arg("--no-unicode-map").arg("fullwidth-ascii");
    c.write_stdin("Ａ\n");
    c.assert().success().stdout("Ａ\n");
}

#[test]
fn no_unicode_map_math_latin_bold_no_unsafe_needed() {
    let mut c = cmd();
    c.arg("--no-unicode-map").arg("math-latin-bold");
    c.write_stdin("𝐇𝐞𝐥𝐥𝐨\n");
    c.assert().success().stdout("𝐇𝐞𝐥𝐥𝐨\n");
}

#[test]
fn no_unicode_map_latin_ligatures_no_unsafe_needed() {
    let mut c = cmd();
    c.arg("--no-unicode-map").arg("latin-ligatures");
    c.write_stdin("ﬁle\n");
    c.assert().success().stdout("ﬁle\n");
}

// ── --unicode-map adds TOML files ───────────────────────────────────

#[cfg(feature = "toml-config")]
mod with_toml {
    use super::*;

    #[test]
    fn unicode_map_japanese_loads_katakana() {
        let mut c = cmd();
        c.arg("--unicode-map").arg("@japanese");
        // Halfwidth katakana ｱ should normalize to standard ア
        c.write_stdin("ｱ\n");
        c.assert().success().stdout("ア\n");
    }

    #[test]
    fn unicode_map_specific_file() {
        let mut c = cmd();
        c.arg("--unicode-map").arg("math-greek");
        // Bold alpha 𝚨 (U+1D6A8) → Α (U+0391)
        c.write_stdin("𝚨\n");
        c.assert().success().stdout("Α\n");
    }

    #[test]
    fn unicode_map_all_loads_everything() {
        let mut c = cmd();
        c.arg("--unicode-map").arg("@all");
        // Halfwidth katakana + fullwidth ASCII both normalize
        c.write_stdin("ｱＡ\n");
        c.assert().success().stdout("アA\n");
    }

    #[test]
    fn unicode_map_unknown_tag_fails() {
        let mut c = cmd();
        c.arg("--unicode-map").arg("@nonexistent");
        c.write_stdin("test\n");
        c.assert().failure().code(2);
    }

    #[test]
    fn unicode_map_and_no_unicode_map_combined() {
        let mut c = cmd();
        c.arg("--unicode-map")
            .arg("@japanese")
            .arg("--no-unicode-map")
            .arg("halfwidth-katakana");
        // Japanese tag adds katakana + cjk-compat, but we removed katakana
        // Halfwidth katakana should pass through
        c.write_stdin("ｱ\n");
        c.assert().success().stdout("ｱ\n");
    }

    #[test]
    fn unicode_map_custom_file() {
        // Create a temp TOML file with a custom mapping
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("custom.toml");
        std::fs::write(
            &path,
            r#"
[metadata]
type = "custom_test"
description = "test"
direction = "neutral"
tags = ["test"]

[[pairs]]
from = "2603"
to = "002A"
"#,
        )
        .unwrap();

        let mut c = cmd();
        c.arg("--unicode-map").arg(path.to_str().unwrap());
        // ☃ (U+2603) → * (U+002A)
        c.write_stdin("☃\n");
        c.assert().success().stdout("*\n");
    }
}

// ── Interaction with color transforms ───────────────────────────────

#[test]
fn unicode_normalize_with_color_depth() {
    let mut c = cmd();
    c.arg("--color-depth").arg("mono");
    // SGR color rewritten to mono + fullwidth normalized
    // mono strips color params but keeps the SGR structure
    c.write_stdin("\x1b[31mＡ\x1b[0m\n");
    c.assert().success().stdout("\x1b[31mA\x1b[0m\n");
}

#[test]
fn unicode_normalize_preserves_ansi_sequences() {
    let mut c = cmd();
    // With truecolor (no color change), ANSI passes through, content normalized
    c.write_stdin("\x1b[1mＡ\x1b[0m\n");
    c.assert().success().stdout("\x1b[1mA\x1b[0m\n");
}
