mod cli;
mod io;

use std::io::{BufRead, BufReader, BufWriter, ErrorKind, Write};
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
        run_check_mode(reader, &args)
    } else {
        run_strip_mode(reader, &args)
    }
}

fn open_writer(args: &Args) -> Result<Box<dyn Write>, std::io::Error> {
    match &args.output {
        Some(path) => {
            let file = std::fs::File::create(path)?;
            Ok(Box::new(BufWriter::with_capacity(32 * 1024, file)))
        }
        None => {
            let stdout = std::io::stdout();
            Ok(Box::new(OutputBuffer::new(stdout)))
        }
    }
}

fn run_strip_mode(mut reader: Box<dyn BufRead>, args: &Args) -> ExitCode {
    let mut writer = match open_writer(args) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("strip-ansi: {}: {e}", args.output.as_deref().unwrap_or("-"));
            return ExitCode::from(1);
        }
    };

    let mut stream = StripStream::new();
    let mut buf = [0u8; 32 * 1024];
    let mut lines_remaining = args.head;
    let mut bytes_read: u64 = 0;
    let mut bytes_stripped: u64 = 0;
    let max_size = args.max_size.unwrap_or(u64::MAX);

    loop {
        // Cap read to max_size budget.
        let budget = max_size.saturating_sub(bytes_read);
        if budget == 0 {
            break;
        }
        let read_len = buf.len().min(budget as usize);

        let n = match reader.read(&mut buf[..read_len]) {
            Ok(0) => {
                if args.follow {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }
                break;
            }
            Ok(n) => n,
            Err(e) => return handle_io_error(e),
        };
        bytes_read += n as u64;

        let mut chunk_clean: u64 = 0;
        for slice in stream.strip_slices(&buf[..n]) {
            let slice: &[u8] = slice;
            chunk_clean += slice.len() as u64;

            if let Some(ref mut remaining) = lines_remaining {
                // Write up to `remaining` newlines, then stop.
                if *remaining == 0 {
                    break;
                }
                if let Err(e) = write_head_limited(&mut writer, slice, remaining) {
                    return handle_io_error(e);
                }
                if *remaining == 0 {
                    break;
                }
            } else if let Err(e) = writer.write_all(slice) {
                return handle_io_error(e);
            }
        }

        if matches!(lines_remaining, Some(0)) {
            break;
        }
        bytes_stripped += n as u64 - chunk_clean;
    }
    stream.finish();

    if let Err(e) = writer.flush() {
        return handle_io_error(e);
    }

    if args.count {
        eprintln!("{bytes_stripped}");
    }

    ExitCode::SUCCESS
}

/// Write `slice` but stop after emitting the Nth newline.
fn write_head_limited(
    writer: &mut dyn Write,
    slice: &[u8],
    remaining: &mut usize,
) -> std::io::Result<()> {
    let mut offset = 0;
    while *remaining > 0 && offset < slice.len() {
        if let Some(pos) = memchr::memchr(b'\n', &slice[offset..]) {
            let end = offset + pos + 1;
            writer.write_all(&slice[offset..end])?;
            *remaining -= 1;
            offset = end;
        } else {
            // No newline in remainder — write it all, line continues in next chunk.
            writer.write_all(&slice[offset..])?;
            break;
        }
    }
    Ok(())
}

fn run_check_mode(mut reader: Box<dyn BufRead>, args: &Args) -> ExitCode {
    let mut bytes_read: u64 = 0;
    let max_size = args.max_size.unwrap_or(u64::MAX);

    loop {
        let buf = match reader.fill_buf() {
            Ok(b) => b,
            Err(e) => return handle_io_error(e),
        };
        if buf.is_empty() {
            return ExitCode::SUCCESS;
        }

        let budget = max_size.saturating_sub(bytes_read);
        if budget == 0 {
            return ExitCode::SUCCESS;
        }
        let check_len = buf.len().min(budget as usize);

        if strip_ansi::contains_ansi(&buf[..check_len]) {
            eprintln!("strip-ansi: ANSI escape sequences detected");
            return ExitCode::from(1);
        }
        bytes_read += check_len as u64;
        reader.consume(check_len);
    }
}

fn handle_io_error(e: std::io::Error) -> ExitCode {
    if e.kind() == ErrorKind::BrokenPipe {
        return ExitCode::SUCCESS;
    }
    eprintln!("strip-ansi: {e}");
    ExitCode::from(1)
}
