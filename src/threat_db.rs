//! External threat database for `--threat-db` support.
//!
//! Two-layer threat detection:
//! 1. Built-in defaults compiled into the binary (immutable).
//! 2. External TOML file loaded at startup (additive only).
//!
//! Gated behind the `toml-config` feature flag.

#![forbid(unsafe_code)]

use std::path::Path;

use serde::Deserialize;

use crate::classifier::{SeqDetail, SeqKind};

// ── ThreatMatch ─────────────────────────────────────────────────────

/// Pattern fields used to match a [`SeqDetail`] against a threat entry.
#[derive(Clone, Debug, Deserialize)]
pub struct ThreatMatch {
    /// SeqKind name: "Dcs", "Osc", "CsiQuery", etc.
    pub kind: String,
    /// OSC number to match (e.g. 50 for font query).
    pub osc_number: Option<u16>,
    /// First CSI parameter to match (e.g. 21 for title report).
    pub first_param: Option<u16>,
    /// Whether the DCS sequence is a DECRQSS query.
    pub dcs_is_query: Option<bool>,
}

// ── ThreatEntry ─────────────────────────────────────────────────────

/// A single threat pattern with metadata.
#[derive(Clone, Debug, Deserialize)]
pub struct ThreatEntry {
    /// Unique threat type name (e.g. "dcs_decrqss", "osc_50").
    #[serde(rename = "type")]
    pub type_name: String,
    /// Pattern to match against classified sequences.
    #[serde(rename = "match")]
    pub match_pattern: ThreatMatch,
    /// CVE identifier, if known.
    pub cve: Option<String>,
    /// Human-readable description.
    pub description: String,
    /// Reference URI (NVD, advisory, etc.).
    #[serde(rename = "ref")]
    pub reference: Option<String>,
    /// Resolved SeqKind — avoids string comparison in classify().
    /// Populated at construction time from match_pattern.kind.
    #[serde(skip)]
    resolved_kind: Option<SeqKind>,
}

// ── TOML file format ────────────────────────────────────────────────

/// Top-level TOML structure for the threat database file.
#[derive(Debug, Deserialize)]
struct ThreatDbFile {
    #[serde(default)]
    threats: Vec<ThreatEntry>,
}

// ── ThreatDb ────────────────────────────────────────────────────────

/// Merged threat database: built-in defaults + external entries.
///
/// Built-in entries are immutable. External entries are additive only.
/// Duplicate `type_name` keys from the external file are rejected with
/// a warning to stderr.
#[derive(Clone, Debug)]
pub struct ThreatDb {
    entries: Vec<ThreatEntry>,
}

impl ThreatDb {
    /// Create a threat database with only the compiled-in default entries.
    ///
    /// These mirror the 6 patterns from `is_threat` + `lookup_cve` in main.rs.
    #[must_use]
    pub fn builtin() -> Self {
        let mut db = Self {
            entries: builtin_entries(),
        };
        db.resolve_kinds();
        db
    }

    /// Load an external TOML threat database and merge with builtins.
    ///
    /// External entries with the same `type_name` as a built-in are
    /// rejected with a warning to stderr. New entries are added.
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let text =
            std::fs::read_to_string(path).map_err(|e| format!("failed to read threat-db: {e}"))?;

        let file: ThreatDbFile =
            toml::from_str(&text).map_err(|e| format!("failed to parse threat-db TOML: {e}"))?;

        let mut db = Self::builtin();

        for entry in file.threats {
            if db.entries.iter().any(|e| e.type_name == entry.type_name) {
                eprintln!(
                    "strip-ansi: --threat-db: rejecting duplicate type {:?} (built-in is immutable)",
                    entry.type_name,
                );
            } else {
                db.entries.push(entry);
            }
        }

        db.resolve_kinds();
        Ok(db)
    }

    /// Classify a sequence detail against the database entries.
    ///
    /// Returns the first matching [`ThreatEntry`], or `None` if no match.
    /// Matches on resolved `SeqKind` enum — no string comparison.
    #[must_use]
    pub fn classify(&self, detail: &SeqDetail) -> Option<&ThreatEntry> {
        self.entries.iter().find(|entry| {
            let m = &entry.match_pattern;

            // Kind must match — use resolved enum, not string.
            let kind_matches = match entry.resolved_kind {
                Some(k) => detail.kind == k,
                None => seq_kind_name(detail.kind) == m.kind,
            };
            if !kind_matches {
                return false;
            }

            if let Some(osc) = m.osc_number {
                if detail.osc_number != osc {
                    return false;
                }
            }
            if let Some(fp) = m.first_param {
                if detail.first_param != fp {
                    return false;
                }
            }
            if let Some(dq) = m.dcs_is_query {
                if detail.dcs_is_query != dq {
                    return false;
                }
            }

            true
        })
    }

    /// Returns the number of entries in the database.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the database has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns a slice of all entries.
    #[must_use]
    pub fn entries(&self) -> &[ThreatEntry] {
        &self.entries
    }

    /// Resolve kind strings to SeqKind enums for all entries.
    fn resolve_kinds(&mut self) {
        for entry in &mut self.entries {
            entry.resolved_kind = parse_seq_kind(&entry.match_pattern.kind);
        }
    }
}

