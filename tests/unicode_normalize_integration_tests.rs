#![cfg(feature = "unicode-normalize")]

use strip_ansi::unicode_map::UnicodeMap;

/// Apply unicode normalization to a UTF-8 string using the given map.
/// This simulates what transform_line() will do for content bytes.
fn normalize_str(input: &str, map: &UnicodeMap) -> String {
    let mut output = String::with_capacity(input.len());
    let mut char_buf = Vec::new();
    for c in input.chars() {
        char_buf.clear();
        if map.lookup_into(c, &mut char_buf) {
            for &tc in &char_buf {
                output.push(tc);
            }
        } else {
            output.push(c);
        }
    }
    output
}

// ── End-to-end with builtins ────────────────────────────────────────

#[test]
fn normalize_fullwidth_admin() {
    let map = UnicodeMap::builtin();
    assert_eq!(normalize_str("Ａdmin", &map), "Admin");
}

#[test]
fn normalize_fullwidth_ip_address() {
    let map = UnicodeMap::builtin();
    assert_eq!(normalize_str("１９２.１６８.１.１", &map), "192.168.1.1");
}

#[test]
fn normalize_math_bold_hello() {
    let map = UnicodeMap::builtin();
    assert_eq!(normalize_str("𝐇𝐞𝐥𝐥𝐨", &map), "Hello");
}

#[test]
fn normalize_circled_test() {
    let map = UnicodeMap::builtin();
    assert_eq!(normalize_str("Ⓣⓔⓢⓣ", &map), "Test");
}

#[test]
fn normalize_superscript_squared() {
    let map = UnicodeMap::builtin();
    assert_eq!(normalize_str("x² + y²", &map), "x2 + y2");
}

#[test]
fn normalize_ligature_file() {
    let map = UnicodeMap::builtin();
    assert_eq!(normalize_str("ﬁle not found", &map), "file not found");
}

#[test]
fn normalize_ligature_ffl() {
    let map = UnicodeMap::builtin();
    assert_eq!(normalize_str("waﬄe", &map), "waffle");
}

#[test]
fn normalize_fullwidth_yen() {
    let map = UnicodeMap::builtin();
    assert_eq!(normalize_str("￥１００", &map), "¥100");
}

#[test]
fn normalize_plain_ascii_unchanged() {
    let map = UnicodeMap::builtin();
    let input = "Hello, World! 123 foo@bar.com";
    assert_eq!(normalize_str(input, &map), input);
}

#[test]
fn normalize_mixed_content() {
    let map = UnicodeMap::builtin();
    // Mix of fullwidth, math bold, circled, ligature, and plain ASCII
    assert_eq!(normalize_str("Ａ𝐁Ⓒﬁ5", &map), "ABCfi5");
}

#[test]
fn normalize_empty_string() {
    let map = UnicodeMap::builtin();
    assert_eq!(normalize_str("", &map), "");
}

#[test]
fn normalize_preserves_newlines() {
    let map = UnicodeMap::builtin();
    assert_eq!(normalize_str("Ａ\n𝐁\n", &map), "A\nB\n");
}

#[test]
fn normalize_preserves_non_mapped_unicode() {
    let map = UnicodeMap::builtin();
    // CJK ideographs, standard katakana, emoji — none should be mapped
    let input = "漢字 カタカナ 🎉";
    assert_eq!(normalize_str(input, &map), input);
}

// ── End-to-end with TOML files ──────────────────────────────────────

#[cfg(feature = "toml-config")]
mod with_toml {
    use super::*;
    use strip_ansi::unicode_map::load_str;

    #[test]
    fn normalize_halfwidth_katakana() {
        let text = std::fs::read_to_string("etc/unicode-mappings/halfwidth-katakana.toml").unwrap();
        let set = load_str(&text, "test".into()).unwrap();
        let mut map = UnicodeMap::builtin();
        map.merge_set(set).unwrap();
        // ｱｲｳ → アイウ
        assert_eq!(normalize_str("ｱｲｳ", &map), "アイウ");
    }

    #[test]
    fn normalize_math_italic() {
        let text = std::fs::read_to_string("etc/unicode-mappings/math-latin.toml").unwrap();
        let set = load_str(&text, "test".into()).unwrap();
        let mut map = UnicodeMap::builtin();
        map.merge_set(set).unwrap();
        // 𝐴𝐵𝐶 (italic) → ABC
        assert_eq!(normalize_str("𝐴𝐵𝐶", &map), "ABC");
    }

    #[test]
    fn normalize_enclosed_numbers() {
        let text =
            std::fs::read_to_string("etc/unicode-mappings/enclosed-alphanumerics.toml").unwrap();
        let set = load_str(&text, "test".into()).unwrap();
        let mut map = UnicodeMap::builtin();
        map.merge_set(set).unwrap();
        // ①②③ → 123
        assert_eq!(normalize_str("①②③", &map), "123");
    }

    #[test]
    fn normalize_enclosed_twenty() {
        let text =
            std::fs::read_to_string("etc/unicode-mappings/enclosed-alphanumerics.toml").unwrap();
        let set = load_str(&text, "test".into()).unwrap();
        let mut map = UnicodeMap::builtin();
        map.merge_set(set).unwrap();
        // ⑳ → 20 (multi-char)
        assert_eq!(normalize_str("step ⑳", &map), "step 20");
    }

    #[test]
    fn normalize_cjk_compat_kiro() {
        let text = std::fs::read_to_string("etc/unicode-mappings/cjk-compatibility.toml").unwrap();
        let set = load_str(&text, "test".into()).unwrap();
        let mut map = UnicodeMap::builtin();
        map.merge_set(set).unwrap();
        // ㌔ → キロ
        assert_eq!(normalize_str("5㌔", &map), "5キロ");
    }

    #[test]
    fn normalize_arabic_beh() {
        let text =
            std::fs::read_to_string("etc/unicode-mappings/arabic-presentation-forms.toml").unwrap();
        let set = load_str(&text, "test".into()).unwrap();
        let mut map = UnicodeMap::builtin();
        map.merge_set(set).unwrap();
        // FE8F (beh isolated) → 0628 (beh)
        assert_eq!(normalize_str("\u{FE8F}", &map), "\u{0628}");
    }

    #[test]
    fn fixture_file_end_to_end() {
        // Load all default builtins — matches what distill-ansi will do
        let map = UnicodeMap::builtin();

        let raw = std::fs::read_to_string("tests/fixtures/unicode-normalize.raw.txt").unwrap();
        let expected =
            std::fs::read_to_string("tests/fixtures/unicode-normalize.expected.txt").unwrap();

        let result = normalize_str(&raw, &map);
        assert_eq!(result, expected);
    }
}
