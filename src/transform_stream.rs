//! Streaming color transform for chunked input.
//!
//! [`TransformStream`] is analogous to [`FilterStream`](crate::FilterStream)
//! but rewrites SGR color sequences instead of just stripping or
//! preserving them. Yields [`TransformSlice`] — either borrowed
//! content or owned rewritten bytes.
//!
//! Gated behind the `transform` feature.

#![forbid(unsafe_code)]

use alloc::vec::Vec;

use memchr::memchr;
use smallvec::SmallVec;

use crate::classifier::{ClassifyingParser, SeqAction, SeqKind, SgrContent};
use crate::downgrade::ColorDepth;
use crate::sgr_rewrite::rewrite_sgr_direct;

/// A slice yielded by [`TransformStream`]: either borrowed from the
/// input or owned (rewritten SGR sequence).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransformSlice<'a> {
    /// Borrowed content or preserved (non-rewritten) sequence bytes.
    Borrowed(&'a [u8]),
    /// Owned bytes from a rewritten SGR sequence.
    Owned(SmallVec<[u8; 32]>),
}

impl<'a> TransformSlice<'a> {
    /// View the slice contents as a byte slice.
    #[inline]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Borrowed(b) => b,
            Self::Owned(v) => v.as_slice(),
        }
    }
}

/// Configuration for the transform stream.
#[derive(Clone, Debug)]
pub struct TransformConfig {
    /// Target color depth for SGR rewriting.
    pub depth: ColorDepth,
}

impl TransformConfig {
    /// Create a new transform config.
    #[must_use]
    pub fn new(depth: ColorDepth) -> Self {
        Self { depth }
    }

    /// Returns `true` if no transform is needed (pass-through).
    #[inline]
    #[must_use]
    pub fn is_passthrough(&self) -> bool {
        self.depth == ColorDepth::Truecolor
    }
}

/// Stateful streaming transform for chunked input.
///
/// Carries [`ClassifyingParser`] state across chunk boundaries.
/// SGR sequences with color content are rewritten to the target
/// depth; all other bytes pass through unchanged.
///
/// Primary API: [`transform_slices`](Self::transform_slices) returns
/// an iterator of [`TransformSlice`].
pub struct TransformStream {
    cp: ClassifyingParser,
    /// Buffer for accumulating sequence bytes within a chunk.
    seq_buf: Vec<u8>,
    in_seq: bool,
}

impl TransformStream {
    /// Create a new transform stream in the ground state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cp: ClassifyingParser::new(),
            seq_buf: Vec::new(),
            in_seq: false,
        }
    }

    /// Reset the parser to ground state.
    pub fn reset(&mut self) {
        self.cp.reset();
        self.seq_buf.clear();
        self.in_seq = false;
    }

    /// Discard any incomplete escape at stream end.
    pub fn finish(&mut self) {
        self.reset();
    }

    /// Returns `true` if the parser is in the ground state.
    #[must_use]
    pub fn is_ground(&self) -> bool {
        self.cp.is_ground()
    }

    /// Transform a chunk, yielding slices of content and
    /// transformed/preserved sequence bytes.
    pub fn transform_slices<'a>(
        &mut self,
        input: &'a [u8],
        config: &'a TransformConfig,
    ) -> TransformSlices<'a, '_> {
        let draining = !self.cp.is_ground();
        TransformSlices {
            remaining: input,
            stream: self,
            config,
            draining,
        }
    }

    /// Transform a chunk, appending all output bytes to `out`.
    pub fn push(&mut self, input: &[u8], config: &TransformConfig, out: &mut Vec<u8>) {
        for slice in self.transform_slices(input, config) {
            out.extend_from_slice(slice.as_bytes());
        }
    }

    /// Transform a chunk, writing all output bytes to `writer`.
    #[cfg(feature = "std")]
    pub fn push_write(
        &mut self,
        input: &[u8],
        config: &TransformConfig,
        writer: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        for slice in self.transform_slices(input, config) {
            writer.write_all(slice.as_bytes())?;
        }
        Ok(())
    }
}

impl Default for TransformStream {
    fn default() -> Self {
        Self::new()
    }
}

/// Iterator yielding [`TransformSlice`] from a single chunk.
pub struct TransformSlices<'a, 'p> {
    remaining: &'a [u8],
    stream: &'p mut TransformStream,
    config: &'a TransformConfig,
    draining: bool,
}

