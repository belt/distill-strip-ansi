use clap::{Parser, ValueEnum};

/// The action to take when threats are detected.
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum OnThreatMode {
    /// Scan only, exit 77 on detection, no output to stdout.
    Fail,
    /// Strip dangerous sequences, write clean output to stdout, report threats to stderr, exit 0.
    Strip,
}

/// Strip ANSI escape sequences from stdin or a file.
///
/// A faster, ECMA-48 compliant alternative to ansifilter for stripping use cases.
#[derive(Parser, Debug)]
#[command(name = "strip-ansi", version, about)]
pub struct Args {
    /// Check for ANSI sequences without stripping (exit 1 if found)
    #[arg(long, conflicts_with_all = ["head", "follow", "output", "check_threats"])]
    pub check: bool,

    /// Scan input for echoback threat vectors
    #[cfg(feature = "filter")]
    #[arg(long, conflicts_with_all = ["check", "head", "follow", "output"])]
    pub check_threats: bool,

    /// Action when threats are detected (default: fail)
    #[cfg(feature = "filter")]
    #[arg(long, value_enum, default_value = "fail", requires = "check_threats")]
    pub on_threat: OnThreatMode,

    /// Suppress threat report on stderr (preserves exit codes)
    #[cfg(feature = "filter")]
    #[arg(long, requires = "check_threats")]
    pub no_threat_report: bool,

    /// Output only the first N lines (after stripping)
    #[arg(long, short = 'n', value_name = "N")]
    pub head: Option<usize>,

    /// Keep reading after EOF (like tail -f)
    #[arg(long, short = 'f', conflicts_with = "check")]
    pub follow: bool,

    /// Write output to FILE instead of stdout
    #[arg(long, short = 'o', value_name = "FILE", conflicts_with = "check")]
    pub output: Option<String>,

    /// Print count of stripped bytes to stderr on exit
    #[arg(long, short = 'c')]
    pub count: bool,

    /// Stop reading after N bytes of input (ansifilter compat)
    #[arg(long, value_name = "BYTES")]
    pub max_size: Option<u64>,

    /// Input file (default: stdin)
    pub input: Option<String>,

    // ── Preset flag ─────────────────────────────────────────────

    /// Terminal capability preset (overrides auto-detection).
    ///
    /// Standard: dumb, color, vt100, tmux, xterm, full.
    /// Aliases:  pipe=dumb, ci=color, pager=color, screen=tmux, modern=full.
    ///
    /// Without --preset, the tool auto-detects output capabilities.
    /// Use --preset dumb to force strip-all behavior.
    #[cfg(feature = "filter")]
    #[arg(long, value_name = "NAME")]
    pub preset: Option<String>,

    /// Allow presets that preserve dangerous sequences (xterm, full).
    ///
    /// Required for pen-testing and terminal development.
    #[cfg(feature = "filter")]
    #[arg(long, hide_short_help = true)]
    pub r#unsafe: bool,

    // ── Filter group flags (--no-strip-{group}) ─────────────────

    /// Preserve all CSI sequences
    #[cfg(feature = "filter")]
    #[arg(long)]
    pub no_strip_csi: bool,

    /// Preserve OSC sequences
    #[cfg(feature = "filter")]
    #[arg(long)]
    pub no_strip_osc: bool,

    /// Preserve DCS sequences
    #[cfg(feature = "filter")]
    #[arg(long)]
    pub no_strip_dcs: bool,

    /// Preserve APC sequences
    #[cfg(feature = "filter")]
    #[arg(long)]
    pub no_strip_apc: bool,

    /// Preserve PM sequences
    #[cfg(feature = "filter")]
    #[arg(long)]
    pub no_strip_pm: bool,

    /// Preserve SOS sequences
    #[cfg(feature = "filter")]
    #[arg(long)]
    pub no_strip_sos: bool,

    /// Preserve SS2 sequences
    #[cfg(feature = "filter")]
    #[arg(long)]
    pub no_strip_ss2: bool,

    /// Preserve SS3 sequences
    #[cfg(feature = "filter")]
    #[arg(long)]
    pub no_strip_ss3: bool,

    /// Preserve other Fe sequences
    #[cfg(feature = "filter")]
    #[arg(long)]
    pub no_strip_fe: bool,

    // ── CSI sub-group flags (--no-strip-csi-{sub}) ──────────────

    /// Preserve SGR (colors/styles) only
    #[cfg(feature = "filter")]
    #[arg(long)]
    pub no_strip_csi_sgr: bool,

    /// Preserve cursor movement only
    #[cfg(feature = "filter")]
    #[arg(long)]
    pub no_strip_csi_cursor: bool,

    /// Preserve erase sequences only
    #[cfg(feature = "filter")]
    #[arg(long)]
    pub no_strip_csi_erase: bool,

    /// Preserve scroll sequences only
    #[cfg(feature = "filter")]
    #[arg(long)]
    pub no_strip_csi_scroll: bool,

    /// Preserve mode set/reset only
    #[cfg(feature = "filter")]
    #[arg(long)]
    pub no_strip_csi_mode: bool,

    /// Preserve window manipulation only
    #[cfg(feature = "filter")]
    #[arg(long)]
    pub no_strip_csi_window: bool,

    // ── Config file flag ────────────────────────────────────────

    /// Load filter configuration from TOML file
    #[cfg(feature = "toml-config")]
    #[arg(long, value_name = "PATH")]
    pub config: Option<String>,

    /// Load external threat database from TOML file (additive to builtins)
    #[cfg(feature = "toml-config")]
    #[arg(long, value_name = "PATH", requires = "check_threats")]
    pub threat_db: Option<String>,
}
