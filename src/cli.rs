use clap::Parser;

/// Strip ANSI escape sequences from stdin or a file.
///
/// A faster, ECMA-48 compliant alternative to ansifilter for stripping use cases.
#[derive(Parser, Debug)]
#[command(name = "strip-ansi", version, about)]
pub struct Args {
    /// Check for ANSI sequences without stripping (exit 1 if found)
    #[arg(long, conflicts_with_all = ["head", "follow", "output"])]
    pub check: bool,

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
}
