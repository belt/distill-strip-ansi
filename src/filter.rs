//! Filter configuration and stateless filter API for selective ANSI
//! escape sequence stripping.
//!
//! [`FilterConfig`] expresses a strip/preserve policy over sequence
//! groups ([`SeqGroup`]) and sub-kinds ([`SeqKind`]). The default
//! mode strips all escape sequences; builder methods selectively
//! preserve groups or individual sub-kinds.
//!
//! Stateless functions:
//! - [`filter_strip`] — byte slice → `Cow<[u8]>`
//! - [`filter_strip_str`] — UTF-8 string → `Cow<str>`
//! - [`filter_strip_into`] — byte slice → caller-provided `Vec<u8>`
//!
//! # Performance
//!
//! - Group membership: O(1) bit-field test
//! - Sub-kind membership: bounded linear scan (≤4 inline, spills to heap)
//! - `is_strip_all()` / `is_pass_all()`: O(1) predicate checks
//! - `filter_strip()` with StripAll delegates to SIMD-optimized `strip()`
//! - No-ESC and pass-all fast paths return borrowed (zero alloc)

#![forbid(unsafe_code)]

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use memchr::memchr;
use smallvec::SmallVec;

use crate::classifier::{ClassifyingParser, SeqAction, SeqKind, SeqGroup};
use crate::strip::strip;

/// All group bits set (bits 0..8 inclusive).
const ALL_GROUPS: u16 = 0x01FF;

// ── FilterMode ──────────────────────────────────────────────────────

/// Filtering mode for [`FilterConfig`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FilterMode {
    /// Strip all escape sequences (default).
    StripAll,
    /// Strip all except specified groups/sub-kinds.
    StripExcept,
}

// ── FilterConfig ────────────────────────────────────────────────────

/// Configuration expressing which ANSI escape sequences to strip or
/// preserve.
///
/// Construct via [`strip_all()`](Self::strip_all) or
/// [`pass_all()`](Self::pass_all), then refine with builder methods.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilterConfig {
    mode: FilterMode,
    /// Bit-field of preserved [`SeqGroup`] values (bit index = group
    /// discriminant).
    preserved: u16,
    /// Sub-kind-level preservation overrides.
    sub_preserved: SmallVec<[SeqKind; 4]>,
}

impl FilterConfig {
    /// Create a config that strips all escape sequences.
    #[inline]
    #[must_use]
    pub fn strip_all() -> Self {
        Self {
            mode: FilterMode::StripAll,
            preserved: 0,
            sub_preserved: SmallVec::new(),
        }
    }

    /// Create a config that preserves all escape sequences (strips
    /// nothing).
    #[inline]
    #[must_use]
    pub fn pass_all() -> Self {
        Self {
            mode: FilterMode::StripExcept,
            preserved: ALL_GROUPS,
            sub_preserved: SmallVec::new(),
        }
    }

    /// Preserve an entire sequence group (e.g. all CSI sequences).
    ///
    /// Switches mode to [`FilterMode::StripExcept`] and sets the
    /// corresponding group bit.
    #[inline]
    #[must_use]
    pub fn no_strip_group(mut self, group: SeqGroup) -> Self {
        self.mode = FilterMode::StripExcept;
        self.preserved |= 1 << (group as u16);
        self
    }

    /// Preserve a specific sequence sub-kind (e.g. `CsiSgr` only).
    ///
    /// Switches mode to [`FilterMode::StripExcept`], sets the group
    /// bit for the kind's parent group, and adds the kind to the
    /// sub-preserved list.
    #[inline]
    #[must_use]
    pub fn no_strip_kind(mut self, kind: SeqKind) -> Self {
        self.mode = FilterMode::StripExcept;
        self.preserved |= 1 << (kind.group() as u16);
        if !self.sub_preserved.contains(&kind) {
            self.sub_preserved.push(kind);
        }
        self
    }

    /// Returns `true` if `kind` should be stripped according to this
    /// config.
    ///
    /// Algorithm:
    /// - `StripAll` → always `true`
    /// - `StripExcept` → `false` if kind is in `sub_preserved`
    /// - `StripExcept` → `false` if kind's group bit is set in
    ///   `preserved`
    /// - Otherwise `true`
    #[inline]
    #[must_use]
    pub fn should_strip(&self, kind: SeqKind) -> bool {
        if self.mode == FilterMode::StripAll {
            return true;
        }
        if self.sub_preserved.contains(&kind) {
            return false;
        }
        if (self.preserved & (1 << (kind.group() as u16))) != 0 {
            return false;
        }
        true
    }

