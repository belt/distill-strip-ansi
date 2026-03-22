//! TOML configuration loading for `distill-strip-ansi`.
//!
//! Deserializes a TOML file into [`StripAnsiConfig`], which contains
//! general settings ([`GeneralConfig`]) and a filter specification
//! ([`FilterToml`]). The filter specification converts to a
//! [`FilterConfig`] with validation.
//!
//! Gated behind the `toml-config` feature flag.

#![forbid(unsafe_code)]

use std::path::Path;

use serde::Deserialize;
use thiserror::Error;

use crate::classifier::{SeqGroup, SeqKind};
use crate::filter::FilterConfig;

// ── Errors ──────────────────────────────────────────────────────────

/// Errors that can occur when loading or validating configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// File I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// TOML parse error.
    #[error("TOML parse error: {0}")]
    Parse(String),

    /// Validation error (unknown filter names, invalid values).
    #[error("validation error: {0}")]
    Validation(String),
}

// ── Config structs ──────────────────────────────────────────────────

/// Top-level TOML configuration.
#[derive(Clone, Debug, Deserialize)]
pub struct StripAnsiConfig {
    /// General settings.
    #[serde(default)]
    pub general: GeneralConfig,

    /// Filter specification.
    #[serde(default)]
    pub filter: FilterToml,
}

/// General configuration settings.
#[derive(Clone, Debug, Deserialize)]
pub struct GeneralConfig {
    /// Buffer size in bytes. Default: 32768. Valid range: 1024..=16_777_216.
    #[serde(default = "default_buffer_size")]
    pub buffer_size: usize,

    /// Operating mode: `"strip"` or `"check"`.
    #[serde(default)]
    pub mode: Option<String>,
}

fn default_buffer_size() -> usize {
    32_768
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            buffer_size: default_buffer_size(),
            mode: None,
        }
    }
}

/// Filter section of the TOML configuration.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct FilterToml {
    /// Names of groups or sub-kinds to preserve (not strip).
    #[serde(default)]
    pub no_strip: Vec<String>,
}

// ── Parsing ─────────────────────────────────────────────────────────

impl StripAnsiConfig {
    /// Read and parse a TOML configuration file.
    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path)?;
        Self::from_str(&text)
    }

    /// Parse a TOML configuration string.
    pub fn from_str(text: &str) -> Result<Self, ConfigError> {
        toml::from_str(text).map_err(|e| ConfigError::Parse(e.to_string()))
    }

    /// Convert the parsed configuration into a validated [`FilterConfig`].
    ///
    /// Validates:
    /// - `buffer_size` is in range 1024..=16_777_216
    /// - All names in `no_strip` are known group or sub-kind names
    /// - Duplicates are silently ignored
    /// - Empty `no_strip` produces `FilterConfig::strip_all()`
    pub fn to_filter_config(&self) -> Result<FilterConfig, ConfigError> {
        // Validate buffer_size.
        const MIN_BUFFER: usize = 1024;
        const MAX_BUFFER: usize = 16_777_216; // 16 MiB
        if self.general.buffer_size < MIN_BUFFER || self.general.buffer_size > MAX_BUFFER {
            return Err(ConfigError::Validation(format!(
                "buffer_size {} is out of range ({MIN_BUFFER}..={MAX_BUFFER})",
                self.general.buffer_size,
            )));
        }

        // Empty no_strip → strip all.
        if self.filter.no_strip.is_empty() {
            return Ok(FilterConfig::strip_all());
        }

        let mut config = FilterConfig::strip_all();
        for name in &self.filter.no_strip {
            match parse_filter_name(name)? {
                FilterName::Group(g) => {
                    config = config.no_strip_group(g);
                }
                FilterName::Kind(k) => {
                    config = config.no_strip_kind(k);
                }
            }
        }

        Ok(config)
    }
}


// ── Name resolution ─────────────────────────────────────────────────

/// Resolved filter name: either a group or a specific sub-kind.
enum FilterName {
    Group(SeqGroup),
    Kind(SeqKind),
}

