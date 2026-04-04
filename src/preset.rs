//! Terminal capability presets for [`FilterConfig`] construction.
//!
//! Each [`TerminalPreset`] maps to a [`FilterConfig`] that preserves
//! the escape sequences a given class of terminal can handle.
//! Presets form a gradient from `Dumb` (strip everything) to `Full`
//! (preserve everything).
//!
//! Two naming conventions coexist:
//!
//! - **Standard presets** use terminal-standard names (`dumb`, `vt100`,
//!   `xterm`, `full`) for users who think in terminal capabilities.
//! - **Aliases** use use-case names (`pipe`, `ci`, `pager`, `modern`)
//!   for users who think in terms of what they're doing.
//!
//! Both resolve to the same [`FilterConfig`].

#![forbid(unsafe_code)]

use crate::classifier::{OscType, SeqGroup, SeqKind};
use crate::filter::FilterConfig;

/// Named terminal capability presets.
///
/// Each variant maps to a [`FilterConfig`] via
/// [`to_filter_config()`](Self::to_filter_config).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TerminalPreset {
    /// Strip all escape sequences.
    ///
    /// Use: pipes, files, `TERM=dumb`, non-TTY output.
    /// Alias: `pipe`.
    Dumb,

    /// Preserve SGR (colors and text styles) only.
    ///
    /// Use: `less -R`, CI logs, basic color-aware pagers.
    /// Aliases: `ci`, `pager`.
    Color,

    /// Preserve SGR, cursor movement, and erase sequences.
    ///
    /// Use: classic terminals, serial consoles, simple TUIs.
    Vt100,

    /// Preserve all CSI sequences and Fe escapes, but not OSC.
    ///
    /// Use: terminal multiplexers (`tmux`, `screen`) that handle
    /// CSI but filter most OSC sequences.
    Tmux,

    /// Preserve safe CSI, safe OSC, and Fe sequences while stripping
    /// all known echoback vectors.
    ///
    /// Strips: CsiQuery, CsiDeviceStatus, OscClipboard, OscOther,
    /// Dcs, Apc, Pm, Sos, Ss2, Ss3.
    ///
    /// This is the auto-detect ceiling — `detect_preset()` never
    /// returns a preset above `Sanitize`.
    /// Alias: `safe`.
    Sanitize,

    /// Preserve all CSI, OSC, and Fe sequences.
    ///
    /// Use: modern terminal emulators with window title, hyperlink,
    /// and notification support.
    Xterm,

    /// Preserve all escape sequences (strip nothing).
    ///
    /// Use: fully capable terminals (iTerm2, Kitty, WezTerm, foot).
    /// Alias: `modern`.
    Full,
}

impl TerminalPreset {
    /// Convert this preset to a [`FilterConfig`].
    #[must_use]
    pub fn to_filter_config(self) -> FilterConfig {
        match self {
            Self::Dumb => FilterConfig::strip_all(),

            Self::Color => FilterConfig::strip_all()
                .no_strip_kind(SeqKind::CsiSgr),

            Self::Vt100 => FilterConfig::strip_all()
                .no_strip_kind(SeqKind::CsiSgr)
                .no_strip_kind(SeqKind::CsiCursor)
                .no_strip_kind(SeqKind::CsiErase),

            Self::Tmux => FilterConfig::strip_all()
                .no_strip_group(SeqGroup::Csi)
                .no_strip_group(SeqGroup::Fe),

            Self::Sanitize => FilterConfig::strip_all()
                .no_strip_kind(SeqKind::CsiSgr)
                .no_strip_kind(SeqKind::CsiCursor)
                .no_strip_kind(SeqKind::CsiErase)
                .no_strip_kind(SeqKind::CsiScroll)
                .no_strip_kind(SeqKind::CsiMode)
                .no_strip_kind(SeqKind::CsiWindow)
                .no_strip_kind(SeqKind::CsiOther)
                .no_strip_group(SeqGroup::Fe)
                .no_strip_osc_type(OscType::Title)
                .no_strip_osc_type(OscType::Hyperlink)
                .no_strip_osc_type(OscType::Notify)
                .no_strip_osc_type(OscType::WorkingDir)
                .no_strip_osc_type(OscType::ShellInteg),

            Self::Xterm => FilterConfig::strip_all()
                .no_strip_group(SeqGroup::Csi)
                .no_strip_group(SeqGroup::Osc)
                .no_strip_group(SeqGroup::Fe),

            Self::Full => FilterConfig::pass_all(),
        }
    }

    /// Parse a preset name (case-insensitive).
    ///
    /// Accepts both standard names and use-case aliases.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            // Standard presets
            "dumb" => Some(Self::Dumb),
            "color" => Some(Self::Color),
            "vt100" => Some(Self::Vt100),
            "tmux" => Some(Self::Tmux),
            "sanitize" => Some(Self::Sanitize),
            "xterm" => Some(Self::Xterm),
            "full" => Some(Self::Full),

            // Use-case aliases
            "pipe" => Some(Self::Dumb),
            "ci" => Some(Self::Color),
            "pager" => Some(Self::Color),
            "modern" => Some(Self::Full),
            "screen" => Some(Self::Tmux),
            "safe" => Some(Self::Sanitize),

            _ => None,
        }
    }

    /// The canonical name for this preset.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dumb => "dumb",
            Self::Color => "color",
            Self::Vt100 => "vt100",
            Self::Tmux => "tmux",
            Self::Sanitize => "sanitize",
            Self::Xterm => "xterm",
            Self::Full => "full",
        }
    }

    /// Returns `true` for presets above `sanitize` that require `--unsafe`.
    #[must_use]
    pub const fn requires_unsafe(self) -> bool {
        matches!(self, Self::Xterm | Self::Full)
    }

    /// All valid preset names including aliases, for help text.
    pub const ALL_NAMES: &[&str] = &[
        "dumb", "pipe",
        "color", "ci", "pager",
        "vt100",
        "tmux", "screen",
        "sanitize", "safe",
        "xterm",
        "full", "modern",
    ];
}
