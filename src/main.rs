mod cli;
mod io;

use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::process::ExitCode;

use clap::Parser;

use cli::Args;
use io::OutputBuffer;
use strip_ansi::StripStream;

fn main() -> ExitCode {
    sigpipe::reset();

    let args = Args::parse();

    let reader: Box<dyn BufRead> = match &args.input {
        Some(path) => {
            let file = match std::fs::File::open(path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("strip-ansi: {path}: {e}");
                    return ExitCode::from(1);
                }
            };
            Box::new(BufReader::with_capacity(32 * 1024, file))
        }
        None => {
            let stdin = std::io::stdin();
            Box::new(BufReader::with_capacity(32 * 1024, stdin.lock()))
        }
    };

    if args.check {
        run_check_mode(reader)
    } else {
        run_strip_mode(reader)
    }
}

fn run_strip_mode(mut reader: Box<dyn BufRead>) -> ExitCode {
    let stdout = std::io::stdout();
    let mut writer = OutputBuffer::new(&stdout);
    let mut stream = StripStream::new();
    let mut buf = [0u8; 32 * 1024];

    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => return handle_io_error(e),
        };
        for slice in stream.strip_slices(&buf[..n]) {
            if let Err(e) = writer.write_all(slice) {
                return handle_io_error(e);
            }
        }
    }
    stream.finish();

    if let Err(e) = writer.flush() {
        return handle_io_error(e);
    }

    ExitCode::SUCCESS
}

fn run_check_mode(mut reader: Box<dyn BufRead>) -> ExitCode {
    loop {
        let buf = match reader.fill_buf() {
            Ok(b) => b,
            Err(e) => return handle_io_error(e),
        };
        if buf.is_empty() {
            return ExitCode::SUCCESS;
        }
        if strip_ansi::contains_ansi(buf) {
            eprintln!("strip-ansi: ANSI escape sequences detected");
            return ExitCode::from(1);
        }
        let len = buf.len();
        reader.consume(len);
    }
}

fn handle_io_error(e: std::io::Error) -> ExitCode {
    if e.kind() == ErrorKind::BrokenPipe {
        return ExitCode::SUCCESS;
    }
    eprintln!("strip-ansi: {e}");
    ExitCode::from(1)
}
