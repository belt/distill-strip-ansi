//! Unicode character mapping: built-in security defaults + TOML extension.
//!
//! Two-layer architecture (same pattern as [`crate::threat_db`]):
//! 1. Built-in defaults compiled into the binary (immutable, ~247 chars).
//! 2. External TOML files loaded at startup (additive only).
//!
//! Built-in mappings are security-motivated: fullwidth ASCII homographs,
//! math bold Latin, circled letters, superscript/subscript digits.
//!
//! TOML files provide canonicalization for niche audiences: CJK compat
//! ideographs, halfwidth katakana/hangul, styled math Greek, etc.
//!
//! Gated behind the `unicode-normalize` feature flag.
//! TOML loading additionally requires the `toml-config` feature.

#![forbid(unsafe_code)]

use alloc::string::String;
use alloc::vec::Vec;

// ── Data types ──────────────────────────────────────────────────────

/// A contiguous codepoint range with constant offset.
///
/// Every codepoint `c` in `[from_start..=from_end]` maps to
/// `(c as i64 + offset) as u32`, i.e. `c - from_start + to_start`.
#[derive(Clone, Debug)]
pub struct RangeMapping {
    pub from_start: u32,
    pub from_end: u32,
    pub offset: i32,
}

/// A single source→target codepoint pair.
///
/// `target` is a `Vec<char>` to support multi-codepoint targets
/// (e.g. ⑳ → "20"). Most entries have exactly one target char.
#[derive(Clone, Debug)]
pub struct PairMapping {
    pub from: char,
    pub target: Vec<char>,
}

/// Direction metadata: effect on terminal column width.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Direction {
    /// 2-col → 1-col (e.g. fullwidth ASCII → ASCII).
    Narrowing,
    /// 1-col → 2-col (e.g. halfwidth katakana → standard).
    Widening,
    /// Same column width.
    Neutral,
}

/// A named set of character mappings with metadata.
#[derive(Clone, Debug)]
pub struct CharMappingSet {
    pub type_name: String,
    pub description: String,
    pub direction: Direction,
    pub tags: Vec<String>,
    pub ranges: Vec<RangeMapping>,
    pub pairs: Vec<PairMapping>,
}

/// Merged mapping database: built-in defaults + loaded TOML files.
///
/// Lookup is two-phase: ranges first (linear scan, typically 1–5),
/// then pairs (binary search by source codepoint).
#[derive(Clone, Debug)]
pub struct UnicodeMap {
    sets: Vec<CharMappingSet>,
    /// Flattened + sorted pairs from all sets for binary search.
    all_pairs: Vec<PairMapping>,
    /// Flattened ranges from all sets.
    all_ranges: Vec<RangeMapping>,
}

impl UnicodeMap {
    /// Create a map with only the compiled-in built-in mappings.
    #[must_use]
    pub fn builtin() -> Self {
        let sets = builtin_sets();
        Self::from_sets(sets)
    }

    /// Create a map from an explicit list of sets.
    #[must_use]
    pub fn from_sets(sets: Vec<CharMappingSet>) -> Self {
        let mut all_ranges = Vec::new();
        let mut all_pairs = Vec::new();
        for set in &sets {
            all_ranges.extend_from_slice(&set.ranges);
            all_pairs.extend_from_slice(&set.pairs);
        }
        all_pairs.sort_by_key(|p| p.from as u32);
        Self {
            sets,
            all_pairs,
            all_ranges,
        }
    }

    /// Look up a character, returning a single replacement char.
    ///
    /// Handles both ranges (computed) and single-target pairs.
    /// Returns `None` if no mapping exists or if the target is
    /// multi-codepoint (use [`lookup`] for those).
    #[must_use]
    pub fn lookup_char(&self, c: char) -> Option<char> {
        let cp = c as u32;

        // Phase 1: range check.
        for r in &self.all_ranges {
            if cp >= r.from_start && cp <= r.from_end {
                let target_cp = (cp as i64 + r.offset as i64) as u32;
                return char::from_u32(target_cp);
            }
        }

        // Phase 2: binary search in pairs (single-char targets only).
        self.all_pairs
            .binary_search_by_key(&cp, |p| p.from as u32)
            .ok()
            .and_then(|idx| {
                let t = &self.all_pairs[idx].target;
                if t.len() == 1 { Some(t[0]) } else { None }
            })
    }

