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

use crate::classifier::{OscType, SeqGroup, SeqKind, SgrContent};
use crate::filter::FilterConfig;
use crate::preset::TerminalPreset;

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

    /// Allow presets above sanitize (xterm, full). Default: false.
    #[serde(default, rename = "unsafe")]
    pub unsafe_mode: bool,
}

fn default_buffer_size() -> usize {
    32_768
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            buffer_size: default_buffer_size(),
            mode: None,
            unsafe_mode: false,
        }
    }
}

/// Filter section of the TOML configuration.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct FilterToml {
    /// Names of groups or sub-kinds to preserve (not strip).
    #[serde(default)]
    pub no_strip: Vec<String>,

    /// Preset name (overrides no_strip when present).
    #[serde(default)]
    pub preset: Option<String>,

    /// SGR color depth: `"16"`, `"256"`, `"truecolor"`, or `"all"`.
    /// Maps to an [`SgrContent`] preserve mask.
    #[serde(default)]
    pub sgr_depth: Option<String>,
}

// ── Parsing ─────────────────────────────────────────────────────────

impl std::str::FromStr for StripAnsiConfig {
    type Err = ConfigError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        toml::from_str(text).map_err(|e| ConfigError::Parse(e.to_string()))
    }
}

impl StripAnsiConfig {
    /// Read and parse a TOML configuration file.
    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path)?;
        text.parse()
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

        // If a preset is specified, resolve it and validate the unsafe gate.
        if let Some(ref name) = self.filter.preset {
            let preset = TerminalPreset::from_name(name)
                .ok_or_else(|| ConfigError::Validation(format!("unknown preset: {name:?}")))?;
            if preset.requires_unsafe() && !self.general.unsafe_mode {
                return Err(ConfigError::Validation(format!(
                    "--preset {} preserves dangerous sequences (OSC 50, CSI 21t). \
                     Set unsafe = true in [general] to acknowledge the risk.",
                    preset.name(),
                )));
            }
            return Ok(preset.to_filter_config());
        }

        // Empty no_strip → strip all.
        if self.filter.no_strip.is_empty() && self.filter.sgr_depth.is_none() {
            return Ok(FilterConfig::strip_all());
        }

        let mut config = FilterConfig::strip_all();

        // Apply sgr_depth mask if specified.
        if let Some(ref depth) = self.filter.sgr_depth {
            let mask = parse_sgr_depth(depth)?;
            config = config.with_sgr_mask(mask);
        }

        for name in &self.filter.no_strip {
            match parse_filter_name(name)? {
                FilterName::Group(g) => {
                    config = config.no_strip_group(g);
                }
                FilterName::Kind(k) => {
                    config = config.no_strip_kind(k);
                }
                FilterName::OscType(t) => {
                    config = config.no_strip_osc_type(t);
                }
            }
        }

        Ok(config)
    }
}

// ── Name resolution ─────────────────────────────────────────────────

/// Resolved filter name: group, sub-kind, or OSC type.
enum FilterName {
    Group(SeqGroup),
    Kind(SeqKind),
    OscType(OscType),
}

/// Parse a snake_case filter name into a group, sub-kind, or OSC type.
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

        // OSC sub-type names
        "osc_title" => Ok(FilterName::OscType(OscType::Title)),
        "osc_hyperlink" => Ok(FilterName::OscType(OscType::Hyperlink)),
        "osc_clipboard" => Ok(FilterName::OscType(OscType::Clipboard)),
        "osc_notify" => Ok(FilterName::OscType(OscType::Notify)),
        "osc_shell_integration" => Ok(FilterName::OscType(OscType::ShellInteg)),
        "osc_other" => Ok(FilterName::OscType(OscType::Other)),

        _ => Err(ConfigError::Validation(format!(
            "unknown filter name: {name:?}"
        ))),
    }
}