/// Parse a SeqKind name string to the enum variant.
fn parse_seq_kind(name: &str) -> Option<SeqKind> {
    match name {
        "CsiSgr" => Some(SeqKind::CsiSgr),
        "CsiCursor" => Some(SeqKind::CsiCursor),
        "CsiErase" => Some(SeqKind::CsiErase),
        "CsiScroll" => Some(SeqKind::CsiScroll),
        "CsiMode" => Some(SeqKind::CsiMode),
        "CsiDeviceStatus" => Some(SeqKind::CsiDeviceStatus),
        "CsiWindow" => Some(SeqKind::CsiWindow),
        "CsiQuery" => Some(SeqKind::CsiQuery),
        "CsiOther" => Some(SeqKind::CsiOther),
        "Osc" => Some(SeqKind::Osc),
        "Dcs" => Some(SeqKind::Dcs),
        "Apc" => Some(SeqKind::Apc),
        "Pm" => Some(SeqKind::Pm),
        "Sos" => Some(SeqKind::Sos),
        "Ss2" => Some(SeqKind::Ss2),
        "Ss3" => Some(SeqKind::Ss3),
        "Fe" => Some(SeqKind::Fe),
        "Unknown" => Some(SeqKind::Unknown),
        _ => None,
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Map a [`SeqKind`] to the string used in TOML `kind` fields.
fn seq_kind_name(kind: crate::classifier::SeqKind) -> &'static str {
    use crate::classifier::SeqKind;
    match kind {
        SeqKind::CsiSgr => "CsiSgr",
        SeqKind::CsiCursor => "CsiCursor",
        SeqKind::CsiErase => "CsiErase",
        SeqKind::CsiScroll => "CsiScroll",
        SeqKind::CsiMode => "CsiMode",
        SeqKind::CsiDeviceStatus => "CsiDeviceStatus",
        SeqKind::CsiWindow => "CsiWindow",
        SeqKind::CsiQuery => "CsiQuery",
        SeqKind::CsiOther => "CsiOther",
        SeqKind::Osc => "Osc",
        SeqKind::Dcs => "Dcs",
        SeqKind::Apc => "Apc",
        SeqKind::Pm => "Pm",
        SeqKind::Sos => "Sos",
        SeqKind::Ss2 => "Ss2",
        SeqKind::Ss3 => "Ss3",
        SeqKind::Fe => "Fe",
        SeqKind::Unknown => "Unknown",
    }
}

/// The 6 built-in threat entries matching `is_threat` + `lookup_cve`.
fn builtin_entries() -> Vec<ThreatEntry> {
    vec![
        ThreatEntry {
            type_name: "dcs_decrqss".into(),
            match_pattern: ThreatMatch {
                kind: "Dcs".into(),
                osc_number: None,
                first_param: None,
                dcs_is_query: Some(true),
            },
            cve: Some("CVE-2008-2383".into()),
            description: "DECRQSS echoback".into(),
            reference: Some("https://nvd.nist.gov/vuln/detail/CVE-2008-2383".into()),
            resolved_kind: None,
        },
        ThreatEntry {
            type_name: "dcs_other".into(),
            match_pattern: ThreatMatch {
                kind: "Dcs".into(),
                osc_number: None,
                first_param: None,
                dcs_is_query: Some(false),
            },
            cve: None,
            description: "Other DCS sequence".into(),
            reference: None,
            resolved_kind: None,
        },
        ThreatEntry {
            type_name: "osc_50".into(),
            match_pattern: ThreatMatch {
                kind: "Osc".into(),
                osc_number: Some(50),
                first_param: None,
                dcs_is_query: None,
            },
            cve: Some("CVE-2022-45063".into()),
            description: "Font query echoback".into(),
            reference: Some("https://nvd.nist.gov/vuln/detail/CVE-2022-45063".into()),
            resolved_kind: None,
        },
        ThreatEntry {
            type_name: "osc_clipboard".into(),
            match_pattern: ThreatMatch {
                kind: "Osc".into(),
                osc_number: Some(52),
                first_param: None,
                dcs_is_query: None,
            },
            cve: None,
            description: "Clipboard access".into(),
            reference: None,
            resolved_kind: None,
        },
        ThreatEntry {
            type_name: "csi_21t".into(),
            match_pattern: ThreatMatch {
                kind: "CsiQuery".into(),
                osc_number: None,
                first_param: Some(21),
                dcs_is_query: None,
            },
            cve: Some("CVE-2003-0063".into()),
            description: "Title report echoback".into(),
            reference: Some("https://nvd.nist.gov/vuln/detail/CVE-2003-0063".into()),
            resolved_kind: None,
        },
        ThreatEntry {
            type_name: "csi_6n".into(),
            match_pattern: ThreatMatch {
                kind: "CsiQuery".into(),
                osc_number: None,
                first_param: Some(6),
                dcs_is_query: None,
            },
            cve: None,
            description: "Cursor position report".into(),
            reference: None,
            resolved_kind: None,
        },
    ]
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classifier::{OscType, SeqKind, SgrContent};

    fn make_detail(kind: SeqKind) -> SeqDetail {
        SeqDetail {
            kind,
            sgr_content: SgrContent::empty(),
            osc_type: OscType::Unknown,
            osc_number: 0,
            first_param: 0,
            dcs_is_query: false,
        }
    }

    // ── 11.5.10: builtin DB matches all known patterns ──────────

    #[test]
    fn builtin_matches_dcs_decrqss() {
        let db = ThreatDb::builtin();
        let mut d = make_detail(SeqKind::Dcs);
        d.dcs_is_query = true;
        let entry = db.classify(&d).expect("should match dcs_decrqss");
        assert_eq!(entry.type_name, "dcs_decrqss");
        assert_eq!(entry.cve.as_deref(), Some("CVE-2008-2383"));
    }

    #[test]
    fn builtin_matches_dcs_other() {
        let db = ThreatDb::builtin();
        let d = make_detail(SeqKind::Dcs);
        let entry = db.classify(&d).expect("should match dcs_other");
        assert_eq!(entry.type_name, "dcs_other");
        assert!(entry.cve.is_none());
    }

    #[test]
    fn builtin_matches_osc_50() {
        let db = ThreatDb::builtin();
        let mut d = make_detail(SeqKind::Osc);
        d.osc_number = 50;
        let entry = db.classify(&d).expect("should match osc_50");
        assert_eq!(entry.type_name, "osc_50");
        assert_eq!(entry.cve.as_deref(), Some("CVE-2022-45063"));
    }

    #[test]
    fn builtin_matches_osc_clipboard() {
        let db = ThreatDb::builtin();
        let mut d = make_detail(SeqKind::Osc);
        d.osc_number = 52;
        d.osc_type = OscType::Clipboard;
        let entry = db.classify(&d).expect("should match osc_clipboard");
        assert_eq!(entry.type_name, "osc_clipboard");
    }

    #[test]
    fn builtin_matches_csi_21t() {
        let db = ThreatDb::builtin();
        let mut d = make_detail(SeqKind::CsiQuery);
        d.first_param = 21;
        let entry = db.classify(&d).expect("should match csi_21t");
        assert_eq!(entry.type_name, "csi_21t");
        assert_eq!(entry.cve.as_deref(), Some("CVE-2003-0063"));
    }

    #[test]
    fn builtin_matches_csi_6n() {
        let db = ThreatDb::builtin();
        let mut d = make_detail(SeqKind::CsiQuery);
        d.first_param = 6;
        let entry = db.classify(&d).expect("should match csi_6n");
        assert_eq!(entry.type_name, "csi_6n");
    }

    #[test]
    fn builtin_no_match_for_sgr() {
        let db = ThreatDb::builtin();
        let d = make_detail(SeqKind::CsiSgr);
        assert!(db.classify(&d).is_none());
    }

    #[test]
    fn builtin_has_6_entries() {
        let db = ThreatDb::builtin();
        assert_eq!(db.len(), 6);
    }

    // ── 11.5.11: duplicate type key → rejected with warning ─────

    #[test]
    fn external_duplicate_type_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("threat-db.toml");
        std::fs::write(
            &path,
            r#"
[[threats]]
type = "osc_50"
description = "Trying to override builtin"

[threats.match]
kind = "Osc"
osc_number = 50
"#,
        )
        .unwrap();

        let db = ThreatDb::from_file(&path).unwrap();
        // Should still have exactly 6 entries (duplicate rejected).
        assert_eq!(db.len(), 6);
    }

    // ── 11.5.12: external DB adds new entry ─────────────────────

    #[test]
    fn external_adds_new_entry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("threat-db.toml");
        std::fs::write(
            &path,
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

        let db = ThreatDb::from_file(&path).unwrap();
        assert_eq!(db.len(), 7); // 6 builtins + 1 new

        // Verify the new entry matches.
        let mut d = make_detail(SeqKind::Osc);
        d.osc_number = 999;
        let entry = db.classify(&d).expect("should match custom_osc_999");
        assert_eq!(entry.type_name, "custom_osc_999");
        assert_eq!(entry.cve.as_deref(), Some("CVE-2024-99999"));
        assert_eq!(
            entry.reference.as_deref(),
            Some("https://example.com/advisory")
        );
    }

    #[test]
    fn external_empty_file_returns_builtins() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("threat-db.toml");
        std::fs::write(&path, "").unwrap();

        let db = ThreatDb::from_file(&path).unwrap();
        assert_eq!(db.len(), 6);
    }

    #[test]
    fn external_nonexistent_file_returns_error() {
        let result = ThreatDb::from_file(Path::new("/nonexistent/threat-db.toml"));
        assert!(result.is_err());
    }
}