    /// Look up a character, writing replacement chars into `out`.
    ///
    /// Returns `true` if a mapping was found (and chars were pushed).
    /// Handles ranges, single-char pairs, and multi-char pairs.
    pub fn lookup_into(&self, c: char, out: &mut Vec<char>) -> bool {
        let cp = c as u32;

        // Phase 1: range check.
        for r in &self.all_ranges {
            if cp >= r.from_start && cp <= r.from_end {
                let target_cp = (cp as i64 + r.offset as i64) as u32;
                if let Some(tc) = char::from_u32(target_cp) {
                    out.push(tc);
                    return true;
                }
                return false;
            }
        }

        // Phase 2: binary search in pairs.
        if let Ok(idx) = self.all_pairs.binary_search_by_key(&cp, |p| p.from as u32) {
            out.extend_from_slice(&self.all_pairs[idx].target);
            return true;
        }

        false
    }

    /// Returns the number of mapping sets loaded.
    #[must_use]
    pub fn set_count(&self) -> usize {
        self.sets.len()
    }

    /// Returns a slice of all loaded mapping sets.
    #[must_use]
    pub fn sets(&self) -> &[CharMappingSet] {
        &self.sets
    }

    /// Merge another set into this map. Rejects duplicate `type_name`.
    ///
    /// Returns `Err` with the duplicate type name if rejected.
    pub fn merge_set(&mut self, set: CharMappingSet) -> Result<(), String> {
        if self.sets.iter().any(|s| s.type_name == set.type_name) {
            return Err(set.type_name);
        }
        for r in &set.ranges {
            self.all_ranges.push(r.clone());
        }
        for p in &set.pairs {
            // Insert sorted.
            let pos = self
                .all_pairs
                .binary_search_by_key(&(p.from as u32), |x| x.from as u32)
                .unwrap_or_else(|i| i);
            self.all_pairs.insert(pos, p.clone());
        }
        self.sets.push(set);
        Ok(())
    }

    /// Remove a set by type name. Returns `true` if found and removed.
    pub fn remove_set(&mut self, type_name: &str) -> bool {
        let Some(idx) = self.sets.iter().position(|s| s.type_name == type_name) else {
            return false;
        };
        self.sets.remove(idx);
        // Rebuild flattened indexes.
        self.rebuild_indexes();
        true
    }

    fn rebuild_indexes(&mut self) {
        self.all_ranges.clear();
        self.all_pairs.clear();
        for set in &self.sets {
            self.all_ranges.extend_from_slice(&set.ranges);
            self.all_pairs.extend_from_slice(&set.pairs);
        }
        self.all_pairs.sort_by_key(|p| p.from as u32);
    }
}

// ── Built-in mappings ───────────────────────────────────────────────

/// The compiled-in security mapping sets (~247 chars).
fn builtin_sets() -> Vec<CharMappingSet> {
    vec![
        builtin_fullwidth_ascii(),
        builtin_math_latin_bold(),
        builtin_latin_ligatures(),
        builtin_enclosed_circled_letters(),
        builtin_superscript_subscript(),
    ]
}

/// Fullwidth ASCII (94 chars) + fullwidth symbols (7 chars).
fn builtin_fullwidth_ascii() -> CharMappingSet {
    CharMappingSet {
        type_name: "fullwidth_ascii".into(),
        description: "Fullwidth ASCII and symbols → standard equivalents".into(),
        direction: Direction::Narrowing,
        tags: vec!["security".into(), "ascii-normalize".into()],
        ranges: vec![
            // U+FF01–FF5E → U+0021–007E (offset = 0x0021 - 0xFF01 = -0xFEE0)
            RangeMapping {
                from_start: 0xFF01,
                from_end: 0xFF5E,
                offset: -0xFEE0_i32,
            },
        ],
        pairs: vec![
            // Fullwidth symbols: U+FFE0–FFE6
            PairMapping { from: '\u{FFE0}', target: vec!['\u{00A2}'] }, // ￠ → ¢
            PairMapping { from: '\u{FFE1}', target: vec!['\u{00A3}'] }, // ￡ → £
            PairMapping { from: '\u{FFE2}', target: vec!['\u{00AC}'] }, // ￢ → ¬
            PairMapping { from: '\u{FFE3}', target: vec!['\u{00AF}'] }, // ￣ → ¯
            PairMapping { from: '\u{FFE4}', target: vec!['\u{00A6}'] }, // ￤ → ¦
            PairMapping { from: '\u{FFE5}', target: vec!['\u{00A5}'] }, // ￥ → ¥
            PairMapping { from: '\u{FFE6}', target: vec!['\u{20A9}'] }, // ￦ → ₩
        ],
    }
}

