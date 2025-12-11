//! Test input generation and fixture selection — shared across all bench binaries.
//!
//! Fixture-bucketed strategy: for each target size, scan
//! `tests/fixtures/*.raw.txt` for a real-world fixture that fits.
//! If found, use it (all crates bench the same real data).
//! If not, generate synthetic data in RAM.

use std::fmt;
use std::path::{Path, PathBuf};

use strip_ansi::Stats;

// ── Public types ────────────────────────────────────────────────────

/// Where the benchmark input came from.
#[derive(Clone, Debug)]
pub enum InputSource {
    /// Real fixture file from `tests/fixtures/`.
    Fixture(String),
    /// Generated in RAM (synthetic).
    Generated,
}

/// Metadata about a benchmark input.
#[derive(Clone, Debug)]
pub struct InputMeta {
    pub source: InputSource,
    pub stats: Stats,
}

impl fmt::Display for InputMeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let src = match &self.source {
            InputSource::Fixture(name) => name.as_str(),
            InputSource::Generated => "generated",
        };
        let s = &self.stats;
        let total = s.bytes_in.max(1) as f32;
        write!(
            f,
            "{} ({} bytes, {} lines, {:.0}% SGR, {:.0}% OSC, {:.0}% DCS, {:.0}% plain)",
            src,
            s.bytes_in,
            s.lines,
            s.by_kind[0].bytes as f32 / total * 100.0,  // CsiSgr = 0
            s.by_kind[8].bytes as f32 / total * 100.0,  // Osc = 8
            s.by_kind[9].bytes as f32 / total * 100.0,  // Dcs = 9
            s.plain_bytes as f32 / total * 100.0,
        )
    }
}

impl InputMeta {
    /// Human-readable display with verbose sequence type names.
    pub fn verbose_display(&self) -> String {
        let src = match &self.source {
            InputSource::Fixture(name) => name.clone(),
            InputSource::Generated => "generated".to_string(),
        };
        let s = &self.stats;
        let total = s.bytes_in.max(1) as f32;
        let mut parts = Vec::new();
        let sgr_pct = s.by_kind[0].bytes as f32 / total * 100.0;
        let osc_pct = s.by_kind[8].bytes as f32 / total * 100.0;
        let dcs_pct = s.by_kind[9].bytes as f32 / total * 100.0;
        let plain_pct = s.plain_bytes as f32 / total * 100.0;
        if sgr_pct > 0.5 { parts.push(format!("{sgr_pct:.0}% colors/styles")); }
        if osc_pct > 0.5 { parts.push(format!("{osc_pct:.0}% hyperlinks/titles")); }
        if dcs_pct > 0.5 { parts.push(format!("{dcs_pct:.0}% device control")); }
        if plain_pct > 0.5 { parts.push(format!("{plain_pct:.0}% plain text")); }
        format!(
            "{src} — {size} bytes, {lines} lines — {parts}",
            size = s.bytes_in,
            lines = s.lines,
            parts = parts.join(", "),
        )
    }
}

// ── Fixture selection ───────────────────────────────────────────────

/// Select the best input for a target size: fixture if available,
/// generated otherwise.
///
/// Fixture matching: finds the fixture whose size is closest to
/// `target_size`, within a 0.25×–4× tolerance band. Only considers
/// fixtures that contain at least one ESC byte (0x1B) — plain-text
/// fixtures are skipped since they'd only exercise the fast path.
#[must_use]
pub fn select_input(target_size: usize) -> (Vec<u8>, InputMeta) {
    let fixtures = scan_fixtures();

    let lo = target_size / 4;
    let hi = target_size.saturating_mul(4);

    let best = fixtures
        .iter()
        .filter(|(_, sz, has_ansi)| *has_ansi && *sz >= lo && *sz <= hi)
        .min_by_key(|(_, sz, _)| (*sz as isize - target_size as isize).unsigned_abs());

    if let Some((name, _, _)) = best {
        if let Some(data) = load_fixture(name) {
            let meta = InputMeta {
                source: InputSource::Fixture(name.clone()),
                stats: Stats::from_bytes(&data),
            };
            return (data, meta);
        }
    }

    // No ANSI fixture fits — generate synthetic dirty input.
    let data = dirty_input(target_size);
    let meta = InputMeta {
        source: InputSource::Generated,
        stats: Stats::from_bytes(&data),
    };
    (data, meta)
}

