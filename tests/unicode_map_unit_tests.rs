#![cfg(feature = "unicode-normalize")]

use strip_ansi::unicode_map::{CharMappingSet, Direction, PairMapping, UnicodeMap};

// ── Builtin coverage ────────────────────────────────────────────────

#[test]
fn builtin_has_four_sets() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.set_count(), 5);
}

#[test]
fn builtin_set_names() {
    let map = UnicodeMap::builtin();
    let names: Vec<&str> = map.sets().iter().map(|s| s.type_name.as_str()).collect();
    assert!(names.contains(&"fullwidth_ascii"));
    assert!(names.contains(&"math_latin_bold"));
    assert!(names.contains(&"latin_ligatures"));
    assert!(names.contains(&"enclosed_circled_letters"));
    assert!(names.contains(&"superscript_subscript"));
}

// ── Fullwidth ASCII (94 chars + 7 symbols) ──────────────────────────

#[test]
fn fullwidth_exclamation_mark() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{FF01}'), Some('!'));
}

#[test]
fn fullwidth_tilde() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{FF5E}'), Some('~'));
}

#[test]
fn fullwidth_digit_zero() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{FF10}'), Some('0'));
}

#[test]
fn fullwidth_digit_nine() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{FF19}'), Some('9'));
}

#[test]
fn fullwidth_uppercase_a() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{FF21}'), Some('A'));
}

#[test]
fn fullwidth_uppercase_z() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{FF3A}'), Some('Z'));
}

#[test]
fn fullwidth_lowercase_a() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{FF41}'), Some('a'));
}

#[test]
fn fullwidth_lowercase_z() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{FF5A}'), Some('z'));
}

#[test]
fn fullwidth_cent_sign() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{FFE0}'), Some('\u{00A2}'));
}

#[test]
fn fullwidth_pound_sign() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{FFE1}'), Some('\u{00A3}'));
}

#[test]
fn fullwidth_yen_sign() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{FFE5}'), Some('\u{00A5}'));
}

#[test]
fn fullwidth_won_sign() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{FFE6}'), Some('\u{20A9}'));
}

// ── Math bold Latin (52 chars) ──────────────────────────────────────

#[test]
fn math_bold_a() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{1D400}'), Some('A'));
}

#[test]
fn math_bold_z() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{1D419}'), Some('Z'));
}

#[test]
fn math_bold_small_a() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{1D41A}'), Some('a'));
}

#[test]
fn math_bold_small_z() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{1D433}'), Some('z'));
}

#[test]
fn math_bold_boundary_before_range() {
    let map = UnicodeMap::builtin();
    // U+1D3FF is just before bold A
    assert_eq!(map.lookup_char('\u{1D3FF}'), None);
}

#[test]
fn math_bold_boundary_after_range() {
    let map = UnicodeMap::builtin();
    // U+1D434 is italic A — not in builtins (in TOML)
    // Should not match math_latin_bold range
    assert_eq!(map.lookup_char('\u{1D434}'), None);
}

// ── Enclosed circled letters (52 chars) ─────────────────────────────

#[test]
fn circled_a_upper() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{24B6}'), Some('A'));
}

#[test]
fn circled_z_upper() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{24CF}'), Some('Z'));
}

#[test]
fn circled_a_lower() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{24D0}'), Some('a'));
}

#[test]
fn circled_z_lower() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{24E9}'), Some('z'));
}

// ── Superscript / subscript ─────────────────────────────────────────

#[test]
fn superscript_zero() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{2070}'), Some('0'));
}

#[test]
fn superscript_one() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{00B9}'), Some('1'));
}

#[test]
fn superscript_two() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{00B2}'), Some('2'));
}

#[test]
fn superscript_three() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{00B3}'), Some('3'));
}

#[test]
fn superscript_four_through_nine() {
    let map = UnicodeMap::builtin();
    for (cp, digit) in (0x2074..=0x2079u32).zip('4'..='9') {
        let c = char::from_u32(cp).unwrap();
        assert_eq!(map.lookup_char(c), Some(digit), "U+{cp:04X} should map to {digit}");
    }
}

#[test]
fn subscript_zero_through_nine() {
    let map = UnicodeMap::builtin();
    for (cp, digit) in (0x2080..=0x2089u32).zip('0'..='9') {
        let c = char::from_u32(cp).unwrap();
        assert_eq!(map.lookup_char(c), Some(digit), "U+{cp:04X} should map to {digit}");
    }
}

#[test]
fn superscript_n() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{207F}'), Some('n'));
}

#[test]
fn superscript_plus() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{207A}'), Some('+'));
}

#[test]
fn subscript_a() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{2090}'), Some('a'));
}

#[test]
fn subscript_t() {
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{209C}'), Some('t'));
}

// ── Latin ligatures (7 chars) ───────────────────────────────────────

#[test]
fn ligature_fi() {
    let map = UnicodeMap::builtin();
    let mut out = Vec::new();
    assert!(map.lookup_into('\u{FB01}', &mut out));
    assert_eq!(out, vec!['f', 'i']);
}