/// Math bold Latin A–Z, a–z (52 chars).
fn builtin_math_latin_bold() -> CharMappingSet {
    CharMappingSet {
        type_name: "math_latin_bold".into(),
        description: "Mathematical bold Latin → plain ASCII".into(),
        direction: Direction::Narrowing,
        tags: vec!["security".into(), "ascii-normalize".into()],
        ranges: vec![
            // Bold uppercase: U+1D400–1D419 → A–Z
            RangeMapping {
                from_start: 0x1D400,
                from_end: 0x1D419,
                offset: 0x0041_i32 - 0x1D400_i32,
            },
            // Bold lowercase: U+1D41A–1D433 → a–z
            RangeMapping {
                from_start: 0x1D41A,
                from_end: 0x1D433,
                offset: 0x0061_i32 - 0x1D41A_i32,
            },
        ],
        pairs: vec![],
    }
}

/// Latin ligatures: ﬀ→ff, ﬁ→fi, ﬂ→fl, ﬃ→ffi, ﬄ→ffl, ﬅ→st, ﬆ→st (7 chars).
///
/// Common in copy-paste from PDFs. `ﬁle` does not match `file` in grep.
fn builtin_latin_ligatures() -> CharMappingSet {
    CharMappingSet {
        type_name: "latin_ligatures".into(),
        description: "Latin ligatures → ASCII letter sequences".into(),
        direction: Direction::Neutral,
        tags: vec!["security".into(), "ascii-normalize".into()],
        ranges: vec![],
        pairs: vec![
            PairMapping { from: '\u{FB00}', target: vec!['f', 'f'] },       // ﬀ
            PairMapping { from: '\u{FB01}', target: vec!['f', 'i'] },       // ﬁ
            PairMapping { from: '\u{FB02}', target: vec!['f', 'l'] },       // ﬂ
            PairMapping { from: '\u{FB03}', target: vec!['f', 'f', 'i'] },  // ﬃ
            PairMapping { from: '\u{FB04}', target: vec!['f', 'f', 'l'] },  // ﬄ
            PairMapping { from: '\u{FB05}', target: vec!['s', 't'] },       // ﬅ (long s t)
            PairMapping { from: '\u{FB06}', target: vec!['s', 't'] },       // ﬆ
        ],
    }
}

/// Enclosed circled letters: Ⓐ–Ⓩ → A–Z, ⓐ–ⓩ → a–z (52 chars).
fn builtin_enclosed_circled_letters() -> CharMappingSet {
    CharMappingSet {
        type_name: "enclosed_circled_letters".into(),
        description: "Circled Latin letters → plain ASCII".into(),
        direction: Direction::Narrowing,
        tags: vec!["security".into(), "ascii-normalize".into()],
        ranges: vec![
            // Ⓐ–Ⓩ: U+24B6–24CF → A–Z
            RangeMapping {
                from_start: 0x24B6,
                from_end: 0x24CF,
                offset: 0x0041_i32 - 0x24B6_i32,
            },
            // ⓐ–ⓩ: U+24D0–24E9 → a–z
            RangeMapping {
                from_start: 0x24D0,
                from_end: 0x24E9,
                offset: 0x0061_i32 - 0x24D0_i32,
            },
        ],
        pairs: vec![],
    }
}