    /// Returns `true` when the config strips all sequences.
    #[inline]
    #[must_use]
    pub fn is_strip_all(&self) -> bool {
        self.mode == FilterMode::StripAll
    }

    /// Returns `true` when the config preserves all sequences.
    #[inline]
    #[must_use]
    pub fn is_pass_all(&self) -> bool {
        self.mode == FilterMode::StripExcept && self.preserved == ALL_GROUPS
    }
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self::strip_all()
    }
}

// ── Stateless filter API ────────────────────────────────────────────

/// Strip ANSI escape sequences from a byte slice according to `config`.
///
/// Fast paths (zero allocation):
/// - `StripAll` → delegates to [`strip()`]
/// - `pass_all()` → returns borrowed input
/// - No ESC byte → returns borrowed input
///
/// Otherwise filters per-sequence using [`ClassifyingParser`] with an
/// inline `[u8; 16]` buffer for retroactive emit (common sequences
/// fit inline; rare >16-byte sequences spill to a `Vec`).
#[inline]
#[must_use]
pub fn filter_strip<'a>(input: &'a [u8], config: &FilterConfig) -> Cow<'a, [u8]> {
    // Fast path: strip everything via SIMD-optimized strip().
    if config.is_strip_all() {
        return strip(input);
    }
    // Fast path: preserve everything.
    if config.is_pass_all() {
        return Cow::Borrowed(input);
    }
    // Fast path: no ESC byte in input.
    if memchr(0x1B, input).is_none() {
        return Cow::Borrowed(input);
    }

    let mut output = Vec::with_capacity(input.len());
    filter_strip_core(input, config, &mut output);
    Cow::Owned(output)
}

/// Strip ANSI escape sequences from a UTF-8 string according to `config`.
///
/// Wraps [`filter_strip`] with `Cow<str>` conversion. UTF-8 validity
/// is preserved: stripping only removes complete bytes from valid
/// UTF-8 input.
#[inline]
#[must_use]
pub fn filter_strip_str<'a>(input: &'a str, config: &FilterConfig) -> Cow<'a, str> {
    match filter_strip(input.as_bytes(), config) {
        Cow::Borrowed(b) => {
            // b is a subslice of input.as_bytes(), so it's valid UTF-8.
            // Recover the &str via pointer offset.
            let start = b.as_ptr() as usize - input.as_ptr() as usize;
            Cow::Borrowed(&input[start..start + b.len()])
        }
        Cow::Owned(v) => {
            // Input was valid UTF-8, stripping only removes bytes,
            // so output is valid UTF-8.
            Cow::Owned(String::from_utf8(v).expect("filter_strip preserves UTF-8"))
        }
    }
}

/// Strip ANSI escape sequences from a byte slice, appending to `out`.
///
/// Does not clear `out` first. Fast paths delegate to borrowed returns
/// or the existing `strip()` function.
#[inline]
pub fn filter_strip_into(input: &[u8], config: &FilterConfig, out: &mut Vec<u8>) {
    // Fast path: strip everything.
    if config.is_strip_all() {
        match strip(input) {
            Cow::Borrowed(b) => out.extend_from_slice(b),
            Cow::Owned(v) => out.extend_from_slice(&v),
        }
        return;
    }
    // Fast path: preserve everything.
    if config.is_pass_all() {
        out.extend_from_slice(input);
        return;
    }
    // Fast path: no ESC byte.
    if memchr(0x1B, input).is_none() {
        out.extend_from_slice(input);
        return;
    }

    filter_strip_core(input, config, out);
}

