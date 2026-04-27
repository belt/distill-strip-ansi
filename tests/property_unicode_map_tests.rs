#![cfg(feature = "unicode-normalize")]

use proptest::prelude::*;
use strip_ansi::unicode_map::UnicodeMap;

// ── Generators ──────────────────────────────────────────────────────

/// Generate an arbitrary valid Unicode char.
fn arb_char() -> impl Strategy<Value = char> {
    (0u32..=0x10FFFF)
        .prop_filter("valid unicode", |&cp| char::from_u32(cp).is_some())
        .prop_map(|cp| char::from_u32(cp).unwrap())
}

/// Generate a char known to be in a builtin range.
fn arb_fullwidth_ascii() -> impl Strategy<Value = char> {
    (0xFF01u32..=0xFF5Eu32).prop_map(|cp| char::from_u32(cp).unwrap())
}

fn arb_math_bold_upper() -> impl Strategy<Value = char> {
    (0x1D400u32..=0x1D419u32).prop_map(|cp| char::from_u32(cp).unwrap())
}

fn arb_math_bold_lower() -> impl Strategy<Value = char> {
    (0x1D41Au32..=0x1D433u32).prop_map(|cp| char::from_u32(cp).unwrap())
}

fn arb_circled_upper() -> impl Strategy<Value = char> {
    (0x24B6u32..=0x24CFu32).prop_map(|cp| char::from_u32(cp).unwrap())
}

fn arb_circled_lower() -> impl Strategy<Value = char> {
    (0x24D0u32..=0x24E9u32).prop_map(|cp| char::from_u32(cp).unwrap())
}

// ── Properties ──────────────────────────────────────────────────────

proptest! {
    /// Fullwidth ASCII always maps to the ASCII range.
    #[test]
    fn fullwidth_ascii_maps_to_ascii(c in arb_fullwidth_ascii()) {
        let map = UnicodeMap::builtin();
        let result = map.lookup_char(c);
        prop_assert!(result.is_some(), "U+{:04X} should map", c as u32);
        let target = result.unwrap();
        prop_assert!(
            target.is_ascii(),
            "U+{:04X} mapped to U+{:04X} which is not ASCII",
            c as u32, target as u32
        );
    }

    /// Math bold uppercase always maps to A-Z.
    #[test]
    fn math_bold_upper_maps_to_az(c in arb_math_bold_upper()) {
        let map = UnicodeMap::builtin();
        let result = map.lookup_char(c);
        prop_assert!(result.is_some());
        let target = result.unwrap();
        prop_assert!(
            ('A'..='Z').contains(&target),
            "U+{:04X} mapped to {:?}, expected A-Z",
            c as u32, target
        );
    }

    /// Math bold lowercase always maps to a-z.
    #[test]
    fn math_bold_lower_maps_to_az(c in arb_math_bold_lower()) {
        let map = UnicodeMap::builtin();
        let result = map.lookup_char(c);
        prop_assert!(result.is_some());
        let target = result.unwrap();
        prop_assert!(
            ('a'..='z').contains(&target),
            "U+{:04X} mapped to {:?}, expected a-z",
            c as u32, target
        );
    }

    /// Circled uppercase always maps to A-Z.
    #[test]
    fn circled_upper_maps_to_az(c in arb_circled_upper()) {
        let map = UnicodeMap::builtin();
        let result = map.lookup_char(c);
        prop_assert!(result.is_some());
        let target = result.unwrap();
        prop_assert!(('A'..='Z').contains(&target));
    }

    /// Circled lowercase always maps to a-z.
    #[test]
    fn circled_lower_maps_to_az(c in arb_circled_lower()) {
        let map = UnicodeMap::builtin();
        let result = map.lookup_char(c);
        prop_assert!(result.is_some());
        let target = result.unwrap();
        prop_assert!(('a'..='z').contains(&target));
    }

    /// No builtin mapping target is itself a builtin source.
    /// (Prevents mapping chains: A→B→C)
    #[test]
    fn no_transitive_mappings(c in arb_char()) {
        let map = UnicodeMap::builtin();
        if let Some(target) = map.lookup_char(c) {
            prop_assert!(
                map.lookup_char(target).is_none(),
                "U+{:04X} maps to U+{:04X} which itself maps — transitive chain",
                c as u32, target as u32
            );
        }
    }

    /// Plain ASCII is never a mapping source.
    #[test]
    fn ascii_never_mapped(cp in 0x20u32..=0x7Eu32) {
        let map = UnicodeMap::builtin();
        let c = char::from_u32(cp).unwrap();
        prop_assert!(
            map.lookup_char(c).is_none(),
            "ASCII U+{:04X} should not be a mapping source",
            cp
        );
    }

    /// lookup_into and lookup_char agree for single-char targets.
    #[test]
    fn lookup_into_consistent_with_lookup_char(c in arb_fullwidth_ascii()) {
        let map = UnicodeMap::builtin();
        let char_result = map.lookup_char(c);
        let mut vec_result = Vec::new();
        let found = map.lookup_into(c, &mut vec_result);

        prop_assert!(found, "lookup_into should find U+{:04X}", c as u32);
        prop_assert_eq!(vec_result.len(), 1);
        prop_assert_eq!(
            char_result, Some(vec_result[0]),
            "lookup_char and lookup_into disagree for U+{:04X}",
            c as u32
        );
    }
}
