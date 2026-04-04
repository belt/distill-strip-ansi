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

#[cfg(feature = "filter")]
use strip_ansi::TerminalPreset;

#[cfg(feature = "filter")]
use strip_ansi::{ClassifyingParser, SeqAction, SeqDetail};

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
        #[cfg(feature = "filter")]
        if args.check_threats {
            return run_check_threats_mode(reader, &args);
        }
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

/// Build a [`FilterConfig`] from CLI flags, preset, and auto-detection.
///
/// Priority (highest to lowest):
/// 1. `--preset <name>` — explicit preset, overrides detection
/// 2. Auto-detection — probe stdout capabilities (default)
/// 3. `--config` TOML — loaded first, then overlaid by above
///
/// `--no-strip-*` flags are always applied last (additive overlay).
#[cfg(feature = "filter")]
fn build_filter_config(args: &Args) -> Result<FilterConfig, ExitCode> {
    // Start from TOML if --config is provided.
    #[cfg(feature = "toml-config")]
    let base_from_toml = if let Some(ref path) = args.config {
        let toml = match strip_ansi::StripAnsiConfig::from_file(std::path::Path::new(path)) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("strip-ansi: --config {path}: {e}");
                return Err(ExitCode::from(1));
            }
        };
        match toml.to_filter_config() {
            Ok(fc) => Some(fc),
            Err(e) => {
                eprintln!("strip-ansi: --config {path}: {e}");
                return Err(ExitCode::from(1));
            }
        }
    } else {
        None
    };

    // Resolve base config: --preset > auto-detect > TOML > strip_all.
    let mut config = if let Some(ref name) = args.preset {
        match TerminalPreset::from_name(name) {
            Some(preset) => {
                // Unsafe gate: presets above sanitize require --unsafe.
                if preset.requires_unsafe() && !args.r#unsafe {
                    eprintln!(
                        "strip-ansi: --preset {} preserves dangerous sequences \
                         (OSC 50, CSI 21t). Use --unsafe to acknowledge the risk.",
                        preset.name(),
                    );
                    return Err(ExitCode::from(1));
                }
                preset.to_filter_config()
            }
            None => {
                eprintln!(
                    "strip-ansi: unknown preset '{name}'. \
                     Valid: {valid}",
                    valid = TerminalPreset::ALL_NAMES.join(", "),
                );
                return Err(ExitCode::from(1));
            }
        }
    } else {
        // Auto-detect when no --preset is given.
        #[cfg(feature = "terminal-detect")]
        {
            strip_ansi::detect_preset().to_filter_config()
        }
        #[cfg(not(feature = "terminal-detect"))]
        {
            #[cfg(feature = "toml-config")]
            {
                base_from_toml.clone().unwrap_or_else(FilterConfig::strip_all)
            }
            #[cfg(not(feature = "toml-config"))]
            {
                FilterConfig::strip_all()
            }
        }
    };

    // If TOML was loaded and no --preset was given, merge TOML as base
    // (auto-detect takes precedence, but TOML --no-strip entries are
    // additive when auto-detect is not available).
    #[cfg(feature = "toml-config")]
    if args.preset.is_none() {
        if let Some(ref _toml_config) = base_from_toml {
            // When auto-detect is active, it already chose the right
            // base. TOML no_strip entries are not merged on top to
            // avoid surprising behavior. Use --preset to override.
        }
    }

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

// ── Threat detection ────────────────────────────────────────────────

// ── Threat reporting ─────────────────────────────────────────────

/// Information about a detected threat, used for structured output.
#[cfg(feature = "filter")]
struct ThreatInfo<'a> {
    threat_type: &'a str,
    line: u64,
    pos: u64,
    offset: u64,
    len: u64,
    cve: Option<&'a str>,
    reference: Option<&'a str>,
}

