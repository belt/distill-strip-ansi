//! `distill-ansi` — transform ANSI escape sequences in terminal output.
//!
//! Companion to `strip-ansi`. Where `strip-ansi` removes sequences,
//! `distill-ansi` rewrites them: color depth reduction, palette
//! remapping, greyscale conversion.
//!
//! Both binaries share the `strip_ansi` library crate.

#![forbid(unsafe_code)]

use std::io::{self, BufRead, BufReader, Write};
use std::process::ExitCode;

use clap::Parser;

use strip_ansi::downgrade::ColorDepth;
use strip_ansi::palette::PaletteTransform;
use strip_ansi::sgr_rewrite::rewrite_sgr_params;
use strip_ansi::{ClassifyingParser, SeqAction, SeqKind, SgrContent};

#[cfg(feature = "color-palette")]
use strip_ansi::palette::{PROTANOPIA_VIENOT, DEUTERANOPIA_VIENOT};

/// Transform ANSI escape sequences in terminal output.
///
/// Rewrites color sequences to match a target color depth or palette.
/// Non-color sequences and plain text pass through unchanged.
#[derive(Parser, Debug)]
#[command(name = "distill-ansi", version, about)]
struct Args {
    /// Target color depth.
    ///
    /// truecolor: no change (pass-through).
    /// 256: downgrade truecolor to 256-color.
    /// 16: downgrade to basic 16 ANSI colors.
    /// greyscale: convert all colors to greyscale ramp.
    /// mono: strip all color, keep text styles.
    #[arg(long, value_name = "DEPTH", default_value = "truecolor")]
    color_depth: String,

    /// Color palette for remapping.
    ///
    /// default: no remapping.
    /// high-contrast-rg: optimize red-green distinction.
    /// high-contrast-by: optimize blue-yellow distinction.
    #[cfg(feature = "color-palette")]
    #[arg(long, value_name = "NAME", default_value = "default")]
    palette: String,

    /// Write output to FILE instead of stdout.
    #[arg(long, short = 'o', value_name = "FILE")]
    output: Option<String>,

    /// Input file (default: stdin).
    pub input: Option<String>,
}

fn main() -> ExitCode {
    sigpipe::reset();
    let args = Args::parse();

    let depth = match parse_depth(&args.color_depth) {
        Some(d) => d,
        None => {
            eprintln!(
                "distill-ansi: unknown color depth '{}'. \
                 Expected: truecolor, 256, 16, greyscale, mono",
                args.color_depth
            );
            return ExitCode::from(2);
        }
    };

    #[cfg(feature = "color-palette")]
    let palette = match parse_palette(&args.palette) {
        Some(p) => p,
        None => {
            eprintln!(
                "distill-ansi: unknown palette '{}'. \
                 Expected: default, high-contrast-rg, high-contrast-by",
                args.palette
            );
            return ExitCode::from(2);
        }
    };
    #[cfg(not(feature = "color-palette"))]
    let palette = PaletteTransform::default();

    let reader: Box<dyn BufRead> = match &args.input {
        Some(path) => match std::fs::File::open(path) {
            Ok(f) => Box::new(BufReader::new(f)),
            Err(e) => {
                eprintln!("distill-ansi: {path}: {e}");
                return ExitCode::from(1);
            }
        },
        None => Box::new(BufReader::new(io::stdin().lock())),
    };

    let mut writer: Box<dyn Write> = match &args.output {
        Some(path) => match std::fs::File::create(path) {
            Ok(f) => Box::new(io::BufWriter::new(f)),
            Err(e) => {
                eprintln!("distill-ansi: {path}: {e}");
                return ExitCode::from(1);
            }
        },
        None => Box::new(io::BufWriter::new(io::stdout().lock())),
    };

    if let Err(e) = run_transform(reader, &mut writer, depth, &palette) {
        if e.kind() != io::ErrorKind::BrokenPipe {
            eprintln!("distill-ansi: {e}");
            return ExitCode::from(1);
        }
    }

    ExitCode::SUCCESS
}

