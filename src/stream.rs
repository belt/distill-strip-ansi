use crate::parser::{Action, Parser};
use crate::strip::passthrough_skip;

use memchr::memchr;

/// Stateful streaming ANSI stripper for chunked input.
///
/// Escape sequences may span chunk boundaries. The parser state
/// (1 byte) carries across calls. No pending buffer — incomplete
/// escapes are skipped, never retroactively emitted.
///
/// Primary API: [`strip_slices`](StripStream::strip_slices) returns
/// borrowed slices of the input. Zero intermediate copies.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct StripStream {
    parser: Parser,
}

// Compile-time assertions: Send + Sync.
const _: () = {
    const fn assert_send<T: Send>() {}
    const fn assert_sync<T: Sync>() {}
    assert_send::<StripStream>();
    assert_sync::<StripStream>();
};

impl StripStream {
    /// Create a new streaming stripper.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            parser: Parser::new(),
        }
    }

    /// Reset the parser to ground state.
    #[inline]
    pub fn reset(&mut self) {
        self.parser.reset();
    }

    /// Discard any incomplete escape at stream end.
    #[inline]
    pub fn finish(&mut self) {
        self.parser.reset();
    }

    /// Returns `true` if the parser is in the ground state.
    #[inline]
    #[must_use]
    pub const fn is_ground(&self) -> bool {
        self.parser.is_ground()
    }

    /// Strip a chunk, yielding borrowed slices of content bytes.
    ///
    /// Call this repeatedly with each input chunk. Concatenating
    /// all yielded slices across all calls produces the same result
    /// as [`strip`](crate::strip) on the full concatenated input.
    pub fn strip_slices<'a>(&mut self, input: &'a [u8]) -> StripSlices<'a, '_> {
        let draining = !self.parser.is_ground();
        StripSlices {
            remaining: input,
            parser: &mut self.parser,
            draining,
        }
    }

    /// Strip a chunk, appending content bytes to `out`.
    pub fn push(&mut self, input: &[u8], out: &mut alloc::vec::Vec<u8>) {
        for slice in self.strip_slices(input) {
            out.extend_from_slice(slice);
        }
    }

    /// Strip a chunk, writing content bytes to `writer`.
    #[cfg(feature = "std")]
    pub fn push_write(
        &mut self,
        input: &[u8],
        writer: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        for slice in self.strip_slices(input) {
            writer.write_all(slice)?;
        }
        Ok(())
    }
}

impl Default for StripStream {
    fn default() -> Self {
        Self::new()
    }
}

/// Iterator yielding borrowed content slices from a single chunk.
pub struct StripSlices<'a, 'p> {
    remaining: &'a [u8],
    parser: &'p mut Parser,
    draining: bool,
}

impl<'a> Iterator for StripSlices<'a, '_> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<&'a [u8]> {
        // Phase 1: drain mid-escape from previous chunk.
        if self.draining {
            self.draining = false;
            let mut i = 0;
            while i < self.remaining.len() {
                let action = self.parser.feed(self.remaining[i]);
                i += 1;
                if action == Action::Emit {
                    self.remaining = &self.remaining[i - 1..];
                    return self.next_ground();
                }
                if self.parser.is_ground() {
                    self.remaining = &self.remaining[i..];
                    return self.next_ground();
                }
                // Skip passthrough body bytes (OSC/DCS/String) only.
                // Short sequences (CSI, Fe, SS2/SS3) skip this entirely.
                if self.parser.is_passthrough() {
                    i += passthrough_skip(self.parser.state(), &self.remaining[i..]);
                }
            }
            // Entire chunk consumed by escape.
            self.remaining = &[];
            return None;
        }

        self.next_ground()
    }
}

impl<'a> StripSlices<'a, '_> {
    fn next_ground(&mut self) -> Option<&'a [u8]> {
        loop {
            if self.remaining.is_empty() {
                return None;
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
                    // ESC at start — feed through parser until ground.
                    let mut i = 0;
                    let mut found_emit = false;
                    while i < self.remaining.len() {
                        let action = self.parser.feed(self.remaining[i]);
                        i += 1;
                        if action == Action::Emit {
                            self.remaining = &self.remaining[i - 1..];
                            found_emit = true;
                            break;
                        }
                        if self.parser.is_ground() {
                            self.remaining = &self.remaining[i..];
                            found_emit = true;
                            break;
                        }
                        // Skip passthrough body bytes (OSC/DCS/String) only.
                        if self.parser.is_passthrough() {
                            i += passthrough_skip(self.parser.state(), &self.remaining[i..]);
                        }
                    }
                    if found_emit {
                        continue;
                    }
                    // Chunk ended mid-escape.
                    self.remaining = &[];
                    return None;
                }
                Some(p) => {
                    // Content before ESC.
                    let slice = &self.remaining[..p];
                    self.remaining = &self.remaining[p..];

                    // Now consume the escape sequence.
                    let mut i = 0;
                    while i < self.remaining.len() {
                        let action = self.parser.feed(self.remaining[i]);
                        i += 1;
                        if action == Action::Emit {
                            self.remaining = &self.remaining[i - 1..];
                            return Some(slice);
                        }
                        if self.parser.is_ground() {
                            self.remaining = &self.remaining[i..];
                            return Some(slice);
                        }
                        // Skip passthrough body bytes (OSC/DCS/String) only.
                        if self.parser.is_passthrough() {
                            i += passthrough_skip(self.parser.state(), &self.remaining[i..]);
                        }
                    }
                    // Chunk ended mid-escape.
                    self.remaining = &[];
                    return Some(slice);
                }
            }
        }
    }
}
