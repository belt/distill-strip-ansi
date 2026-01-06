mod cli;
mod io;
mod strip;

use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::process::ExitCode;

use clap::Parser;

use cli::Args;
use io::OutputBuffer;
use strip::{run_check, run_strip};

fn main() -> ExitCode {
    // Restore default SIGPIPE handling so piping through `head` etc.
    // terminates cleanly without panic messages on stderr.
    sigpipe::reset();

    let args = Args::parse();

    let stdin = std::io::stdin();
    let reader = BufReader::with_capacity(32 * 1024, stdin.lock());

    if args.check {
        run_check_mode(reader)
    } else {
        run_strip_mode(reader)
    }
}

fn run_strip_mode<R: BufRead>(reader: R) -> ExitCode {
    let stdout = std::io::stdout();
    let mut writer = OutputBuffer::new(&stdout);

    if let Err(e) = run_strip(reader, &mut writer) {
        return handle_io_error(e);
    }

    if let Err(e) = writer.flush() {
        return handle_io_error(e);
    }

    ExitCode::SUCCESS
}

fn run_check_mode<R: BufRead>(reader: R) -> ExitCode {
    match run_check(reader) {
        Ok(true) => {
            eprintln!("strip-ansi: ANSI escape sequences detected");
            ExitCode::from(1)
        }
        Ok(false) => ExitCode::SUCCESS,
        Err(e) => handle_io_error(e),
    }
}

fn handle_io_error(e: std::io::Error) -> ExitCode {
    if e.kind() == ErrorKind::BrokenPipe {
        return ExitCode::SUCCESS;
    }
    eprintln!("strip-ansi: {e}");
    ExitCode::from(1)
}
