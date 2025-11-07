use clap::{Parser, ValueEnum};

/// The action to take when threats are detected.
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum OnThreatMode {
    /// Exit 77, no output
    Fail,
    /// Strip threats, report to stderr, exit 0
    Strip,
}

/// Strip ANSI escape sequences from byte streams
///
/// Remove colors, cursor movement, hyperlinks, and other terminal
/// control sequences from piped output. Auto-detects whether stdout
/// is a terminal and picks safe defaults.
///
/// Examples:
///
///   cargo build --color=always 2>&1 | strip-ansi
///
///   docker build . 2>&1 | strip-ansi > build.log
///
///   strip-ansi --check < input.txt
///
///   strip-ansi --preset color < input.txt
#[derive(Parser, Debug)]
#[command(name = "strip-ansi", version, about, long_about)]
pub struct Args {
    /// Input file [default: stdin]
    pub input: Option<String>,

    // ── Output control ──────────────────────────────────────────
    /// Output first N lines only
    #[arg(long, value_name = "N", help_heading = "Output")]
    pub head: Option<usize>,

    /// Write to FILE instead of stdout
    #[arg(
        long,
        short = 'o',
        value_name = "FILE",
        conflicts_with = "check",
        help_heading = "Output"
    )]
    pub output: Option<String>,

    /// Keep reading after EOF (like tail -f)
    #[arg(long, short = 'f', conflicts_with = "check", help_heading = "Output")]
    pub follow: bool,

    /// Stop after reading N bytes
    #[arg(long, value_name = "BYTES", help_heading = "Output")]
    pub max_size: Option<u64>,

    /// Print stripped byte count to stderr on exit
    #[arg(long, short = 'c', help_heading = "Counting (wc-esque)")]
    pub count: bool,

    /// Print line count to stderr on exit
    #[arg(long, short = 'l', help_heading = "Counting (wc-esque)")]
    pub lines: bool,

    /// Print word count to stderr on exit
    #[arg(long, short = 'w', help_heading = "Counting (wc-esque)")]
    pub word_count: bool,

    /// Print character count to stderr on exit
    #[arg(long, short = 'm', help_heading = "Counting (wc-esque)")]
    pub char_count: bool,

    /// Show non-printing characters (like cat -v)
    #[arg(long, short = 'v', help_heading = "Display (cat-esque)")]
    pub show_nonprinting: bool,

    /// Number output lines (like cat -n)
    #[arg(long = "number", short = 'n', help_heading = "Display (cat-esque)")]
    pub number_lines: bool,

    /// Show tabs as ^I (like cat -t, implies -v)
    #[arg(long, short = 't', help_heading = "Display (cat-esque)")]
    pub show_tabs: bool,

    /// Show $ at end of lines (like cat -e, implies -v)
    #[arg(long, short = 'e', help_heading = "Display (cat-esque)")]
    pub show_ends: bool,

    /// Show all: -vET (like cat -A)
    #[arg(short = 'A', help_heading = "Display (cat-esque)")]
    pub show_all: bool,

    /// Full stream stats as JSON to stderr on exit
    #[cfg(feature = "filter")]
    #[arg(long, help_heading = "Counting (wc-esque)")]
    pub stats: bool,

    // ── Check mode ──────────────────────────────────────────────
    /// Detect ANSI sequences (exit 1 if found, no output)
    #[arg(long, conflicts_with_all = ["head", "follow", "output", "check_threats"],
          help_heading = "Check mode")]
    pub check: bool,

    // ── Security ────────────────────────────────────────────────
    /// Scan for echoback attack vectors (exit 77 if found)
    #[cfg(feature = "filter")]
    #[arg(long, conflicts_with_all = ["check", "head", "follow", "output"],
          help_heading = "Security")]
    pub check_threats: bool,

    /// Action on threat: fail or strip
    #[cfg(feature = "filter")]
    #[arg(
        long,
        value_enum,
        default_value = "fail",
        requires = "check_threats",
        hide_default_value = true,
        help_heading = "Security"
    )]
    pub on_threat: OnThreatMode,

    /// Suppress threat report on stderr
    #[cfg(feature = "filter")]
    #[arg(long, requires = "check_threats", help_heading = "Security")]
    pub no_threat_report: bool,

    /// Load external threat database (additive to builtins)
    #[cfg(feature = "toml-config")]
    #[arg(
        long,
        value_name = "PATH",
        requires = "check_threats",
        help_heading = "Security"
    )]
    pub threat_db: Option<String>,

    // ── Presets ─────────────────────────────────────────────────
    /// dumb, color, vt100, tmux, sanitize, xterm, full
    ///
    /// Aliases: pipe=dumb ci=color pager=color
    /// screen=tmux safe=sanitize modern=full
    ///
    /// Default: auto-detect
    #[cfg(feature = "filter")]
    #[arg(long, value_name = "NAME", help_heading = "Presets")]
    pub preset: Option<String>,

    /// Allow xterm/full (preserves dangerous sequences)
    #[cfg(feature = "filter")]
    #[arg(long, hide_short_help = true, help_heading = "Presets")]
    pub r#unsafe: bool,

    // ── Selective preserve ──────────────────────────────────────
    //
    // Fine-grained control. Most users should use --preset instead.
    /// Colors and styles
    #[cfg(feature = "filter")]
    #[arg(long, hide_short_help = true, help_heading = "Preserve (fine-grained)")]
    pub no_strip_csi_sgr: bool,

    /// Cursor movement
    #[cfg(feature = "filter")]
    #[arg(long, hide_short_help = true, help_heading = "Preserve (fine-grained)")]
    pub no_strip_csi_cursor: bool,

    /// Erase (clear screen/line)
    #[cfg(feature = "filter")]
    #[arg(long, hide_short_help = true, help_heading = "Preserve (fine-grained)")]
    pub no_strip_csi_erase: bool,

    /// Scroll regions
    #[cfg(feature = "filter")]
    #[arg(long, hide_short_help = true, help_heading = "Preserve (fine-grained)")]
    pub no_strip_csi_scroll: bool,

    /// Terminal mode changes
    #[cfg(feature = "filter")]
    #[arg(long, hide_short_help = true, help_heading = "Preserve (fine-grained)")]
    pub no_strip_csi_mode: bool,

    /// Window title/resize
    #[cfg(feature = "filter")]
    #[arg(long, hide_short_help = true, help_heading = "Preserve (fine-grained)")]
    pub no_strip_csi_window: bool,

    /// All CSI (colors, cursor, erase, scroll, modes)
    #[cfg(feature = "filter")]
    #[arg(long, hide_short_help = true, help_heading = "Preserve (by group)")]
    pub no_strip_csi: bool,

    /// Hyperlinks, titles, notifications (OSC)
    #[cfg(feature = "filter")]
    #[arg(long, hide_short_help = true, help_heading = "Preserve (by group)")]
    pub no_strip_osc: bool,

    /// Device control strings (DCS)
    #[cfg(feature = "filter")]
    #[arg(long, hide_short_help = true, help_heading = "Preserve (by group)")]
    pub no_strip_dcs: bool,

    /// Application commands (APC)
    #[cfg(feature = "filter")]
    #[arg(long, hide_short_help = true, help_heading = "Preserve (by group)")]
    pub no_strip_apc: bool,

    /// Privacy messages (PM)
    #[cfg(feature = "filter")]
    #[arg(long, hide_short_help = true, help_heading = "Preserve (by group)")]
    pub no_strip_pm: bool,

    /// Start-of-string (SOS)
    #[cfg(feature = "filter")]
    #[arg(long, hide_short_help = true, help_heading = "Preserve (by group)")]
    pub no_strip_sos: bool,

    /// Single-shift 2 / G2 charset (SS2)
    #[cfg(feature = "filter")]
    #[arg(long, hide_short_help = true, help_heading = "Preserve (by group)")]
    pub no_strip_ss2: bool,

    /// Single-shift 3 / G3 charset (SS3)
    #[cfg(feature = "filter")]
    #[arg(long, hide_short_help = true, help_heading = "Preserve (by group)")]
    pub no_strip_ss3: bool,

    /// Other escape sequences (Fe)
    #[cfg(feature = "filter")]
    #[arg(long, hide_short_help = true, help_heading = "Preserve (by group)")]
    pub no_strip_fe: bool,

    // ── Config ──────────────────────────────────────────────────
    /// Load config from TOML file
    #[cfg(feature = "toml-config")]
    #[arg(long, value_name = "PATH")]
    pub config: Option<String>,
}