impl<'a> Iterator for TransformSlices<'a, '_> {
    type Item = TransformSlice<'a>;

    fn next(&mut self) -> Option<TransformSlice<'a>> {
        // Phase 1: drain mid-escape from previous chunk.
        if self.draining {
            self.draining = false;
            let mut i = 0;
            while i < self.remaining.len() {
                let action = self.stream.cp.feed(self.remaining[i]);
                i += 1;
                match action {
                    SeqAction::EndSeq => {
                        // Cross-chunk sequences: we don't have the
                        // start bytes, so we can't rewrite. Strip them
                        // (same as FilterStream behavior).
                        self.stream.seq_buf.clear();
                        self.stream.in_seq = false;
                        self.remaining = &self.remaining[i..];
                        return self.next_ground();
                    }
                    SeqAction::Emit => {
                        let byte = self.remaining[i - 1];
                        self.stream.in_seq = false;
                        self.stream.seq_buf.clear();
                        if byte == 0x18 || byte == 0x1A {
                            self.remaining = &self.remaining[i..];
                        } else {
                            self.remaining = &self.remaining[i - 1..];
                        }
                        return self.next_ground();
                    }
                    SeqAction::InSeq | SeqAction::StartSeq => {}
                }
            }
            self.remaining = &[];
            return None;
        }

        self.next_ground()
    }
}

impl<'a> TransformSlices<'a, '_> {
    fn next_ground(&mut self) -> Option<TransformSlice<'a>> {
        loop {
            if self.remaining.is_empty() {
                return None;
            }

            // Fast path: passthrough yields entire remaining.
            if self.config.is_passthrough() {
                let slice = self.remaining;
                self.remaining = &[];
                return Some(TransformSlice::Borrowed(slice));
            }

            let pos = memchr(0x1B, self.remaining);
            match pos {
                None => {
                    let slice = self.remaining;
                    self.remaining = &[];
                    return Some(TransformSlice::Borrowed(slice));
                }
                Some(0) => {
                    // ESC at start — process escape sequence.
                    let seq_start = self.remaining;
                    self.stream.seq_buf.clear();
                    let mut i = 0;
                    while i < self.remaining.len() {
                        let action = self.stream.cp.feed(self.remaining[i]);
                        i += 1;
                        match action {
                            SeqAction::StartSeq => {
                                self.stream.in_seq = true;
                                self.stream.seq_buf.clear();
                                self.stream.seq_buf.push(self.remaining[i - 1]);
                            }
                            SeqAction::InSeq => {
                                self.stream.seq_buf.push(self.remaining[i - 1]);
                            }
                            SeqAction::EndSeq => {
                                self.stream.seq_buf.push(self.remaining[i - 1]);
                                self.stream.in_seq = false;
                                self.remaining = &self.remaining[i..];

                                let needs_rewrite = self.stream.cp.current_kind()
                                    == SeqKind::CsiSgr
                                    && self.stream.cp.sgr_content() != SgrContent::empty();

                                if needs_rewrite {
                                    let param_bytes =
                                        &self.stream.seq_buf[2..self.stream.seq_buf.len() - 1];
                                    let mut rewritten = SmallVec::<[u8; 32]>::new();
                                    rewrite_sgr_direct(
                                        param_bytes,
                                        self.config.depth,
                                        &mut rewritten,
                                    );
                                    self.stream.seq_buf.clear();
                                    return Some(TransformSlice::Owned(rewritten));
                                }

                                // Non-SGR or no color: yield borrowed.
                                let seq_slice = &seq_start[..i];
                                self.stream.seq_buf.clear();
                                return Some(TransformSlice::Borrowed(seq_slice));
                            }
                            SeqAction::Emit => {
                                let byte = self.remaining[i - 1];
                                self.stream.in_seq = false;
                                self.stream.seq_buf.clear();
                                if byte == 0x18 || byte == 0x1A {
                                    self.remaining = &self.remaining[i..];
                                } else {
                                    self.remaining = &self.remaining[i - 1..];
                                }
                                break;
                            }
                        }
                    }
                    // Exhausted remaining without EndSeq — spans next chunk.
                    if i >= self.remaining.len() && !self.stream.cp.is_ground() {
                        self.remaining = &[];
                        return None;
                    }
                }
                Some(p) => {
                    let slice = &self.remaining[..p];
                    self.remaining = &self.remaining[p..];
                    return Some(TransformSlice::Borrowed(slice));
                }
            }
        }
    }
}
