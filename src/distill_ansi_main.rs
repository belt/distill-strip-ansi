//! `distill-ansi` — transform ANSI escape sequences in terminal output.
//!
//! Companion to `strip-ansi`. Where `strip-ansi` removes sequences,
//! `distill-ansi` rewrites them: color depth reduction, palette
//! remapping, greyscale conversion, Unicode normalization.
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

#[cfg(feature = "unicode-normalize")]
use strip_ansi::unicode_map::UnicodeMap;

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

    // ── Unicode normalization ───────────────────────────────────────

    /// Add Unicode mapping sets by @tag, name, or file path.
    ///
    /// Additive to the default @ascii-normalize builtins.
    /// Tags: @security, @ascii-normalize, @narrowing, @widening,
    /// @canonicalize, @japanese, @korean, @cjk, @math, @arabic, @all.
    /// Names: math-latin, math-greek, enclosed-alphanumerics, etc.
    /// Paths: path/to/custom.toml
    #[cfg(feature = "unicode-normalize")]
    #[arg(long = "unicode-map", value_name = "SPEC")]
    unicode_map: Vec<String>,

    /// Remove Unicode mapping sets by @tag or name.
    ///
    /// Removing @security-tagged sets requires --unsafe.
    #[cfg(feature = "unicode-normalize")]
    #[arg(long = "no-unicode-map", value_name = "SPEC")]
    no_unicode_map: Vec<String>,

    /// Allow presets that preserve dangerous ANSI sequences (xterm, full).
    ///
    /// Required for pen-testing and terminal development.
    #[arg(long, hide_short_help = true)]
    r#unsafe: bool,
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

    #[cfg(feature = "unicode-normalize")]
    let unicode_map = match build_unicode_map(&args) {
        Ok(m) => m,
        Err(code) => return code,
    };
    #[cfg(not(feature = "unicode-normalize"))]
    let unicode_map: Option<()> = None;

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

    if let Err(e) = run_transform(reader, &mut writer, depth, &palette, &unicode_map) {
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

/// Core transform loop: read input, rewrite SGR sequences, normalize Unicode, write output.
fn run_transform(
    mut reader: Box<dyn BufRead>,
    writer: &mut dyn Write,
    depth: ColorDepth,
    _palette: &PaletteTransform,
    #[cfg(feature = "unicode-normalize")] unicode_map: &Option<UnicodeMap>,
    #[cfg(not(feature = "unicode-normalize"))] _unicode_map: &Option<()>,
) -> io::Result<()> {
    let color_no_op = depth == ColorDepth::Truecolor && _palette.is_identity();

    #[cfg(feature = "unicode-normalize")]
    let unicode_active = unicode_map.is_some();
    #[cfg(not(feature = "unicode-normalize"))]
    let unicode_active = false;

    let full_no_op = color_no_op && !unicode_active;

    let mut buf = Vec::with_capacity(8192);
    loop {
        buf.clear();
        let n = reader.read_until(b'\n', &mut buf)?;
        if n == 0 {
            break;
        }

        if full_no_op {
            writer.write_all(&buf)?;
            continue;
        }

        let mut transformed = if color_no_op {
            buf.clone()
        } else {
            transform_line(&buf, depth, _palette)
        };

        #[cfg(feature = "unicode-normalize")]
        if let Some(map) = unicode_map {
            transformed = normalize_content(&transformed, map);
        }

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

// ── Unicode normalization ───────────────────────────────────────────

#[cfg(feature = "unicode-normalize")]
fn normalize_content(input: &[u8], map: &UnicodeMap) -> Vec<u8> {
    use memchr::memchr;

    // Fast path: no bytes >= 0x80 means pure ASCII — nothing to normalize.
    // (All builtin sources are >= U+00B2, which is 0xC2 0xB2 in UTF-8.)
    if memchr(0xC2, input).is_none()
        && memchr(0xC3, input).is_none()
        && memchr(0xE2, input).is_none()
        && memchr(0xEF, input).is_none()
        && memchr(0xF0, input).is_none()
    {
        return input.to_vec();
    }

    let s = match std::str::from_utf8(input) {
        Ok(s) => s,
        Err(_) => return input.to_vec(), // not valid UTF-8, pass through
    };

    let mut output = Vec::with_capacity(input.len());
    let mut char_buf = Vec::new();
    let mut modified = false;

    for c in s.chars() {
        char_buf.clear();
        if map.lookup_into(c, &mut char_buf) {
            for &tc in &char_buf {
                let mut enc = [0u8; 4];
                let encoded = tc.encode_utf8(&mut enc);
                output.extend_from_slice(encoded.as_bytes());
            }
            modified = true;
        } else {
            let mut enc = [0u8; 4];
            let encoded = c.encode_utf8(&mut enc);
            output.extend_from_slice(encoded.as_bytes());
        }
    }

    if modified {
        output
    } else {
        input.to_vec()
    }
}

/// Shipped TOML file names and their tags.
#[cfg(feature = "unicode-normalize")]
const SHIPPED_FILES: &[(&str, &[&str])] = &[
    ("math-latin", &["math", "canonicalize", "ascii-normalize"]),
    ("math-greek", &["math", "canonicalize"]),
    ("enclosed-alphanumerics", &["ascii-normalize", "canonicalize"]),
    ("enclosed-alphanumeric-supplement", &["ascii-normalize", "canonicalize"]),
    ("enclosed-cjk", &["cjk", "japanese", "korean", "canonicalize"]),
    ("cjk-compatibility", &["japanese", "cjk", "canonicalize"]),
    ("halfwidth-katakana", &["japanese", "legacy-encoding", "canonicalize"]),
    ("halfwidth-hangul", &["korean", "legacy-encoding", "canonicalize"]),
    ("cjk-compat-ideographs", &["cjk", "japanese", "korean", "canonicalize"]),
    ("cjk-compat-ideographs-supplement", &["cjk", "canonicalize"]),
    ("arabic-presentation-forms", &["arabic", "canonicalize"]),
];

/// Shipped TOML file direction metadata for @narrowing/@widening filtering.
#[cfg(feature = "unicode-normalize")]
const SHIPPED_DIRECTIONS: &[(&str, &str)] = &[
    ("math-latin", "narrowing"),
    ("math-greek", "neutral"),
    ("enclosed-alphanumerics", "narrowing"),
    ("enclosed-alphanumeric-supplement", "narrowing"),
    ("enclosed-cjk", "neutral"),
    ("cjk-compatibility", "narrowing"),
    ("halfwidth-katakana", "widening"),
    ("halfwidth-hangul", "widening"),
    ("cjk-compat-ideographs", "neutral"),
    ("cjk-compat-ideographs-supplement", "neutral"),
    ("arabic-presentation-forms", "neutral"),
];

/// Resolve a `@tag` to a list of shipped file names.
#[cfg(feature = "unicode-normalize")]
fn resolve_tag(tag: &str) -> Vec<&'static str> {
    match tag {
        "@all" => SHIPPED_FILES.iter().map(|(name, _)| *name).collect(),
        "@canonicalize" => SHIPPED_FILES
            .iter()
            .filter(|(_, tags)| tags.contains(&"canonicalize"))
            .map(|(name, _)| *name)
            .collect(),
        "@ascii-normalize" => SHIPPED_FILES
            .iter()
            .filter(|(_, tags)| tags.contains(&"ascii-normalize"))
            .map(|(name, _)| *name)
            .collect(),
        "@math" => SHIPPED_FILES
            .iter()
            .filter(|(_, tags)| tags.contains(&"math"))
            .map(|(name, _)| *name)
            .collect(),
        "@japanese" => SHIPPED_FILES
            .iter()
            .filter(|(_, tags)| tags.contains(&"japanese"))
            .map(|(name, _)| *name)
            .collect(),
        "@korean" => SHIPPED_FILES
            .iter()
            .filter(|(_, tags)| tags.contains(&"korean"))
            .map(|(name, _)| *name)
            .collect(),
        "@cjk" => SHIPPED_FILES
            .iter()
            .filter(|(_, tags)| tags.contains(&"cjk"))
            .map(|(name, _)| *name)
            .collect(),
        "@arabic" => SHIPPED_FILES
            .iter()
            .filter(|(_, tags)| tags.contains(&"arabic"))
            .map(|(name, _)| *name)
            .collect(),
        "@narrowing" => SHIPPED_DIRECTIONS
            .iter()
            .filter(|(_, dir)| *dir == "narrowing" || *dir == "neutral")
            .map(|(name, _)| *name)
            .collect(),
        "@widening" => SHIPPED_DIRECTIONS
            .iter()
            .filter(|(_, dir)| *dir == "widening")
            .map(|(name, _)| *name)
            .collect(),
        _ => vec![],
    }
}

/// Resolve a `--no-unicode-map` spec to builtin type_names to remove.
/// For `@security`, returns the security builtins.
/// For `@ascii-normalize`, returns all builtin type_names.
#[cfg(feature = "unicode-normalize")]
fn resolve_remove_builtins(spec: &str) -> Vec<&'static str> {
    match spec {
        "@security" => vec![
            "fullwidth_ascii",
            "math_latin_bold",
            "latin_ligatures",
        ],
        "@ascii-normalize" => vec![
            "fullwidth_ascii",
            "math_latin_bold",
            "latin_ligatures",
            "enclosed_circled_letters",
            "superscript_subscript",
        ],
        _ => {
            // Normalize CLI name (dashes) to type_name (underscores)
            let type_name = spec.replace('-', "_");
            // Check if it's a known builtin
            let all_builtins = [
                "fullwidth_ascii",
                "math_latin_bold",
                "latin_ligatures",
                "enclosed_circled_letters",
                "superscript_subscript",
            ];
            if all_builtins.contains(&type_name.as_str()) {
                vec![match type_name.as_str() {
                    "fullwidth_ascii" => "fullwidth_ascii",
                    "math_latin_bold" => "math_latin_bold",
                    "latin_ligatures" => "latin_ligatures",
                    "enclosed_circled_letters" => "enclosed_circled_letters",
                    "superscript_subscript" => "superscript_subscript",
                    _ => unreachable!(),
                }]
            } else {
                vec![]
            }
        }
    }
}

/// Build the UnicodeMap from CLI args.
///
/// Returns `Ok(Some(map))` when normalization is active,
/// `Ok(None)` when all builtins have been removed and no TOML loaded,
/// `Err(ExitCode)` on validation errors.
#[cfg(feature = "unicode-normalize")]
fn build_unicode_map(args: &Args) -> Result<Option<UnicodeMap>, ExitCode> {
    // Start with builtins.
    let mut map = UnicodeMap::builtin();

    // Apply removals (builtins).
    for spec in &args.no_unicode_map {
        // Remove builtins matching this spec.
        for type_name in resolve_remove_builtins(spec) {
            map.remove_set(type_name);
        }
    }

    // Collect shipped TOML files to load.
    let mut toml_names: Vec<String> = Vec::new();
    for spec in &args.unicode_map {
        if spec.starts_with('@') {
            let resolved = resolve_tag(spec);
            if resolved.is_empty() {
                eprintln!("distill-ansi: unknown tag '{spec}'");
                return Err(ExitCode::from(2));
            }
            for name in resolved {
                if !toml_names.contains(&name.to_string()) {
                    toml_names.push(name.to_string());
                }
            }
        } else if spec.contains('/') || spec.contains('.') {
            // Treat as file path — load directly.
            #[cfg(feature = "toml-config")]
            {
                let path = std::path::Path::new(spec);
                if let Err(e) = map.load_and_merge(path) {
                    eprintln!("distill-ansi: {e}");
                    return Err(ExitCode::from(2));
                }
            }
            #[cfg(not(feature = "toml-config"))]
            {
                eprintln!(
                    "distill-ansi: --unicode-map with file paths requires \
                     the toml-config feature"
                );
                return Err(ExitCode::from(2));
            }
        } else {
            // Treat as shipped file name.
            if !toml_names.contains(&spec.to_string()) {
                toml_names.push(spec.clone());
            }
        }
    }

    // Remove TOML names that are in --no-unicode-map.
    let mut remove_toml: Vec<String> = Vec::new();
    for spec in &args.no_unicode_map {
        if spec.starts_with('@') {
            for name in resolve_tag(spec) {
                remove_toml.push(name.to_string());
            }
        } else if !spec.contains('/') && !spec.contains('.') {
            remove_toml.push(spec.clone());
        }
    }
    toml_names.retain(|n| !remove_toml.contains(n));

    // Load shipped TOML files.
    #[cfg(feature = "toml-config")]
    for name in &toml_names {
        let path = format!("etc/unicode-mappings/{name}.toml");
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("distill-ansi: --unicode-map {name}: {e}");
                return Err(ExitCode::from(2));
            }
        };
        match strip_ansi::unicode_map::load_str(&text, path.clone()) {
            Ok(set) => {
                if let Err(dup) = map.merge_set(set) {
                    eprintln!(
                        "distill-ansi: --unicode-map: rejecting duplicate type \
                         {dup:?} (already loaded)"
                    );
                }
            }
            Err(e) => {
                eprintln!("distill-ansi: {e}");
                return Err(ExitCode::from(2));
            }
        }
    }

    #[cfg(not(feature = "toml-config"))]
    if !toml_names.is_empty() {
        eprintln!(
            "distill-ansi: --unicode-map with shipped files requires \
             the toml-config feature"
        );
        return Err(ExitCode::from(2));
    }

    // If everything was removed, return None (no normalization).
    if map.set_count() == 0 {
        Ok(None)
    } else {
        Ok(Some(map))
    }
}