/// Core filtering logic shared by [`filter_strip`] and
/// [`filter_strip_into`].
///
/// Appends filtered bytes to `output`. Caller handles fast paths.
#[allow(unused_assignments)] // strip_current and seq_buf_len are set/read across loop iterations
fn filter_strip_core(input: &[u8], config: &FilterConfig, output: &mut Vec<u8>) {
    let mut cp = ClassifyingParser::new();
    let mut strip_current = false;
    let mut seq_buf: [u8; 16] = [0; 16];
    let mut seq_buf_len: usize = 0;
    let mut seq_spill: Vec<u8> = Vec::new();
    let mut remaining = input;

    while !remaining.is_empty() {
        // Bulk-copy content bytes up to the next ESC.
        let pos = memchr(0x1B, remaining).unwrap_or(remaining.len());
        output.extend_from_slice(&remaining[..pos]);
        remaining = &remaining[pos..];
        if remaining.is_empty() {
            break;
        }

        // Process escape sequence byte-by-byte.
        let mut i = 0;
        let mut broke_on_end = false;
        while i < remaining.len() {
            let action = cp.feed(remaining[i]);

            match action {
                SeqAction::StartSeq => {
                    strip_current = true; // tentative until kind is known
                    seq_buf_len = 0;
                    seq_buf[0] = remaining[i];
                    seq_buf_len = 1;
                }
                SeqAction::InSeq => {
                    if cp.current_kind() != SeqKind::Unknown {
                        strip_current = config.should_strip(cp.current_kind());
                    }
                    if seq_buf_len < 16 {
                        seq_buf[seq_buf_len] = remaining[i];
                        seq_buf_len += 1;
                    } else {
                        seq_spill.push(remaining[i]);
                    }
                }
                SeqAction::EndSeq => {
                    strip_current = config.should_strip(cp.current_kind());
                    if !strip_current {
                        output.extend_from_slice(&seq_buf[..seq_buf_len]);
                        if !seq_spill.is_empty() {
                            output.extend_from_slice(&seq_spill);
                        }
                        output.push(remaining[i]);
                    }
                    seq_spill.clear();
                    remaining = &remaining[i + 1..];
                    broke_on_end = true;
                    break;
                }
                SeqAction::Emit => {
                    output.push(remaining[i]);
                }
            }

            i += 1;
        }

        // If we exhausted remaining without hitting EndSeq, mark empty.
        if !broke_on_end {
            remaining = &[];
        }
    }
}

// ── FilterStream ────────────────────────────────────────────────────

/// Stateful streaming filter for chunked input with configurable
/// sequence preservation.
///
/// Analogous to [`StripStream`](crate::StripStream) but consults a
/// [`FilterConfig`] to decide per-sequence whether to strip or
/// preserve. The [`ClassifyingParser`] state carries across chunk
/// boundaries. Zero heap allocations — yields borrowed slices only.
///
/// Primary API: [`filter_slices`](FilterStream::filter_slices)
/// returns borrowed slices of the input.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FilterStream {
    cp: ClassifyingParser,
    strip_current: bool,
}

// Compile-time assertions: Send + Sync.
const _: () = {
    const fn assert_send<T: Send>() {}
    const fn assert_sync<T: Sync>() {}
    assert_send::<FilterStream>();
    assert_sync::<FilterStream>();
};

impl FilterStream {
    /// Create a new streaming filter in the ground state.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            cp: ClassifyingParser::new(),
            strip_current: true,
        }
    }

    /// Reset the parser to ground state.
    #[inline]
    pub fn reset(&mut self) {
        self.cp.reset();
        self.strip_current = true;
    }

    /// Discard any incomplete escape at stream end (same as reset).
    #[inline]
    pub fn finish(&mut self) {
        self.reset();
    }

    /// Returns `true` if the parser is in the ground state.
    #[inline]
    #[must_use]
    pub fn is_ground(&self) -> bool {
        self.cp.is_ground()
    }

    /// Filter a chunk, yielding borrowed slices of content and
    /// preserved sequence bytes.
    ///
    /// Call repeatedly with each input chunk. Concatenating all
    /// yielded slices across all calls produces the same result as
    /// [`filter_strip`] on the full concatenated input (modulo
    /// incomplete sequences at chunk boundaries, which are always
    /// stripped).
    pub fn filter_slices<'a>(
        &mut self,
        input: &'a [u8],
        config: &'a FilterConfig,
    ) -> FilterSlices<'a, '_> {
        let draining = !self.cp.is_ground();
        FilterSlices {
            remaining: input,
            cp: &mut self.cp,
            strip_current: &mut self.strip_current,
            config,
            draining,
        }
    }

    /// Filter a chunk, appending content and preserved sequence bytes
    /// to `out`.
    pub fn push(&mut self, input: &[u8], config: &FilterConfig, out: &mut Vec<u8>) {
        for slice in self.filter_slices(input, config) {
            out.extend_from_slice(slice);
        }
    }

    /// Filter a chunk, writing content and preserved sequence bytes
    /// to `writer`.
    #[cfg(feature = "std")]
    pub fn push_write(
        &mut self,
        input: &[u8],
        config: &FilterConfig,
        writer: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        for slice in self.filter_slices(input, config) {
            writer.write_all(slice)?;
        }
        Ok(())
    }
}

