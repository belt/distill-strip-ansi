//! Unified metrics collector for stream analysis.
//!
//! Collects raw `u64` counts — no percentages, no formatting.
//! Consumers (CLI `--stats`, bench harness, Python generator)
//! derive higher-level metrics from the raw counts.
//!
//! Collection piggybacks on [`ClassifyingParser`]: two increments
//! per sequence end, one per content byte. The `by_kind` array is
//! indexed by `SeqKind as u8` — same table pattern as the parser.

#![forbid(unsafe_code)]

use alloc::string::String;
use alloc::vec::Vec;

use crate::classifier::{ClassifyingParser, SeqAction, SeqKind};

/// Number of `SeqKind` variants (max discriminant + 1).
const NUM_KINDS: usize = 18;

/// Per-kind count: how many sequences and how many bytes.
#[derive(Clone, Debug, Default)]
pub struct KindCount {
    pub count: u64,
    pub bytes: u64,
}

/// A detected threat — first-class citizen in the stats output.
#[derive(Clone, Debug)]
pub struct ThreatHit {
    pub type_name: &'static str,
    pub kind: SeqKind,
    pub offset: u64,
    pub len: u64,
}

/// Raw stream metrics. All fields are `u64` counts.
#[derive(Clone, Debug)]
pub struct Stats {
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub lines: u64,
    pub words: u64,
    pub chars: u64,
    pub sequences: u64,
    pub seq_bytes: u64,
    pub plain_bytes: u64,
    pub by_kind: [KindCount; NUM_KINDS],
    pub threats: Vec<ThreatHit>,
    seq_len: u64,
    seq_start_offset: u64,
    in_word: bool,
}

impl Default for Stats {
    fn default() -> Self { Self::new() }
}

impl Stats {
    #[must_use]
    pub fn new() -> Self {
        Self {
            bytes_in: 0, bytes_out: 0, lines: 0, words: 0, chars: 0,
            sequences: 0, seq_bytes: 0, plain_bytes: 0,
            by_kind: Default::default(), threats: Vec::new(),
            seq_len: 0, seq_start_offset: 0, in_word: false,
        }
    }

    /// Record one classified action from `ClassifyingParser::feed`.
    #[inline]
    pub fn record(&mut self, byte: u8, action: SeqAction, kind: SeqKind) {
        self.bytes_in += 1;
        match action {
            SeqAction::Emit => {
                self.bytes_out += 1;
                self.plain_bytes += 1;
                if byte == b'\n' { self.lines += 1; }
                let is_ws = byte == b' ' || byte == b'\t' || byte == b'\n'
                    || byte == b'\r' || byte == 0x0B || byte == 0x0C;
                if !is_ws {
                    if !self.in_word { self.words += 1; }
                    self.in_word = true;
                } else {
                    self.in_word = false;
                }
                if byte & 0xC0 != 0x80 { self.chars += 1; }
            }
            SeqAction::StartSeq => {
                self.seq_len = 1;
                self.seq_start_offset = self.bytes_in - 1;
            }
            SeqAction::InSeq => {
                self.seq_len += 1;
            }
            SeqAction::EndSeq => {
                self.seq_len += 1;
                self.sequences += 1;
                self.seq_bytes += self.seq_len;
                let idx = kind as u8 as usize;
                if idx < NUM_KINDS {
                    self.by_kind[idx].count += 1;
                    self.by_kind[idx].bytes += self.seq_len;
                }
                self.seq_len = 0;
            }
        }
    }

    /// Check a completed sequence for threats. Call on EndSeq after record().
    pub fn check_threat(&mut self, detail: &crate::classifier::SeqDetail) {
        if let Some(type_name) = classify_threat(detail) {
            self.threats.push(ThreatHit {
                type_name,
                kind: detail.kind,
                offset: self.seq_start_offset,
                len: self.bytes_in - self.seq_start_offset,
            });
        }
    }

    /// Analyze a complete byte slice with threat detection.
    #[must_use]
    pub fn from_bytes(input: &[u8]) -> Self {
        let mut stats = Self::new();
        let mut cp = ClassifyingParser::new();
        for &b in input {
            let action = cp.feed(b);
            let kind = cp.current_kind();
            stats.record(b, action, kind);
            if action == SeqAction::EndSeq {
                stats.check_threat(&cp.detail());
            }
        }
        stats
    }

    /// Serialize to JSON. No serde — hand-rolled for minimal binary size.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn to_json(&self) -> String {
        use alloc::fmt::Write;
        let mut s = String::with_capacity(512);
        let _ = write!(s,
            concat!(
                "{{\"bytes_in\":{},\"bytes_out\":{},",
                "\"lines\":{},\"words\":{},\"chars\":{},",
                "\"sequences\":{},\"seq_bytes\":{},\"plain_bytes\":{},",
                "\"by_kind\":{{"
            ),
            self.bytes_in, self.bytes_out,
            self.lines, self.words, self.chars,
            self.sequences, self.seq_bytes, self.plain_bytes,
        );
        let mut first = true;
        for (i, kc) in self.by_kind.iter().enumerate() {
            if kc.count == 0 && kc.bytes == 0 { continue; }
            if !first { s.push(','); }
            first = false;
            let _ = write!(s, "\"{}\":{{\"count\":{},\"bytes\":{}}}",
                KIND_NAMES[i], kc.count, kc.bytes);
        }
        s.push_str("},\"threats\":[");
        for (i, t) in self.threats.iter().enumerate() {
            if i > 0 { s.push(','); }
            let _ = write!(s,
                "{{\"type\":\"{}\",\"kind\":\"{}\",\"offset\":{},\"len\":{}}}",
                t.type_name, KIND_NAMES[t.kind as u8 as usize], t.offset, t.len);
        }
        s.push_str("]}");
        s
    }
}

// ── SeqKind name table ──────────────────────────────────────────────

static KIND_NAMES: [&str; NUM_KINDS] = [
    "csi_sgr", "csi_cursor", "csi_erase", "csi_scroll",
    "csi_mode", "csi_device_status", "csi_window", "csi_other",
    "osc", "dcs", "apc", "pm", "sos", "ss2", "ss3", "fe",
    "unknown", "csi_query",
];

// ── Builtin threat classification ───────────────────────────────────

fn classify_threat(detail: &crate::classifier::SeqDetail) -> Option<&'static str> {
    match detail.kind {
        SeqKind::Dcs => {
            if detail.dcs_is_query { Some("dcs_decrqss") }
            else { Some("dcs_other") }
        }
        SeqKind::Osc => {
            if detail.osc_number == 50 { Some("osc_50") }
            else if detail.osc_type == crate::classifier::OscType::Clipboard { Some("osc_clipboard") }
            else { None }
        }
        SeqKind::CsiQuery => {
            if detail.first_param == 21 { Some("csi_21t") }
            else if detail.first_param == 6 { Some("csi_6n") }
            else { None }
        }
        _ => None,
    }
}