/// Parse a snake_case filter name into a group or sub-kind.
fn parse_filter_name(name: &str) -> Result<FilterName, ConfigError> {
    match name {
        // Group names
        "csi" => Ok(FilterName::Group(SeqGroup::Csi)),
        "osc" => Ok(FilterName::Group(SeqGroup::Osc)),
        "dcs" => Ok(FilterName::Group(SeqGroup::Dcs)),
        "apc" => Ok(FilterName::Group(SeqGroup::Apc)),
        "pm" => Ok(FilterName::Group(SeqGroup::Pm)),
        "sos" => Ok(FilterName::Group(SeqGroup::Sos)),
        "ss2" => Ok(FilterName::Group(SeqGroup::Ss2)),
        "ss3" => Ok(FilterName::Group(SeqGroup::Ss3)),
        "fe" => Ok(FilterName::Group(SeqGroup::Fe)),

        // CSI sub-kind names
        "csi_sgr" => Ok(FilterName::Kind(SeqKind::CsiSgr)),
        "csi_cursor" => Ok(FilterName::Kind(SeqKind::CsiCursor)),
        "csi_erase" => Ok(FilterName::Kind(SeqKind::CsiErase)),
        "csi_scroll" => Ok(FilterName::Kind(SeqKind::CsiScroll)),
        "csi_mode" => Ok(FilterName::Kind(SeqKind::CsiMode)),
        "csi_device_status" => Ok(FilterName::Kind(SeqKind::CsiDeviceStatus)),
        "csi_window" => Ok(FilterName::Kind(SeqKind::CsiWindow)),
        "csi_other" => Ok(FilterName::Kind(SeqKind::CsiOther)),

        _ => Err(ConfigError::Validation(format!(
            "unknown filter name: {name:?}"
        ))),
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_toml_produces_strip_all() {
        let config = StripAnsiConfig::from_str("").unwrap();
        let fc = config.to_filter_config().unwrap();
        assert!(fc.is_strip_all());
    }

    #[test]
    fn empty_no_strip_produces_strip_all() {
        let config = StripAnsiConfig::from_str("[filter]\nno_strip = []\n").unwrap();
        let fc = config.to_filter_config().unwrap();
        assert!(fc.is_strip_all());
    }

    #[test]
    fn preserves_group_by_name() {
        let toml = r#"
[filter]
no_strip = ["osc"]
"#;
        let config = StripAnsiConfig::from_str(toml).unwrap();
        let fc = config.to_filter_config().unwrap();
        assert!(!fc.should_strip(SeqKind::Osc));
        assert!(fc.should_strip(SeqKind::CsiSgr));
    }

    #[test]
    fn preserves_sub_kind_by_name() {
        let toml = r#"
[filter]
no_strip = ["csi_sgr", "osc"]
"#;
        let config = StripAnsiConfig::from_str(toml).unwrap();
        let fc = config.to_filter_config().unwrap();
        assert!(!fc.should_strip(SeqKind::CsiSgr));
        assert!(!fc.should_strip(SeqKind::Osc));
        assert!(fc.should_strip(SeqKind::Dcs));
    }

    #[test]
    fn unknown_name_returns_validation_error() {
        let toml = r#"
[filter]
no_strip = ["bogus"]
"#;
        let config = StripAnsiConfig::from_str(toml).unwrap();
        let err = config.to_filter_config().unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));
        assert!(err.to_string().contains("bogus"));
    }

    #[test]
    fn duplicates_silently_ignored() {
        let toml = r#"
[filter]
no_strip = ["osc", "osc", "osc"]
"#;
        let config = StripAnsiConfig::from_str(toml).unwrap();
        let fc = config.to_filter_config().unwrap();
        assert!(!fc.should_strip(SeqKind::Osc));
    }

    #[test]
    fn buffer_size_default() {
        let config = StripAnsiConfig::from_str("").unwrap();
        assert_eq!(config.general.buffer_size, 32_768);
    }

    #[test]
    fn buffer_size_custom() {
        let toml = "[general]\nbuffer_size = 65536\n";
        let config = StripAnsiConfig::from_str(toml).unwrap();
        assert_eq!(config.general.buffer_size, 65_536);
    }

    #[test]
    fn buffer_size_too_small() {
        let toml = "[general]\nbuffer_size = 512\n";
        let config = StripAnsiConfig::from_str(toml).unwrap();
        let err = config.to_filter_config().unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));
        assert!(err.to_string().contains("512"));
    }

    #[test]
    fn buffer_size_too_large() {
        let toml = "[general]\nbuffer_size = 33554432\n"; // 32 MiB
        let config = StripAnsiConfig::from_str(toml).unwrap();
        let err = config.to_filter_config().unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));
    }

    #[test]
    fn buffer_size_at_min_boundary() {
        let toml = "[general]\nbuffer_size = 1024\n";
        let config = StripAnsiConfig::from_str(toml).unwrap();
        assert!(config.to_filter_config().is_ok());
    }

    #[test]
    fn buffer_size_at_max_boundary() {
        let toml = "[general]\nbuffer_size = 16777216\n";
        let config = StripAnsiConfig::from_str(toml).unwrap();
        assert!(config.to_filter_config().is_ok());
    }

    #[test]
    fn invalid_toml_returns_parse_error() {
        let err = StripAnsiConfig::from_str("not valid toml [[[").unwrap_err();
        assert!(matches!(err, ConfigError::Parse(_)));
    }

    #[test]
    fn mode_field_parsed() {
        let toml = r#"
[general]
mode = "strip"
"#;
        let config = StripAnsiConfig::from_str(toml).unwrap();
        assert_eq!(config.general.mode.as_deref(), Some("strip"));
    }

    #[test]
    fn full_example_toml() {
        let toml = r#"
[general]
buffer_size = 65536
mode = "strip"

[filter]
no_strip = ["csi_sgr", "osc"]
"#;
        let config = StripAnsiConfig::from_str(toml).unwrap();
        assert_eq!(config.general.buffer_size, 65_536);
        assert_eq!(config.general.mode.as_deref(), Some("strip"));
        let fc = config.to_filter_config().unwrap();
        assert!(!fc.should_strip(SeqKind::CsiSgr));
        assert!(!fc.should_strip(SeqKind::Osc));
        // CsiCursor is also preserved because no_strip_kind(CsiSgr)
        // sets the CSI group bit, preserving all CSI sub-kinds.
        assert!(!fc.should_strip(SeqKind::CsiCursor));
        assert!(fc.should_strip(SeqKind::Dcs));
    }

    #[test]
    fn from_file_nonexistent_returns_io_error() {
        let err = StripAnsiConfig::from_file(Path::new("/nonexistent/path.toml")).unwrap_err();
        assert!(matches!(err, ConfigError::Io(_)));
    }

    #[test]
    fn all_group_names_valid() {
        for name in &["csi", "osc", "dcs", "apc", "pm", "sos", "ss2", "ss3", "fe"] {
            assert!(
                parse_filter_name(name).is_ok(),
                "group name {name:?} should be valid"
            );
        }
    }

    #[test]
    fn all_sub_kind_names_valid() {
        for name in &[
            "csi_sgr",
            "csi_cursor",
            "csi_erase",
            "csi_scroll",
            "csi_mode",
            "csi_device_status",
            "csi_window",
            "csi_other",
        ] {
            assert!(
                parse_filter_name(name).is_ok(),
                "sub-kind name {name:?} should be valid"
            );
        }
    }
}
