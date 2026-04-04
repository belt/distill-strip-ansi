//! Terminal capability auto-detection.
//!
//! Probes stdout to determine what ANSI escape sequences the output
//! terminal can handle, returning the appropriate [`TerminalPreset`].
//!
//! Detection order:
//! 1. Not a TTY → [`Dumb`](TerminalPreset::Dumb)
//! 2. `TERM=dumb` → [`Dumb`](TerminalPreset::Dumb)
//! 3. `NO_COLOR` set → [`Dumb`](TerminalPreset::Dumb)
//! 4. Color detected → [`Sanitize`](TerminalPreset::Sanitize)
//!
//! Auto-detect always caps at `Sanitize`. The color level from
//! `supports-color` refines the SGR mask via [`detect_sgr_mask`].
//! `Xterm` and `Full` require `--unsafe`.
//!
//! Gated behind the `terminal-detect` feature flag.

#![forbid(unsafe_code)]

use std::io::IsTerminal;

use crate::classifier::SgrContent;
use crate::preset::TerminalPreset;

/// Detect the appropriate [`TerminalPreset`] for stdout.
///
/// Always caps at [`Sanitize`](TerminalPreset::Sanitize) — never
/// returns `Xterm` or `Full`. Use [`detect_sgr_mask`] to get the
/// SGR content mask based on color level.
#[must_use]
pub fn detect_preset() -> TerminalPreset {
    // Not a TTY → strip everything.
    if !std::io::stdout().is_terminal() {
        return TerminalPreset::Dumb;
    }

    // TERM=dumb → strip everything.
    if is_term_dumb() {
        return TerminalPreset::Dumb;
    }

    // NO_COLOR spec compliance (https://no-color.org).
    if std::env::var_os("NO_COLOR").is_some() {
        return TerminalPreset::Dumb;
    }

    // Probe color support.
    let has_color = supports_color::on_cached(supports_color::Stream::Stdout).is_some();
    if !has_color {
        return TerminalPreset::Dumb;
    }

    // Cap at Sanitize — never return Xterm or Full.
    TerminalPreset::Sanitize
}

/// Detect the SGR content mask based on `supports-color` level.
///
/// Returns the appropriate [`SgrContent`] mask for the detected
/// color level. Returns `None` when no color support is detected
/// (caller should use `FilterConfig::strip_all()`).
#[must_use]
pub fn detect_sgr_mask() -> Option<SgrContent> {
    let level = supports_color::on_cached(supports_color::Stream::Stdout)?;

    let mut mask = SgrContent::BASIC;
    if level.has_256 {
        mask = mask.union(SgrContent::EXTENDED);
    }
    if level.has_16m {
        mask = mask.union(SgrContent::TRUECOLOR);
    }
    Some(mask)
}

/// Returns `true` if `TERM` is literally `"dumb"`.
fn is_term_dumb() -> bool {
    std::env::var("TERM")
        .map(|v| v == "dumb")
        .unwrap_or(false)
}

/// Detect the appropriate [`TerminalPreset`] for stdout, ignoring
/// attacker-controllable environment variables.
///
/// Like [`detect_preset`], but treats the following env vars as
/// absent (attacker can set them):
/// - `FORCE_COLOR`, `FORCE_HYPERLINK`, `COLORTERM`
/// - `TERM_PROGRAM`, `TERM_PROGRAM_VERSION`, `VTE_VERSION`
///
/// Trusted signals: `isatty(stdout)`, `TERM`.
///
/// Fallback from `TERM` alone:
/// - `"256color"` suffix → `BASIC | EXTENDED`
/// - `"color"` suffix → `BASIC`
/// - `"dumb"` → strip all
/// - else → `BASIC` (conservative)
///
/// Always caps at [`Sanitize`](TerminalPreset::Sanitize).
#[must_use]
pub fn detect_preset_untrusted() -> TerminalPreset {
    // Not a TTY → strip everything.
    if !std::io::stdout().is_terminal() {
        return TerminalPreset::Dumb;
    }

    // TERM=dumb → strip everything.
    if is_term_dumb() {
        return TerminalPreset::Dumb;
    }

    // NO_COLOR spec compliance (https://no-color.org).
    if std::env::var_os("NO_COLOR").is_some() {
        return TerminalPreset::Dumb;
    }

    // Ignore attacker-controllable env vars entirely.
    // FORCE_COLOR, FORCE_HYPERLINK, COLORTERM, TERM_PROGRAM,
    // TERM_PROGRAM_VERSION, VTE_VERSION are all untrusted.
    //
    // Only trust: isatty(stdout) [checked above] and TERM.
    let has_color = match std::env::var("TERM") {
        Ok(term) if !term.is_empty() && term != "dumb" => true,
        _ => false,
    };

    if !has_color {
        return TerminalPreset::Dumb;
    }

    // Cap at Sanitize.
    TerminalPreset::Sanitize
}

/// Detect the SGR content mask from `TERM` alone (untrusted mode).
///
/// Ignores `COLORTERM`, `TERM_PROGRAM`, `VTE_VERSION`, and other
/// attacker-controllable env vars. Falls back to conservative
/// heuristics based on the `TERM` value.
#[must_use]
pub fn detect_sgr_mask_untrusted() -> Option<SgrContent> {
    let term = std::env::var("TERM").ok()?;
    if term.is_empty() || term == "dumb" {
        return None;
    }

    if term.ends_with("256color") {
        Some(SgrContent::BASIC.union(SgrContent::EXTENDED))
    } else if term.ends_with("color") {
        Some(SgrContent::BASIC)
    } else {
        // Conservative: assume basic color support.
        Some(SgrContent::BASIC)
    }
}