#[test]
fn ligature_ffl() {
    let map = UnicodeMap::builtin();
    let mut out = Vec::new();
    assert!(map.lookup_into('\u{FB04}', &mut out));
    assert_eq!(out, vec!['f', 'f', 'l']);
}

#[test]
fn ligature_st() {
    let map = UnicodeMap::builtin();
    let mut out = Vec::new();
    assert!(map.lookup_into('\u{FB06}', &mut out));
    assert_eq!(out, vec!['s', 't']);
}

#[test]
fn ligature_fi_not_single_char() {
    // Multi-char target: lookup_char returns None
    let map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{FB01}'), None);
}

// ── No false positives ──────────────────────────────────────────────

#[test]
fn plain_ascii_not_mapped() {
    let map = UnicodeMap::builtin();
    for c in ' '..='~' {
        assert_eq!(map.lookup_char(c), None, "ASCII {c:?} should not be mapped");
    }
}

#[test]
fn common_unicode_not_mapped() {
    let map = UnicodeMap::builtin();
    // Common non-ASCII chars that should NOT be mapped
    assert_eq!(map.lookup_char('é'), None);
    assert_eq!(map.lookup_char('ñ'), None);
    assert_eq!(map.lookup_char('ü'), None);
    assert_eq!(map.lookup_char('中'), None);
    assert_eq!(map.lookup_char('日'), None);
    assert_eq!(map.lookup_char('ア'), None); // standard katakana, not halfwidth
}

// ── lookup_into ─────────────────────────────────────────────────────

#[test]
fn lookup_into_range_hit() {
    let map = UnicodeMap::builtin();
    let mut out = Vec::new();
    assert!(map.lookup_into('\u{FF21}', &mut out));
    assert_eq!(out, vec!['A']);
}

#[test]
fn lookup_into_pair_hit() {
    let map = UnicodeMap::builtin();
    let mut out = Vec::new();
    assert!(map.lookup_into('\u{FFE0}', &mut out));
    assert_eq!(out, vec!['\u{00A2}']);
}

#[test]
fn lookup_into_miss() {
    let map = UnicodeMap::builtin();
    let mut out = Vec::new();
    assert!(!map.lookup_into('A', &mut out));
    assert!(out.is_empty());
}

#[test]
fn lookup_into_accumulates() {
    let map = UnicodeMap::builtin();
    let mut out = Vec::new();
    map.lookup_into('\u{FF21}', &mut out); // A
    map.lookup_into('\u{FF22}', &mut out); // B
    assert_eq!(out, vec!['A', 'B']);
}

// ── Merge / remove ──────────────────────────────────────────────────

#[test]
fn merge_duplicate_rejected() {
    let mut map = UnicodeMap::builtin();
    let dup = CharMappingSet {
        type_name: "fullwidth_ascii".into(),
        description: "dup".into(),
        direction: Direction::Narrowing,
        tags: vec![],
        ranges: vec![],
        pairs: vec![],
    };
    let err = map.merge_set(dup).unwrap_err();
    assert_eq!(err, "fullwidth_ascii");
}

#[test]
fn merge_new_set_adds_mappings() {
    let mut map = UnicodeMap::builtin();
    let count_before = map.set_count();
    let new = CharMappingSet {
        type_name: "test_custom".into(),
        description: "test".into(),
        direction: Direction::Neutral,
        tags: vec![],
        ranges: vec![],
        pairs: vec![PairMapping {
            from: '\u{2603}',
            target: vec!['*'],
        }],
    };
    map.merge_set(new).unwrap();
    assert_eq!(map.set_count(), count_before + 1);
    assert_eq!(map.lookup_char('\u{2603}'), Some('*'));
}

#[test]
fn remove_builtin_disables_mappings() {
    let mut map = UnicodeMap::builtin();
    assert_eq!(map.lookup_char('\u{FF21}'), Some('A'));
    assert!(map.remove_set("fullwidth_ascii"));
    assert_eq!(map.lookup_char('\u{FF21}'), None);
    // Other builtins still work
    assert_eq!(map.lookup_char('\u{1D400}'), Some('A'));
}

#[test]
fn remove_nonexistent_is_noop() {
    let mut map = UnicodeMap::builtin();
    let count = map.set_count();
    assert!(!map.remove_set("does_not_exist"));
    assert_eq!(map.set_count(), count);
}

// ── Direction metadata ──────────────────────────────────────────────

#[test]
fn builtin_directions() {
    let map = UnicodeMap::builtin();
    for set in map.sets() {
        match set.type_name.as_str() {
            "fullwidth_ascii" | "math_latin_bold" | "enclosed_circled_letters" => {
                assert_eq!(set.direction, Direction::Narrowing, "{}", set.type_name);
            }
            "superscript_subscript" | "latin_ligatures" => {
                assert_eq!(set.direction, Direction::Neutral, "{}", set.type_name);
            }
            _ => panic!("unexpected builtin set: {}", set.type_name),
        }
    }
}
