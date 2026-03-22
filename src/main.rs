mod cli;
mod io;

use std::io::{BufRead, BufReader, BufWriter, ErrorKind, Write};
use std::process::ExitCode;

use clap::Parser;

use cli::Args;
use io::OutputBuffer;
use strip_ansi::StripStream;

#[cfg(feature = "filter")]
use strip_ansi::{FilterConfig, FilterStream, SeqGroup, SeqKind};

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

// ── Filter config builder ───────────────────────────────────────────

/// Build a [`FilterConfig`] from CLI flags and optional TOML config.
///
/// - `--config` provided: load TOML first, then overlay CLI flags
/// - CLI flags only: build from flags directly
/// - Neither: returns `FilterConfig::strip_all()` (zero overhead)
#[cfg(feature = "filter")]
fn build_filter_config(args: &Args) -> Result<FilterConfig, ExitCode> {
    // Start from TOML if --config is provided.
    #[cfg(feature = "toml-config")]
    let mut config = if let Some(ref path) = args.config {
        let toml = match strip_ansi::StripAnsiConfig::from_file(std::path::Path::new(path)) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("strip-ansi: --config {path}: {e}");
                return Err(ExitCode::from(1));
            }
        };
        match toml.to_filter_config() {
            Ok(fc) => fc,
            Err(e) => {
                eprintln!("strip-ansi: --config {path}: {e}");
                return Err(ExitCode::from(1));
            }
        }
    } else {
        FilterConfig::strip_all()
    };

    #[cfg(not(feature = "toml-config"))]
    let mut config = FilterConfig::strip_all();

    // Overlay CLI --no-strip-* flags (additive).
    if args.no_strip_csi {
        config = config.no_strip_group(SeqGroup::Csi);
    }
    if args.no_strip_osc {
        config = config.no_strip_group(SeqGroup::Osc);
    }
    if args.no_strip_dcs {
        config = config.no_strip_group(SeqGroup::Dcs);
    }
    if args.no_strip_apc {
        config = config.no_strip_group(SeqGroup::Apc);
    }
    if args.no_strip_pm {
        config = config.no_strip_group(SeqGroup::Pm);
    }
    if args.no_strip_sos {
        config = config.no_strip_group(SeqGroup::Sos);
    }
    if args.no_strip_ss2 {
        config = config.no_strip_group(SeqGroup::Ss2);
    }
    if args.no_strip_ss3 {
        config = config.no_strip_group(SeqGroup::Ss3);
    }
    if args.no_strip_fe {
        config = config.no_strip_group(SeqGroup::Fe);
    }

    // CSI sub-group flags.
    if args.no_strip_csi_sgr {
        config = config.no_strip_kind(SeqKind::CsiSgr);
    }
    if args.no_strip_csi_cursor {
        config = config.no_strip_kind(SeqKind::CsiCursor);
    }
    if args.no_strip_csi_erase {
        config = config.no_strip_kind(SeqKind::CsiErase);
    }
    if args.no_strip_csi_scroll {
        config = config.no_strip_kind(SeqKind::CsiScroll);
    }
    if args.no_strip_csi_mode {
        config = config.no_strip_kind(SeqKind::CsiMode);
    }
    if args.no_strip_csi_window {
        config = config.no_strip_kind(SeqKind::CsiWindow);
    }

    Ok(config)
}

fn run_strip_mode(mut reader: Box<dyn BufRead>, args: &Args) -> ExitCode {
    let mut writer = match open_writer(args) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("strip-ansi: {}: {e}", args.output.as_deref().unwrap_or("-"));
            return ExitCode::from(1);
        }
    };

    // Build filter config; use FilterStream when not strip-all.
    #[cfg(feature = "filter")]
    let filter_config = match build_filter_config(args) {
        Ok(fc) => fc,
        Err(code) => return code,
    };

    #[cfg(feature = "filter")]
    let use_filter = !filter_config.is_strip_all();

    let mut strip_stream = StripStream::new();
    #[cfg(feature = "filter")]
    let mut filter_stream = FilterStream::new();

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

        // Choose the appropriate streaming path.
        #[cfg(feature = "filter")]
        let slices: Box<dyn Iterator<Item = &[u8]>> = if use_filter {
            Box::new(filter_stream.filter_slices(&buf[..n], &filter_config))
        } else {
            Box::new(strip_stream.strip_slices(&buf[..n]))
        };
        #[cfg(not(feature = "filter"))]
        let slices = strip_stream.strip_slices(&buf[..n]);

        for slice in slices {
            let slice: &[u8] = slice;
            chunk_clean += slice.len() as u64;

            if let Some(ref mut remaining) = lines_remaining {
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

    #[cfg(feature = "filter")]
    if use_filter {
        filter_stream.finish();
    } else {
        strip_stream.finish();
    }
    #[cfg(not(feature = "filter"))]
    strip_stream.finish();

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