/// Format a threat as a structured key=value string.
///
/// Output: `[strip-ansi:threat] type=X line=N pos=N offset=N len=N [cve=X] [ref=URI]`
#[cfg(feature = "filter")]
fn format_threat(info: &ThreatInfo<'_>) -> String {
    let mut s = format!(
        "[strip-ansi:threat] type={} line={} pos={} offset={} len={}",
        info.threat_type, info.line, info.pos, info.offset, info.len
    );
    if let Some(cve) = info.cve {
        s.push_str(&format!(" cve={}", cve));
    }
    if let Some(r) = info.reference {
        s.push_str(&format!(" ref={}", r));
    }
    s
}

/// Look up CVE and reference URI for a built-in threat type.
///
/// Returns `(cve, ref)` — both `None` when no CVE is known.
#[cfg(feature = "filter")]
fn lookup_cve(threat_type: &str) -> (Option<&'static str>, Option<&'static str>) {
    match threat_type {
        "dcs_decrqss" => (
            Some("CVE-2008-2383"),
            Some("https://nvd.nist.gov/vuln/detail/CVE-2008-2383"),
        ),
        "osc_50" => (
            Some("CVE-2022-45063"),
            Some("https://nvd.nist.gov/vuln/detail/CVE-2022-45063"),
        ),
        "csi_21t" => (
            Some("CVE-2003-0063"),
            Some("https://nvd.nist.gov/vuln/detail/CVE-2003-0063"),
        ),
        _ => (None, None),
    }
}

/// Simple line/position tracker for byte-by-byte scanning.
///
/// Both `line` and `pos` are 1-indexed. Newline (0x0A) increments
/// `line` and resets `pos` to 1.
#[cfg(feature = "filter")]
struct LineTracker {
    line: u64,
    pos: u64,
}

#[cfg(feature = "filter")]
impl LineTracker {
    fn new() -> Self {
        Self { line: 1, pos: 1 }
    }

    /// Advance the tracker by one byte. Call AFTER processing the byte.
    fn advance(&mut self, byte: u8) {
        if byte == 0x0A {
            self.line += 1;
            self.pos = 1;
        } else {
            self.pos += 1;
        }
    }
}