impl Default for FilterStream {
    fn default() -> Self {
        Self::new()
    }
}

// ── FilterSlices ────────────────────────────────────────────────────

/// Iterator yielding borrowed slices from a single chunk, preserving
/// or stripping escape sequences according to a [`FilterConfig`].
pub struct FilterSlices<'a, 'p> {
    remaining: &'a [u8],
    cp: &'p mut ClassifyingParser,
    strip_current: &'p mut bool,
    config: &'a FilterConfig,
    draining: bool,
}

impl<'a> Iterator for FilterSlices<'a, '_> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<&'a [u8]> {
        // Phase 1: drain mid-escape from previous chunk.
        if self.draining {
            self.draining = false;
            let mut i = 0;
            while i < self.remaining.len() {
                let action = self.cp.feed(self.remaining[i]);
                i += 1;
                match action {
                    SeqAction::EndSeq => {
                        // Sequence that started in a previous chunk is
                        // complete. We cannot yield it as a preserved
                        // slice because the start bytes are gone.
                        // Always strip cross-chunk sequences.
                        self.remaining = &self.remaining[i..];
                        return self.next_ground();
                    }
                    SeqAction::Emit => {
                        // Content byte emitted mid-sequence (abort).
                        self.remaining = &self.remaining[i - 1..];
                        return self.next_ground();
                    }
                    SeqAction::InSeq => {
                        // Update strip decision as kind becomes known.
                        if self.cp.current_kind() != SeqKind::Unknown {
                            *self.strip_current =
                                self.config.should_strip(self.cp.current_kind());
                        }
                    }
                    SeqAction::StartSeq => {
                        // Shouldn't happen during drain, but handle
                        // gracefully: treat as new sequence start.
                        *self.strip_current = true;
                    }
                }
            }
            // Entire chunk consumed by escape.
            self.remaining = &[];
            return None;
        }

        self.next_ground()
    }
}

impl<'a> FilterSlices<'a, '_> {
    fn next_ground(&mut self) -> Option<&'a [u8]> {
        loop {
            if self.remaining.is_empty() {
                return None;
            }

            // Fast path: pass-all yields entire remaining.
            if self.config.is_pass_all() {
                let slice = self.remaining;
                self.remaining = &[];
                return Some(slice);
            }

            let pos = memchr(0x1B, self.remaining);
            match pos {
                None => {
                    // All remaining is content.
                    let slice = self.remaining;
                    self.remaining = &[];
                    return Some(slice);
                }
                Some(0) => {
                    // ESC at start — process escape sequence.
                    let seq_start = self.remaining;
                    let mut i = 0;
                    while i < self.remaining.len() {
                        let action = self.cp.feed(self.remaining[i]);
                        i += 1;
                        match action {
                            SeqAction::StartSeq => {
                                *self.strip_current = true; // tentative
                            }
                            SeqAction::InSeq => {
                                if self.cp.current_kind() != SeqKind::Unknown {
                                    *self.strip_current =
                                        self.config.should_strip(self.cp.current_kind());
                                }
                            }
                            SeqAction::EndSeq => {
                                *self.strip_current =
                                    self.config.should_strip(self.cp.current_kind());
                                if !*self.strip_current {
                                    // Preserve: yield the full sequence.
                                    let seq_slice = &seq_start[..i];
                                    self.remaining = &self.remaining[i..];
                                    return Some(seq_slice);
                                }
                                // Strip: skip the sequence, continue loop.
                                self.remaining = &self.remaining[i..];
                                break;
                            }
                            SeqAction::Emit => {
                                // Content byte emitted mid-sequence
                                // (e.g. CAN/SUB abort). Yield it via
                                // next_ground on the remaining slice.
                                self.remaining = &self.remaining[i - 1..];
                                break;
                            }
                        }
                    }
                    // If we exhausted remaining without EndSeq, the
                    // sequence spans into the next chunk.
                    if i >= self.remaining.len() && !self.cp.is_ground() {
                        self.remaining = &[];
                        return None;
                    }
                    // Continue outer loop for next content/sequence.
                }
                Some(p) => {
                    // Content before ESC — yield it, advance to ESC.
                    let slice = &self.remaining[..p];
                    self.remaining = &self.remaining[p..];
                    return Some(slice);
                }
            }
        }
    }
}
