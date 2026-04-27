#![cfg(all(feature = "unicode-normalize", feature = "toml-config"))]

use strip_ansi::unicode_map::{load_str, Direction, UnicodeMap};

// ── Shipped file loading ────────────────────────────────────────────

#[test]
fn shipped_math_latin_parses() {
    let text = std::fs::read_to_string("etc/unicode-mappings/math-latin.toml").unwrap();
    let set = load_str(&text, "math-latin.toml".into()).unwrap();
    assert_eq!(set.type_name, "math_latin");
    assert_eq!(set.direction, Direction::Narrowing);
    assert!(!set.ranges.is_empty(), "should have ranges");
    assert!(!set.pairs.is_empty(), "should have Letterlike Symbols pairs");
}

#[test]
fn shipped_math_latin_italic_a_maps() {
    let text = std::fs::read_to_string("etc/unicode-mappings/math-latin.toml").unwrap();
    let set = load_str(&text, "math-latin.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // Italic A: U+1D434 → A
    assert_eq!(map.lookup_char('\u{1D434}'), Some('A'));
}

#[test]
fn shipped_math_latin_script_b_via_letterlike() {
    let text = std::fs::read_to_string("etc/unicode-mappings/math-latin.toml").unwrap();
    let set = load_str(&text, "math-latin.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // ℬ (U+212C, Letterlike Symbols) → B
    assert_eq!(map.lookup_char('\u{212C}'), Some('B'));
}

#[test]
fn shipped_math_latin_double_struck_c() {
    let text = std::fs::read_to_string("etc/unicode-mappings/math-latin.toml").unwrap();
    let set = load_str(&text, "math-latin.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // ℂ (U+2102, Letterlike Symbols) → C
    assert_eq!(map.lookup_char('\u{2102}'), Some('C'));
}

#[test]
fn shipped_math_latin_monospace_a() {
    let text = std::fs::read_to_string("etc/unicode-mappings/math-latin.toml").unwrap();
    let set = load_str(&text, "math-latin.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // Monospace A: U+1D670 → A
    assert_eq!(map.lookup_char('\u{1D670}'), Some('A'));
}

#[test]
fn shipped_math_latin_bold_digit_zero() {
    let text = std::fs::read_to_string("etc/unicode-mappings/math-latin.toml").unwrap();
    let set = load_str(&text, "math-latin.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // Bold digit 0: U+1D7CE → 0
    assert_eq!(map.lookup_char('\u{1D7CE}'), Some('0'));
}

// ── Math Greek ──────────────────────────────────────────────────────

#[test]
fn shipped_math_greek_parses() {
    let text = std::fs::read_to_string("etc/unicode-mappings/math-greek.toml").unwrap();
    let set = load_str(&text, "math-greek.toml".into()).unwrap();
    assert_eq!(set.type_name, "math_greek");
    assert_eq!(set.direction, Direction::Neutral);
}

#[test]
fn shipped_math_greek_bold_alpha() {
    let text = std::fs::read_to_string("etc/unicode-mappings/math-greek.toml").unwrap();
    let set = load_str(&text, "math-greek.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // Bold Α: U+1D6A8 → Α U+0391
    assert_eq!(map.lookup_char('\u{1D6A8}'), Some('\u{0391}'));
}

#[test]
fn shipped_math_greek_bold_omega() {
    let text = std::fs::read_to_string("etc/unicode-mappings/math-greek.toml").unwrap();
    let set = load_str(&text, "math-greek.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // Bold Ω: U+1D6C0 → Ω U+03A9
    assert_eq!(map.lookup_char('\u{1D6C0}'), Some('\u{03A9}'));
}

#[test]
fn shipped_math_greek_bold_nabla() {
    let text = std::fs::read_to_string("etc/unicode-mappings/math-greek.toml").unwrap();
    let set = load_str(&text, "math-greek.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // Bold ∇: U+1D6C1 → ∇ U+2207
    assert_eq!(map.lookup_char('\u{1D6C1}'), Some('\u{2207}'));
}

// ── Enclosed Alphanumerics ──────────────────────────────────────────

#[test]
fn shipped_enclosed_parses() {
    let text =
        std::fs::read_to_string("etc/unicode-mappings/enclosed-alphanumerics.toml").unwrap();
    let set = load_str(&text, "enclosed-alphanumerics.toml".into()).unwrap();
    assert_eq!(set.type_name, "enclosed_alphanumerics");
    assert_eq!(set.direction, Direction::Narrowing);
}

#[test]
fn shipped_enclosed_circled_one() {
    let text =
        std::fs::read_to_string("etc/unicode-mappings/enclosed-alphanumerics.toml").unwrap();
    let set = load_str(&text, "enclosed-alphanumerics.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // ① U+2460 → 1
    assert_eq!(map.lookup_char('\u{2460}'), Some('1'));
}

#[test]
fn shipped_enclosed_circled_twenty_is_multi_char() {
    let text =
        std::fs::read_to_string("etc/unicode-mappings/enclosed-alphanumerics.toml").unwrap();
    let set = load_str(&text, "enclosed-alphanumerics.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // ⑳ U+2473 → "20" (multi-char)
    let mut out = Vec::new();
    assert!(map.lookup_into('\u{2473}', &mut out));
    assert_eq!(out, vec!['2', '0']);
}

#[test]
fn shipped_enclosed_circled_zero() {
    let text =
        std::fs::read_to_string("etc/unicode-mappings/enclosed-alphanumerics.toml").unwrap();
    let set = load_str(&text, "enclosed-alphanumerics.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // ⓪ U+24EA → 0
    assert_eq!(map.lookup_char('\u{24EA}'), Some('0'));
}

#[test]
fn shipped_enclosed_parenthesized_a() {
    let text =
        std::fs::read_to_string("etc/unicode-mappings/enclosed-alphanumerics.toml").unwrap();
    let set = load_str(&text, "enclosed-alphanumerics.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // ⒜ U+249C → a
    assert_eq!(map.lookup_char('\u{249C}'), Some('a'));
}

// ── Halfwidth Katakana ──────────────────────────────────────────────

#[test]
fn shipped_katakana_parses() {
    let text =
        std::fs::read_to_string("etc/unicode-mappings/halfwidth-katakana.toml").unwrap();
    let set = load_str(&text, "halfwidth-katakana.toml".into()).unwrap();
    assert_eq!(set.type_name, "halfwidth_katakana");
    assert_eq!(set.direction, Direction::Widening);
    assert!(set.tags.contains(&"japanese".to_string()));
}

#[test]
fn shipped_katakana_a_maps() {
    let text =
        std::fs::read_to_string("etc/unicode-mappings/halfwidth-katakana.toml").unwrap();
    let set = load_str(&text, "halfwidth-katakana.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // ｱ U+FF71 → ア U+30A2
    assert_eq!(map.lookup_char('\u{FF71}'), Some('\u{30A2}'));
}

#[test]
fn shipped_katakana_n_maps() {
    let text =
        std::fs::read_to_string("etc/unicode-mappings/halfwidth-katakana.toml").unwrap();
    let set = load_str(&text, "halfwidth-katakana.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // ﾝ U+FF9D → ン U+30F3
    assert_eq!(map.lookup_char('\u{FF9D}'), Some('\u{30F3}'));
}

#[test]
fn shipped_katakana_dakuten_maps_to_combining() {
    let text =
        std::fs::read_to_string("etc/unicode-mappings/halfwidth-katakana.toml").unwrap();
    let set = load_str(&text, "halfwidth-katakana.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // ﾞ U+FF9E → combining dakuten U+3099
    assert_eq!(map.lookup_char('\u{FF9E}'), Some('\u{3099}'));
}

#[test]
fn shipped_katakana_includes_halfwidth_symbols() {
    let text =
        std::fs::read_to_string("etc/unicode-mappings/halfwidth-katakana.toml").unwrap();
    let set = load_str(&text, "halfwidth-katakana.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // ￨ U+FFE8 → │ U+2502
    assert_eq!(map.lookup_char('\u{FFE8}'), Some('\u{2502}'));
    // ￫ U+FFEB → → U+2192
    assert_eq!(map.lookup_char('\u{FFEB}'), Some('\u{2192}'));
}

// ── Halfwidth Hangul ────────────────────────────────────────────────

#[test]
fn shipped_hangul_parses() {
    let text =
        std::fs::read_to_string("etc/unicode-mappings/halfwidth-hangul.toml").unwrap();
    let set = load_str(&text, "halfwidth-hangul.toml".into()).unwrap();
    assert_eq!(set.type_name, "halfwidth_hangul");
    assert_eq!(set.direction, Direction::Widening);
    assert!(set.tags.contains(&"korean".to_string()));
}

#[test]
fn shipped_hangul_first_consonant() {
    let text =
        std::fs::read_to_string("etc/unicode-mappings/halfwidth-hangul.toml").unwrap();
    let set = load_str(&text, "halfwidth-hangul.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // ﾡ U+FFA1 → ㄱ U+3131
    assert_eq!(map.lookup_char('\u{FFA1}'), Some('\u{3131}'));
}

#[test]
fn shipped_hangul_last_consonant() {
    let text =
        std::fs::read_to_string("etc/unicode-mappings/halfwidth-hangul.toml").unwrap();
    let set = load_str(&text, "halfwidth-hangul.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // ﾾ U+FFBE → ㅎ U+314E
    assert_eq!(map.lookup_char('\u{FFBE}'), Some('\u{314E}'));
}

// ── CJK Compatibility Ideographs ────────────────────────────────────

#[test]
fn shipped_cjk_compat_parses() {
    let text =
        std::fs::read_to_string("etc/unicode-mappings/cjk-compat-ideographs.toml").unwrap();
    let set = load_str(&text, "cjk-compat-ideographs.toml".into()).unwrap();
    assert_eq!(set.type_name, "cjk_compat_ideographs");
    assert_eq!(set.direction, Direction::Neutral);
}

#[test]
fn shipped_cjk_compat_f900_maps() {
    let text =
        std::fs::read_to_string("etc/unicode-mappings/cjk-compat-ideographs.toml").unwrap();
    let set = load_str(&text, "cjk-compat-ideographs.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // 豈 U+F900 → 豈 U+8C48
    assert_eq!(map.lookup_char('\u{F900}'), Some('\u{8C48}'));
}

// ── TOML format validation ──────────────────────────────────────────

#[test]
fn rejects_invalid_toml_syntax() {
    let bad = "not valid toml [[[";
    assert!(load_str(bad, "test".into()).is_err());
}

#[test]
fn rejects_missing_metadata() {
    let toml = r#"
[[pairs]]
from = "FF01"
to = "0021"
"#;
    assert!(load_str(toml, "test".into()).is_err());
}

#[test]
fn rejects_invalid_hex() {
    let toml = r#"
[metadata]
type = "test"
description = "test"
direction = "neutral"
tags = []

[[pairs]]
from = "ZZZZ"
to = "0021"
"#;
    assert!(load_str(toml, "test".into()).is_err());
}

#[test]
fn rejects_surrogate_codepoint() {
    let toml = r#"
[metadata]
type = "test"
description = "test"
direction = "neutral"
tags = []

[[pairs]]
from = "D800"
to = "0021"
"#;
    assert!(load_str(toml, "test".into()).is_err());
}

#[test]
fn accepts_empty_ranges_and_pairs() {
    let toml = r#"
[metadata]
type = "test_empty"
description = "empty set"
direction = "neutral"
tags = []
"#;
    let set = load_str(toml, "test".into()).unwrap();
    assert!(set.ranges.is_empty());
    assert!(set.pairs.is_empty());
}

#[test]
fn duplicate_type_rejected_on_merge() {
    let toml = r#"
[metadata]
type = "fullwidth_ascii"
description = "trying to override builtin"
direction = "narrowing"
tags = []
"#;
    let set = load_str(toml, "test".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    assert!(map.merge_set(set).is_err());
    // Builtin still works
    assert_eq!(map.lookup_char('\u{FF21}'), Some('A'));
}

// ── Enclosed CJK ────────────────────────────────────────────────────

#[test]
fn shipped_enclosed_cjk_parses() {
    let text = std::fs::read_to_string("etc/unicode-mappings/enclosed-cjk.toml").unwrap();
    let set = load_str(&text, "enclosed-cjk.toml".into()).unwrap();
    assert_eq!(set.type_name, "enclosed_cjk");
    assert_eq!(set.direction, Direction::Neutral);
    assert!(set.tags.contains(&"cjk".to_string()));
}

#[test]
fn shipped_enclosed_cjk_parenthesized_one() {
    let text = std::fs::read_to_string("etc/unicode-mappings/enclosed-cjk.toml").unwrap();
    let set = load_str(&text, "enclosed-cjk.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // ㈠ U+3220 → 一 U+4E00
    assert_eq!(map.lookup_char('\u{3220}'), Some('\u{4E00}'));
}

#[test]
fn shipped_enclosed_cjk_circled_katakana_a() {
    let text = std::fs::read_to_string("etc/unicode-mappings/enclosed-cjk.toml").unwrap();
    let set = load_str(&text, "enclosed-cjk.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // ㋐ U+32D0 → ア U+30A2
    assert_eq!(map.lookup_char('\u{32D0}'), Some('\u{30A2}'));
}

#[test]
fn shipped_enclosed_cjk_reiwa() {
    let text = std::fs::read_to_string("etc/unicode-mappings/enclosed-cjk.toml").unwrap();
    let set = load_str(&text, "enclosed-cjk.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // ㋿ U+32FF → 令和
    let mut out = Vec::new();
    assert!(map.lookup_into('\u{32FF}', &mut out));
    assert_eq!(out, vec!['\u{4EE4}', '\u{548C}']);
}

// ── CJK Compatibility ───────────────────────────────────────────────

#[test]
fn shipped_cjk_compatibility_parses() {
    let text = std::fs::read_to_string("etc/unicode-mappings/cjk-compatibility.toml").unwrap();
    let set = load_str(&text, "cjk-compatibility.toml".into()).unwrap();
    assert_eq!(set.type_name, "cjk_compatibility");
    assert_eq!(set.direction, Direction::Narrowing);
    assert!(set.tags.contains(&"japanese".to_string()));
}

#[test]
fn shipped_cjk_compatibility_kiro() {
    let text = std::fs::read_to_string("etc/unicode-mappings/cjk-compatibility.toml").unwrap();
    let set = load_str(&text, "cjk-compatibility.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // ㌔ U+3314 → キロ
    let mut out = Vec::new();
    assert!(map.lookup_into('\u{3314}', &mut out));
    assert_eq!(out, vec!['\u{30AD}', '\u{30ED}']);
}

#[test]
fn shipped_cjk_compatibility_heisei() {
    let text = std::fs::read_to_string("etc/unicode-mappings/cjk-compatibility.toml").unwrap();
    let set = load_str(&text, "cjk-compatibility.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // ㍻ U+337B → 平成
    let mut out = Vec::new();
    assert!(map.lookup_into('\u{337B}', &mut out));
    assert_eq!(out, vec!['\u{5E73}', '\u{6210}']);
}

#[test]
fn shipped_cjk_compatibility_cm() {
    let text = std::fs::read_to_string("etc/unicode-mappings/cjk-compatibility.toml").unwrap();
    let set = load_str(&text, "cjk-compatibility.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // ㎝ U+339D → cm
    let mut out = Vec::new();
    assert!(map.lookup_into('\u{339D}', &mut out));
    assert_eq!(out, vec!['c', 'm']);
}

// ── Enclosed Alphanumeric Supplement ────────────────────────────────

#[test]
fn shipped_enclosed_supplement_parses() {
    let text =
        std::fs::read_to_string("etc/unicode-mappings/enclosed-alphanumeric-supplement.toml")
            .unwrap();
    let set = load_str(&text, "enclosed-alphanumeric-supplement.toml".into()).unwrap();
    assert_eq!(set.type_name, "enclosed_alphanumeric_supplement");
    assert_eq!(set.direction, Direction::Narrowing);
}

#[test]
fn shipped_enclosed_supplement_parenthesized_a() {
    let text =
        std::fs::read_to_string("etc/unicode-mappings/enclosed-alphanumeric-supplement.toml")
            .unwrap();
    let set = load_str(&text, "enclosed-alphanumeric-supplement.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // 🄐 U+1F110 → A
    assert_eq!(map.lookup_char('\u{1F110}'), Some('A'));
}

#[test]
fn shipped_enclosed_supplement_negative_circled_z() {
    let text =
        std::fs::read_to_string("etc/unicode-mappings/enclosed-alphanumeric-supplement.toml")
            .unwrap();
    let set = load_str(&text, "enclosed-alphanumeric-supplement.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // 🅩 U+1F169 → Z
    assert_eq!(map.lookup_char('\u{1F169}'), Some('Z'));
}

// ── CJK Compat Ideographs Supplement ────────────────────────────────

#[test]
fn shipped_cjk_supplement_parses() {
    let text =
        std::fs::read_to_string("etc/unicode-mappings/cjk-compat-ideographs-supplement.toml")
            .unwrap();
    let set = load_str(&text, "cjk-compat-ideographs-supplement.toml".into()).unwrap();
    assert_eq!(set.type_name, "cjk_compat_ideographs_supplement");
    assert_eq!(set.direction, Direction::Neutral);
}

#[test]
fn shipped_cjk_supplement_first_entry() {
    let text =
        std::fs::read_to_string("etc/unicode-mappings/cjk-compat-ideographs-supplement.toml")
            .unwrap();
    let set = load_str(&text, "cjk-compat-ideographs-supplement.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // U+2F800 → U+4E3D
    assert_eq!(map.lookup_char('\u{2F800}'), Some('\u{4E3D}'));
}

// ── Arabic Presentation Forms ───────────────────────────────────────

#[test]
fn shipped_arabic_parses() {
    let text =
        std::fs::read_to_string("etc/unicode-mappings/arabic-presentation-forms.toml").unwrap();
    let set = load_str(&text, "arabic-presentation-forms.toml".into()).unwrap();
    assert_eq!(set.type_name, "arabic_presentation_forms");
    assert_eq!(set.direction, Direction::Neutral);
    assert!(set.tags.contains(&"arabic".to_string()));
}

#[test]
fn shipped_arabic_hamza() {
    let text =
        std::fs::read_to_string("etc/unicode-mappings/arabic-presentation-forms.toml").unwrap();
    let set = load_str(&text, "arabic-presentation-forms.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // FE80 → 0621 (hamza)
    assert_eq!(map.lookup_char('\u{FE80}'), Some('\u{0621}'));
}

#[test]
fn shipped_arabic_beh_isolated() {
    let text =
        std::fs::read_to_string("etc/unicode-mappings/arabic-presentation-forms.toml").unwrap();
    let set = load_str(&text, "arabic-presentation-forms.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // FE8F → 0628 (beh isolated)
    assert_eq!(map.lookup_char('\u{FE8F}'), Some('\u{0628}'));
}

#[test]
fn shipped_arabic_lam_alef_ligature() {
    let text =
        std::fs::read_to_string("etc/unicode-mappings/arabic-presentation-forms.toml").unwrap();
    let set = load_str(&text, "arabic-presentation-forms.toml".into()).unwrap();
    let mut map = UnicodeMap::builtin();
    map.merge_set(set).unwrap();
    // FEFB → lam + alef
    let mut out = Vec::new();
    assert!(map.lookup_into('\u{FEFB}', &mut out));
    assert_eq!(out, vec!['\u{0644}', '\u{0627}']);
}