fn parse_depth(s: &str) -> Option<ColorDepth> {
    match s.to_ascii_lowercase().as_str() {
        "truecolor" | "true" | "24bit" => Some(ColorDepth::Truecolor),
        "256" | "256color" => Some(ColorDepth::Color256),
        "16" | "16color" => Some(ColorDepth::Color16),
        "greyscale" | "grayscale" | "grey" | "gray" => Some(ColorDepth::Greyscale),
        "mono" | "monochrome" => Some(ColorDepth::Mono),
        _ => None,
    }
}

#[cfg(feature = "color-palette")]
fn parse_palette(s: &str) -> Option<PaletteTransform> {
    match s.to_ascii_lowercase().as_str() {
        "default" | "none" => Some(PaletteTransform::default()),
        "high-contrast-rg" => Some(PaletteTransform::from_matrix(PROTANOPIA_VIENOT)),
        "high-contrast-by" => Some(PaletteTransform::from_matrix(DEUTERANOPIA_VIENOT)),
        _ => None,
    }
}

/// Core transform loop: read input, rewrite SGR sequences, write output.
fn run_transform(
    mut reader: Box<dyn BufRead>,
    writer: &mut dyn Write,
    depth: ColorDepth,
    _palette: &PaletteTransform,
) -> io::Result<()> {
    let no_op = depth == ColorDepth::Truecolor && _palette.is_identity();

    let mut buf = Vec::with_capacity(8192);
    loop {
        buf.clear();
        let n = reader.read_until(b'\n', &mut buf)?;
        if n == 0 {
            break;
        }

        if no_op {
            writer.write_all(&buf)?;
            continue;
        }

        let transformed = transform_line(&buf, depth, _palette);
        writer.write_all(&transformed)?;
    }
    writer.flush()
}

/// Transform a single line/chunk: rewrite SGR color sequences.
fn transform_line(input: &[u8], depth: ColorDepth, _palette: &PaletteTransform) -> Vec<u8> {
    use memchr::memchr;

    // Fast path: no ESC byte.
    if memchr(0x1B, input).is_none() {
        return input.to_vec();
    }

    let mut cp = ClassifyingParser::new();
    let mut output = Vec::with_capacity(input.len());
    let mut seq_buf: Vec<u8> = Vec::new();
    let mut in_seq = false;
    let mut remaining = input;

    while !remaining.is_empty() {
        let pos = memchr(0x1B, remaining).unwrap_or(remaining.len());
        output.extend_from_slice(&remaining[..pos]);
        remaining = &remaining[pos..];
        if remaining.is_empty() {
            break;
        }

        let mut i = 0;
        let mut broke_on_end = false;
        while i < remaining.len() {
            let action = cp.feed(remaining[i]);
            match action {
                SeqAction::StartSeq => {
                    in_seq = true;
                    seq_buf.clear();
                    seq_buf.push(remaining[i]);
                }
                SeqAction::InSeq => {
                    seq_buf.push(remaining[i]);
                }
                SeqAction::EndSeq => {
                    seq_buf.push(remaining[i]);
                    in_seq = false;

                    if cp.current_kind() == SeqKind::CsiSgr
                        && cp.sgr_content() != SgrContent::empty()
                    {
                        // Rewrite SGR color params.
                        let rewritten = rewrite_sgr_params(&seq_buf, depth);
                        output.extend_from_slice(&rewritten);
                    } else {
                        // Non-SGR or no color content — pass through.
                        output.extend_from_slice(&seq_buf);
                    }

                    seq_buf.clear();
                    remaining = &remaining[i + 1..];
                    broke_on_end = true;
                    break;
                }
                SeqAction::Emit => {
                    let b = remaining[i];
                    if in_seq && (b == 0x18 || b == 0x1A) {
                        // Abort byte — suppress.
                    } else {
                        output.push(b);
                    }
                    in_seq = false;
                }
            }
            i += 1;
        }

        if !broke_on_end {
            remaining = &[];
        }
    }

    output
}