/// Parse an `sgr_depth` string into an [`SgrContent`] mask.
fn parse_sgr_depth(depth: &str) -> Result<SgrContent, ConfigError> {
    match depth {
        "16" => Ok(SgrContent::BASIC),
        "256" => Ok(SgrContent::BASIC | SgrContent::EXTENDED),
        "truecolor" => Ok(SgrContent::BASIC | SgrContent::EXTENDED | SgrContent::TRUECOLOR),
        "all" => Ok(SgrContent::BASIC | SgrContent::EXTENDED | SgrContent::TRUECOLOR),
        _ => Err(ConfigError::Validation(format!(
            "unknown sgr_depth: {depth:?} (expected \"16\", \"256\", \"truecolor\", or \"all\")"
        ))),
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

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
        // no_strip_kind(CsiSgr) preserves only SGR, not all CSI.
        assert!(fc.should_strip(SeqKind::CsiCursor));
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

    #[test]
    fn all_osc_type_names_valid() {
        for name in &[
            "osc_title",
            "osc_hyperlink",
            "osc_clipboard",
            "osc_notify",
            "osc_shell_integration",
            "osc_other",
        ] {
            assert!(
                parse_filter_name(name).is_ok(),
                "OSC type name {name:?} should be valid"
            );
        }
    }

    #[test]
    fn osc_title_in_no_strip_preserves_osc_title() {
        let toml = r#"
[filter]
no_strip = ["osc_title", "osc_hyperlink"]
"#;
        let config = StripAnsiConfig::from_str(toml).unwrap();
        let fc = config.to_filter_config().unwrap();
        // OSC types are preserved via osc_preserve, not group-level.
        // The filter should not strip-all since we have osc types.
        assert!(!fc.is_strip_all());
    }

    #[test]
    fn sgr_depth_16() {
        let toml = r#"
[filter]
no_strip = ["csi_sgr"]
sgr_depth = "16"
"#;
        let config = StripAnsiConfig::from_str(toml).unwrap();
        let fc = config.to_filter_config().unwrap();
        assert!(!fc.is_strip_all());
    }

    #[test]
    fn sgr_depth_256() {
        let toml = r#"
[filter]
no_strip = ["csi_sgr"]
sgr_depth = "256"
"#;
        let config = StripAnsiConfig::from_str(toml).unwrap();
        let fc = config.to_filter_config().unwrap();
        assert!(!fc.is_strip_all());
    }

    #[test]
    fn sgr_depth_truecolor() {
        let toml = r#"
[filter]
no_strip = ["csi_sgr"]
sgr_depth = "truecolor"
"#;
        let config = StripAnsiConfig::from_str(toml).unwrap();
        let fc = config.to_filter_config().unwrap();
        assert!(!fc.is_strip_all());
    }

    #[test]
    fn sgr_depth_all() {
        let toml = r#"
[filter]
no_strip = ["csi_sgr"]
sgr_depth = "all"
"#;
        let config = StripAnsiConfig::from_str(toml).unwrap();
        let fc = config.to_filter_config().unwrap();
        assert!(!fc.is_strip_all());
    }

    #[test]
    fn sgr_depth_invalid() {
        let toml = r#"
[filter]
sgr_depth = "bogus"
"#;
        let config = StripAnsiConfig::from_str(toml).unwrap();
        let err = config.to_filter_config().unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));
        assert!(err.to_string().contains("bogus"));
    }

    #[test]
    fn preset_sanitize() {
        let toml = r#"
[filter]
preset = "sanitize"
"#;
        let config = StripAnsiConfig::from_str(toml).unwrap();
        let fc = config.to_filter_config().unwrap();
        assert!(!fc.should_strip(SeqKind::CsiSgr));
        assert!(fc.should_strip(SeqKind::Dcs));
    }

    #[test]
    fn preset_xterm_without_unsafe_errors() {
        let toml = r#"
[filter]
preset = "xterm"
"#;
        let config = StripAnsiConfig::from_str(toml).unwrap();
        let err = config.to_filter_config().unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));
        assert!(err.to_string().contains("unsafe"));
    }

    #[test]
    fn preset_xterm_with_unsafe_ok() {
        let toml = r#"
[general]
unsafe = true

[filter]
preset = "xterm"
"#;
        let config = StripAnsiConfig::from_str(toml).unwrap();
        let fc = config.to_filter_config().unwrap();
        assert!(!fc.should_strip(SeqKind::CsiSgr));
        assert!(!fc.should_strip(SeqKind::Osc));
    }

    #[test]
    fn preset_unknown_errors() {
        let toml = r#"
[filter]
preset = "bogus"
"#;
        let config = StripAnsiConfig::from_str(toml).unwrap();
        let err = config.to_filter_config().unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));
        assert!(err.to_string().contains("bogus"));
    }

    #[test]
    fn preset_overrides_no_strip() {
        // When preset is set, no_strip is ignored.
        let toml = r#"
[filter]
preset = "dumb"
no_strip = ["csi_sgr", "osc"]
"#;
        let config = StripAnsiConfig::from_str(toml).unwrap();
        let fc = config.to_filter_config().unwrap();
        assert!(fc.is_strip_all());
    }

    #[test]
    fn backward_compat_no_sgr_depth_no_preset() {
        // Existing configs without sgr_depth/preset still work.
        let toml = r#"
[filter]
no_strip = ["csi_sgr"]
"#;
        let config = StripAnsiConfig::from_str(toml).unwrap();
        let fc = config.to_filter_config().unwrap();
        assert!(!fc.should_strip(SeqKind::CsiSgr));
        assert!(fc.should_strip(SeqKind::Dcs));
    }
}