/// Check if a classified sequence detail matches a known echoback threat pattern.
///
/// Returns the threat type string if the detail matches, `None` otherwise.
#[cfg(feature = "filter")]
fn is_threat(detail: &SeqDetail) -> Option<&'static str> {
    match detail.kind {
        SeqKind::Dcs => {
            if detail.dcs_is_query {
                Some("dcs_decrqss")
            } else {
                Some("dcs_other")
            }
        }
        SeqKind::Osc => {
            if detail.osc_number == 50 {
                Some("osc_50")
            } else if detail.osc_type == strip_ansi::OscType::Clipboard {
                Some("osc_clipboard")
            } else {
                None
            }
        }
        SeqKind::CsiQuery => {
            if detail.first_param == 21 {
                Some("csi_21t")
            } else if detail.first_param == 6 {
                Some("csi_6n")
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Run --check-threats mode: scan input for echoback vectors.
///
/// In fail mode (default): scan entire input, report all threats to stderr, exit 77 if any found.
/// In strip mode: filter with sanitize policy, report threats to stderr, exit 0.
#[cfg(feature = "filter")]
fn run_check_threats_mode(reader: Box<dyn BufRead>, args: &Args) -> ExitCode {
    use cli::OnThreatMode;

    // Load external threat DB if --threat-db is provided.
    #[cfg(feature = "toml-config")]
    let threat_db = if let Some(ref path) = args.threat_db {
        match strip_ansi::ThreatDb::from_file(std::path::Path::new(path)) {
            Ok(db) => Some(db),
            Err(e) => {
                eprintln!("strip-ansi: --threat-db {path}: {e}");
                return ExitCode::from(1);
            }
        }
    } else {
        None
    };

    match args.on_threat {
        OnThreatMode::Fail => {
            #[cfg(feature = "toml-config")]
            {
                run_check_threats_fail(reader, args, threat_db.as_ref())
            }
            #[cfg(not(feature = "toml-config"))]
            {
                run_check_threats_fail(reader, args)
            }
        }
        OnThreatMode::Strip => {
            #[cfg(feature = "toml-config")]
            {
                run_check_threats_strip(reader, args, threat_db.as_ref())
            }
            #[cfg(not(feature = "toml-config"))]
            {
                run_check_threats_strip(reader, args)
            }
        }
    }
}

/// Fail mode: scan input byte-by-byte, report all threats, exit 77 if any found.
#[cfg(feature = "filter")]
fn run_check_threats_fail(
    mut reader: Box<dyn BufRead>,
    args: &Args,
    #[cfg(feature = "toml-config")] threat_db: Option<&strip_ansi::ThreatDb>,
) -> ExitCode {
    let mut cp = ClassifyingParser::new();
    let mut threats_found = false;
    let mut byte_offset: u64 = 0;
    let mut seq_start_offset: u64 = 0;
    let mut seq_start_line: u64 = 1;
    let mut seq_start_pos: u64 = 1;
    let mut tracker = LineTracker::new();
    let max_size = args.max_size.unwrap_or(u64::MAX);

    loop {
        let budget = max_size.saturating_sub(byte_offset);
        if budget == 0 {
            break;
        }

        let buf = match reader.fill_buf() {
            Ok(b) => b,
            Err(e) => return handle_io_error(e),
        };
        if buf.is_empty() {
            break;
        }

        let check_len = buf.len().min(budget as usize);

        for &b in &buf[..check_len] {
            let action = cp.feed(b);
            match action {
                SeqAction::StartSeq => {
                    seq_start_offset = byte_offset;
                    seq_start_line = tracker.line;
                    seq_start_pos = tracker.pos;
                }
                SeqAction::EndSeq => {
                    let detail = cp.detail();
                    #[cfg(feature = "toml-config")]
                    let matched = if let Some(db) = threat_db {
                        db.classify(&detail)
                    } else {
                        None
                    };
                    #[cfg(feature = "toml-config")]
                    if let Some(entry) = matched {
                        threats_found = true;
                        if !args.no_threat_report {
                            let len = byte_offset - seq_start_offset + 1;
                            let info = ThreatInfo {
                                threat_type: &entry.type_name,
                                line: seq_start_line,
                                pos: seq_start_pos,
                                offset: seq_start_offset,
                                len,
                                cve: entry.cve.as_deref(),
                                reference: entry.reference.as_deref(),
                            };
                            eprintln!("{}", format_threat(&info));
                        }
                    } else if matched.is_none() && threat_db.is_none() {
                        // No ThreatDb — fall back to hardcoded is_threat.
                        if let Some(threat_type) = is_threat(&detail) {
                            threats_found = true;
                            if !args.no_threat_report {
                                let (cve, reference) = lookup_cve(threat_type);
                                let len = byte_offset - seq_start_offset + 1;
                                let info = ThreatInfo {
                                    threat_type,
                                    line: seq_start_line,
                                    pos: seq_start_pos,
                                    offset: seq_start_offset,
                                    len,
                                    cve,
                                    reference,
                                };
                                eprintln!("{}", format_threat(&info));
                            }
                        }
                    }
                    #[cfg(not(feature = "toml-config"))]
                    if let Some(threat_type) = is_threat(&detail) {
                        threats_found = true;
                        if !args.no_threat_report {
                            let (cve, reference) = lookup_cve(threat_type);
                            let len = byte_offset - seq_start_offset + 1;
                            let info = ThreatInfo {
                                threat_type,
                                line: seq_start_line,
                                pos: seq_start_pos,
                                offset: seq_start_offset,
                                len,
                                cve,
                                reference,
                            };
                            eprintln!("{}", format_threat(&info));
                        }
                    }
                }
                _ => {}
            }
            tracker.advance(b);
            byte_offset += 1;
        }

        reader.consume(check_len);
    }

    if threats_found {
        ExitCode::from(77)
    } else {
        ExitCode::SUCCESS
    }
}

/// Strip mode: filter with sanitize policy, report threats to stderr, exit 0.
#[cfg(feature = "filter")]
fn run_check_threats_strip(
    mut reader: Box<dyn BufRead>,
    args: &Args,
    #[cfg(feature = "toml-config")] threat_db: Option<&strip_ansi::ThreatDb>,
) -> ExitCode {
    let filter_config = TerminalPreset::Sanitize.to_filter_config();

    let mut writer = match open_writer(args) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("strip-ansi: {e}");
            return ExitCode::from(1);
        }
    };

    // We use a ClassifyingParser to detect threats while also using
    // FilterStream for the actual stripping.
    let mut cp = ClassifyingParser::new();
    let mut filter_stream = FilterStream::new();
    let mut byte_offset: u64 = 0;
    let mut seq_start_offset: u64 = 0;
    let mut seq_start_line: u64 = 1;
    let mut seq_start_pos: u64 = 1;
    let mut tracker = LineTracker::new();
    let max_size = args.max_size.unwrap_or(u64::MAX);

    let mut buf = [0u8; 32 * 1024];

    loop {
        let budget = max_size.saturating_sub(byte_offset);
        if budget == 0 {
            break;
        }
        let read_len = buf.len().min(budget as usize);

        let n = match reader.read(&mut buf[..read_len]) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => return handle_io_error(e),
        };

        // Scan for threats in this chunk.
        for &b in &buf[..n] {
            let action = cp.feed(b);
            match action {
                SeqAction::StartSeq => {
                    seq_start_offset = byte_offset;
                    seq_start_line = tracker.line;
                    seq_start_pos = tracker.pos;
                }
                SeqAction::EndSeq => {
                    let detail = cp.detail();
                    #[cfg(feature = "toml-config")]
                    let matched = if let Some(db) = threat_db {
                        db.classify(&detail)
                    } else {
                        None
                    };
                    #[cfg(feature = "toml-config")]
                    if let Some(entry) = matched {
                        if !args.no_threat_report {
                            let len = byte_offset - seq_start_offset + 1;
                            let info = ThreatInfo {
                                threat_type: &entry.type_name,
                                line: seq_start_line,
                                pos: seq_start_pos,
                                offset: seq_start_offset,
                                len,
                                cve: entry.cve.as_deref(),
                                reference: entry.reference.as_deref(),
                            };
                            eprintln!("{}", format_threat(&info));
                        }
                    } else if matched.is_none() && threat_db.is_none() {
                        if let Some(threat_type) = is_threat(&detail) {
                            if !args.no_threat_report {
                                let (cve, reference) = lookup_cve(threat_type);
                                let len = byte_offset - seq_start_offset + 1;
                                let info = ThreatInfo {
                                    threat_type,
                                    line: seq_start_line,
                                    pos: seq_start_pos,
                                    offset: seq_start_offset,
                                    len,
                                    cve,
                                    reference,
                                };
                                eprintln!("{}", format_threat(&info));
                            }
                        }
                    }
                    #[cfg(not(feature = "toml-config"))]
                    if let Some(threat_type) = is_threat(&detail) {
                        if !args.no_threat_report {
                            let (cve, reference) = lookup_cve(threat_type);
                            let len = byte_offset - seq_start_offset + 1;
                            let info = ThreatInfo {
                                threat_type,
                                line: seq_start_line,
                                pos: seq_start_pos,
                                offset: seq_start_offset,
                                len,
                                cve,
                                reference,
                            };
                            eprintln!("{}", format_threat(&info));
                        }
                    }
                }
                _ => {}
            }
            tracker.advance(b);
            byte_offset += 1;
        }

        // Filter and write clean output.
        for slice in filter_stream.filter_slices(&buf[..n], &filter_config) {
            if let Err(e) = writer.write_all(slice) {
                return handle_io_error(e);
            }
        }
    }

    filter_stream.finish();

    if let Err(e) = writer.flush() {
        return handle_io_error(e);
    }

    ExitCode::SUCCESS
}
