use clap::Parser;

/// Strip ANSI escape sequences from stdin.
#[derive(Parser, Debug)]
#[command(name = "strip-ansi", version, about)]
pub struct Args {
    /// Check for ANSI sequences without stripping (exit 1 if found)
    #[arg(long)]
    pub check: bool,
}