/// Scan `tests/fixtures/*.raw.txt` and return (filename, size, has_ansi).
///
/// Reads each file to check for ESC bytes. Only fixtures with ANSI
/// sequences are useful for dirty benchmarks.
fn scan_fixtures() -> Vec<(String, usize, bool)> {
    let mut results = Vec::new();
    for dir in &["tests/fixtures", "../../tests/fixtures"] {
        let path = Path::new(dir);
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.ends_with(".raw.txt") {
                    if let Ok(data) = std::fs::read(entry.path()) {
                        let has_ansi = data.contains(&0x1B);
                        results.push((fname, data.len(), has_ansi));
                    }
                }
            }
            if !results.is_empty() {
                break;
            }
        }
    }
    results
}

// ── Sequence analysis (now uses Stats::from_bytes) ──────────────────

/// Public wrapper for analyzing generated inputs (no fixture source).
pub fn analyze_pub(data: &[u8]) -> InputMeta {
    InputMeta {
        source: InputSource::Generated,
        stats: Stats::from_bytes(data),
    }
}

// ── Existing generators (kept for fallback + clean benchmarks) ──────

/// Generate clean input (no ANSI) of the given size.
///
/// Produces repeating ASCII lines — exercises the memchr fast path.
#[must_use]
pub fn clean_input(size: usize) -> Vec<u8> {
    let line = b"The quick brown fox jumps over the lazy dog.\n";
    let mut buf = Vec::with_capacity(size);
    while buf.len() < size {
        let remaining = size - buf.len();
        buf.extend_from_slice(&line[..remaining.min(line.len())]);
    }
    buf.truncate(size);
    buf
}

/// Generate dirty input (~20% ANSI sequences) of the given size.
///
/// Produces colored build-log-style output — representative of
/// real-world CI/CD pipeline captures.
#[must_use]
pub fn dirty_input(size: usize) -> Vec<u8> {
    let lines: &[&[u8]] = &[
        b"\x1b[0m\x1b[1m\x1b[32m   Compiling\x1b[0m memchr v2.7.1\n",
        b"\x1b[0m\x1b[1m\x1b[32m   Compiling\x1b[0m serde v1.0.203\n",
        b"\x1b[0m\x1b[1m\x1b[32m   Compiling\x1b[0m regex v1.10.5\n",
        b"\x1b[0m\x1b[1m\x1b[33m    warning\x1b[0m: unused import `std::io`\n",
        b"\x1b[0m\x1b[1m\x1b[31merror[E0308]\x1b[0m: mismatched types\n",
        b"  \x1b[0m\x1b[1m\x1b[36m-->\x1b[0m src/main.rs:42:5\n",
        b"   \x1b[0m\x1b[1m\x1b[36m|\x1b[0m\n",
        b"42 \x1b[0m\x1b[1m\x1b[36m|\x1b[0m     let x: u32 = \"hello\";\n",
    ];
    let mut buf = Vec::with_capacity(size);
    let mut i = 0;
    while buf.len() < size {
        let remaining = size - buf.len();
        let line = lines[i % lines.len()];
        buf.extend_from_slice(&line[..remaining.min(line.len())]);
        i += 1;
    }
    buf.truncate(size);
    buf
}

/// Load a fixture file from `tests/fixtures/`.
///
/// Returns `None` if the file doesn't exist (bench runs from
/// various working directories).
#[must_use]
pub fn load_fixture(name: &str) -> Option<Vec<u8>> {
    let candidates = [
        PathBuf::from(format!("tests/fixtures/{name}")),
        PathBuf::from(format!("../../tests/fixtures/{name}")),
    ];
    for path in &candidates {
        if let Ok(data) = std::fs::read(path) {
            return Some(data);
        }
    }
    None
}

/// Format a byte count for human display.
#[must_use]
pub fn fmt_bytes(b: u64) -> String {
    if b >= 1024 * 1024 * 1024 {
        format!("{:.1} GiB", b as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if b >= 1024 * 1024 {
        format!("{:.1} MiB", b as f64 / (1024.0 * 1024.0))
    } else if b >= 1024 {
        format!("{:.1}K", b as f64 / 1024.0)
    } else {
        format!("{b}B")
    }
}