/// Superscript and subscript digits, letters, operators (~42 chars).
fn builtin_superscript_subscript() -> CharMappingSet {
    CharMappingSet {
        type_name: "superscript_subscript".into(),
        description: "Superscript/subscript forms → plain ASCII".into(),
        direction: Direction::Neutral,
        tags: vec!["security".into(), "ascii-normalize".into()],
        ranges: vec![
            // Superscript ⁴–⁹: U+2074–2079 → 4–9
            RangeMapping {
                from_start: 0x2074,
                from_end: 0x2079,
                offset: 0x0034_i32 - 0x2074_i32,
            },
            // Subscript ₀–₉: U+2080–2089 → 0–9
            RangeMapping {
                from_start: 0x2080,
                from_end: 0x2089,
                offset: 0x0030_i32 - 0x2080_i32,
            },
        ],
        pairs: vec![
            // Superscript digits (scattered)
            PairMapping { from: '\u{2070}', target: vec!['0'] }, // ⁰
            PairMapping { from: '\u{00B9}', target: vec!['1'] }, // ¹
            PairMapping { from: '\u{00B2}', target: vec!['2'] }, // ²
            PairMapping { from: '\u{00B3}', target: vec!['3'] }, // ³
            // Superscript operators
            PairMapping { from: '\u{207A}', target: vec!['+'] }, // ⁺
            PairMapping { from: '\u{207B}', target: vec!['-'] }, // ⁻
            PairMapping { from: '\u{207C}', target: vec!['='] }, // ⁼
            PairMapping { from: '\u{207D}', target: vec!['('] }, // ⁽
            PairMapping { from: '\u{207E}', target: vec![')'] }, // ⁾
            // Superscript letters
            PairMapping { from: '\u{2071}', target: vec!['i'] }, // ⁱ
            PairMapping { from: '\u{207F}', target: vec!['n'] }, // ⁿ
            // Subscript operators
            PairMapping { from: '\u{208A}', target: vec!['+'] }, // ₊
            PairMapping { from: '\u{208B}', target: vec!['-'] }, // ₋
            PairMapping { from: '\u{208C}', target: vec!['='] }, // ₌
            PairMapping { from: '\u{208D}', target: vec!['('] }, // ₍
            PairMapping { from: '\u{208E}', target: vec![')'] }, // ₎
            // Subscript letters
            PairMapping { from: '\u{2090}', target: vec!['a'] }, // ₐ
            PairMapping { from: '\u{2091}', target: vec!['e'] }, // ₑ
            PairMapping { from: '\u{2092}', target: vec!['o'] }, // ₒ
            PairMapping { from: '\u{2093}', target: vec!['x'] }, // ₓ
            PairMapping { from: '\u{2094}', target: vec!['\u{0259}'] }, // ₔ → ə (schwa)
            PairMapping { from: '\u{2095}', target: vec!['h'] }, // ₕ
            PairMapping { from: '\u{2096}', target: vec!['k'] }, // ₖ
            PairMapping { from: '\u{2097}', target: vec!['l'] }, // ₗ
            PairMapping { from: '\u{2098}', target: vec!['m'] }, // ₘ
            PairMapping { from: '\u{2099}', target: vec!['n'] }, // ₙ
            PairMapping { from: '\u{209A}', target: vec!['p'] }, // ₚ
            PairMapping { from: '\u{209B}', target: vec!['s'] }, // ₛ
            PairMapping { from: '\u{209C}', target: vec!['t'] }, // ₜ
        ],
    }
}

// ── TOML loading ────────────────────────────────────────────────────

#[cfg(feature = "toml-config")]
mod toml_loader {
    use super::*;
    use serde::Deserialize;
    use std::path::Path;

    /// Top-level TOML file structure.
    #[derive(Debug, Deserialize)]
    struct MappingFile {
        metadata: MetadataToml,
        #[serde(default)]
        ranges: Vec<RangeToml>,
        #[serde(default)]
        pairs: Vec<PairToml>,
    }

    /// `[metadata]` section.
    #[derive(Debug, Deserialize)]
    struct MetadataToml {
        #[serde(rename = "type")]
        type_name: String,
        description: String,
        direction: String,
        tags: Vec<String>,
    }

    /// `[[ranges]]` entry.
    #[derive(Debug, Deserialize)]
    struct RangeToml {
        from_start: String,
        from_end: String,
        to_start: String,
    }

    /// `[[pairs]]` entry.
    #[derive(Debug, Deserialize)]
    struct PairToml {
        from: String,
        #[serde(default)]
        to: Option<String>,
        #[serde(default)]
        to_seq: Option<String>,
    }

    /// Parse a hex codepoint string (e.g. "FF01") into a `u32`.
    fn parse_hex_cp(s: &str) -> Result<u32, String> {
        u32::from_str_radix(s.trim(), 16)
            .map_err(|e| format!("invalid hex codepoint {s:?}: {e}"))
    }

    /// Parse a hex codepoint string into a `char`.
    fn parse_hex_char(s: &str) -> Result<char, String> {
        let cp = parse_hex_cp(s)?;
        char::from_u32(cp).ok_or_else(|| format!("invalid Unicode codepoint: U+{cp:04X}"))
    }

