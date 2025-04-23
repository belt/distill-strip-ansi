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
    std::env::var("TERM").is_ok_and(|v| v == "dumb")
}

/// Shared untrusted-TERM classifier.
///
/// Single source of truth for the "is TERM non-empty and non-dumb?"
/// predicate. Both [`detect_preset_untrusted`] and
/// [`detect_sgr_mask_untrusted`] route through here so their policies
/// cannot drift. If one grows stricter (e.g. require a `color` suffix),
/// the other must move in lockstep.
///
/// Returns `true` when `TERM` is set to something other than empty
/// or `"dumb"`. Trusted signal: TERM is the only env var a caller
/// can't trivially spoof from an unprivileged context that also
/// matters for attack-surface decisions.
fn untrusted_term_has_color() -> bool {
    matches!(std::env::var("TERM"), Ok(term) if !term.is_empty() && term != "dumb")
}

/// Detect the appropriate [`TerminalPreset`] for stdout, ignoring
/// attacker-controllable environment variables.
///
/// Like [`detect_preset`], but treats the following env vars as
/// absent (attacker can set them):
/// - `FORCE_COLOR`, `FORCE_HYPERLINK`, `COLORTERM`
/// - `TERM_PROGRAM`, `TERM_PROGRAM_VERSION`, `VTE_VERSION`
///
/// Trusted signals: `isatty(stdout)`, `TERM`, `NO_COLOR`.
///
/// Decision is binary:
/// - not a TTY, `NO_COLOR` set, `TERM=dumb`, or `TERM` unset/empty
///   → [`Dumb`](TerminalPreset::Dumb)
/// - otherwise → [`Sanitize`](TerminalPreset::Sanitize)
///
/// The finer-grained `BASIC`/`EXTENDED`/`TRUECOLOR` distinctions live
/// in [`detect_sgr_mask_untrusted`]. Both functions share the same
/// "TERM is color-capable?" heuristic via [`untrusted_term_has_color`]
/// so their policies stay aligned.
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

    // Ignore attacker-controllable env vars (FORCE_COLOR, FORCE_HYPERLINK,
    // COLORTERM, TERM_PROGRAM, TERM_PROGRAM_VERSION, VTE_VERSION). Only
    // TERM and isatty(stdout) are considered trusted.
    if !untrusted_term_has_color() {
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
///
/// Routes through [`untrusted_term_has_color`] for the base
/// "TERM is color-capable?" predicate, then refines by suffix:
/// - `"256color"` suffix → `BASIC | EXTENDED`
/// - `"color"` suffix → `BASIC`
/// - else (non-empty, non-dumb) → `BASIC` (conservative)
#[must_use]
pub fn detect_sgr_mask_untrusted() -> Option<SgrContent> {
    if !untrusted_term_has_color() {
        return None;
    }
    // Safe to unwrap: `untrusted_term_has_color` guarantees Ok + non-empty.
    let term = std::env::var("TERM").ok()?;

    if term.ends_with("256color") {
        Some(SgrContent::BASIC.union(SgrContent::EXTENDED))
    } else if term.ends_with("color") {
        Some(SgrContent::BASIC)
    } else {
        // Conservative: assume basic color support.
        Some(SgrContent::BASIC)
    }
}

/// Detect whether stdout supports OSC 8 hyperlinks.
///
/// Uses the `supports-hyperlinks` crate which checks `TERM_PROGRAM`,
/// `VTE_VERSION`, and other signals to determine if the terminal
/// can render clickable hyperlinks.
///
/// Returns `false` when stdout is not a TTY or when hyperlink
/// support cannot be determined.
///
/// OSC 8 hyperlinks are not a security concern (no echoback vector),
/// so this is purely a UX signal: avoid emitting sequences that the
/// terminal would render as garbage or ignore.
#[must_use]
pub fn detect_hyperlinks() -> bool {
    supports_hyperlinks::on(supports_hyperlinks::Stream::Stdout)
}

/// Detect hyperlink support from trusted signals only (untrusted mode).
///
/// Like [`detect_hyperlinks`], but ignores attacker-controllable env
/// vars (`FORCE_HYPERLINK`, `TERM_PROGRAM`, `VTE_VERSION`).
///
/// Falls back to `false` — conservative default when env cannot be
/// trusted. Only `isatty(stdout)` and `TERM` are considered trusted.
///
/// In untrusted mode, hyperlink support cannot be reliably determined
/// from `TERM` alone (no standard suffix convention), so this always
/// returns `false`.
#[must_use]
pub fn detect_hyperlinks_untrusted() -> bool {
    // TERM alone cannot indicate hyperlink support — no convention.
    // Conservative: assume no hyperlinks in untrusted environments.
    false
}