    /// Parse a space-separated hex codepoint sequence into `Vec<char>`.
    fn parse_hex_seq(s: &str) -> Result<Vec<char>, String> {
        s.split_whitespace()
            .map(parse_hex_char)
            .collect()
    }

    fn parse_direction(s: &str) -> Result<Direction, String> {
        match s {
            "narrowing" => Ok(Direction::Narrowing),
            "widening" => Ok(Direction::Widening),
            "neutral" => Ok(Direction::Neutral),
            _ => Err(format!(
                "invalid direction {s:?}: expected narrowing, widening, or neutral"
            )),
        }
    }

    fn convert_range(r: &RangeToml) -> Result<RangeMapping, String> {
        let from_start = parse_hex_cp(&r.from_start)?;
        let from_end = parse_hex_cp(&r.from_end)?;
        let to_start = parse_hex_cp(&r.to_start)?;

        if from_end < from_start {
            return Err(format!(
                "range from_end ({:04X}) < from_start ({:04X})",
                from_end, from_start
            ));
        }

        let offset = to_start as i32 - from_start as i32;

        // Validate that all target codepoints are valid Unicode.
        let last_target = (from_end as i64 + offset as i64) as u32;
        if char::from_u32(last_target).is_none() {
            return Err(format!(
                "range target end U+{last_target:04X} is not a valid Unicode codepoint"
            ));
        }

        Ok(RangeMapping {
            from_start,
            from_end,
            offset,
        })
    }

    fn convert_pair(p: &PairToml) -> Result<PairMapping, String> {
        let from = parse_hex_char(&p.from)?;

        let target = match (&p.to, &p.to_seq) {
            (Some(to), None) => vec![parse_hex_char(to)?],
            (None, Some(seq)) => parse_hex_seq(seq)?,
            (Some(_), Some(_)) => {
                return Err(format!(
                    "pair for U+{:04X}: specify `to` or `to_seq`, not both",
                    from as u32
                ));
            }
            (None, None) => {
                return Err(format!(
                    "pair for U+{:04X}: missing `to` or `to_seq`",
                    from as u32
                ));
            }
        };

        if target.is_empty() {
            return Err(format!(
                "pair for U+{:04X}: target sequence is empty",
                from as u32
            ));
        }

        Ok(PairMapping { from, target })
    }

    /// Load a TOML mapping file and parse it into a [`CharMappingSet`].
    pub fn load_file(path: &Path) -> Result<CharMappingSet, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        load_str(&text, path.display().to_string())
    }

    /// Parse a TOML string into a [`CharMappingSet`].
    pub fn load_str(text: &str, source: String) -> Result<CharMappingSet, String> {
        let file: MappingFile = toml::from_str(text)
            .map_err(|e| format!("failed to parse TOML from {source}: {e}"))?;

        let direction = parse_direction(&file.metadata.direction)?;

        let mut ranges = Vec::with_capacity(file.ranges.len());
        for (i, r) in file.ranges.iter().enumerate() {
            ranges.push(
                convert_range(r)
                    .map_err(|e| format!("{source}: ranges[{i}]: {e}"))?,
            );
        }

        let mut pairs = Vec::with_capacity(file.pairs.len());
        for (i, p) in file.pairs.iter().enumerate() {
            pairs.push(
                convert_pair(p)
                    .map_err(|e| format!("{source}: pairs[{i}]: {e}"))?,
            );
        }

        Ok(CharMappingSet {
            type_name: file.metadata.type_name,
            description: file.metadata.description,
            direction,
            tags: file.metadata.tags,
            pairs,
            ranges,
        })
    }

    impl UnicodeMap {
        /// Load a TOML file and merge it into this map.
        ///
        /// Rejects files whose `type_name` collides with an existing set.
        pub fn load_and_merge(&mut self, path: &Path) -> Result<(), String> {
            let set = load_file(path)?;
            self.merge_set(set).map_err(|dup| {
                format!(
                    "distill-ansi: --unicode-map: rejecting duplicate type {:?} \
                     (already loaded)",
                    dup,
                )
            })
        }
    }
}

#[cfg(feature = "toml-config")]
pub use toml_loader::{load_file, load_str};

// Tests live in tests/unicode_map_unit_tests.rs and
// tests/unicode_map_toml_tests.rs.
