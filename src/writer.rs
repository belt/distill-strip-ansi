use std::io::{self, Write};

use crate::stream::StripStream;

/// An `io::Write` adapter that strips ANSI escape sequences on the fly.
///
/// Wraps any `W: Write` with a 1-byte parser state machine. Bytes
/// written via [`write`](Write::write) are passed through
/// [`StripStream::strip_slices`] and only content bytes reach the
/// inner writer. Escape sequences spanning multiple `write` calls
/// are handled correctly.
///
/// # Comparison with `strip-ansi-escapes::Writer`
///
/// | | `StripWriter` | `strip-ansi-escapes::Writer` |
/// |---|---|---|
/// | Parser state | 1 byte | ~1 KB (`vte`) |
/// | Allocations | zero (borrowed slices) | per-write Vec |
/// | `no_std` lib | yes (writer is `std`-only) | no |
///
/// # Example
///
/// ```
/// use strip_ansi::StripWriter;
/// use std::io::Write;
///
/// let mut buf = Vec::new();
/// let mut writer = StripWriter::new(&mut buf);
/// writer.write_all(b"\x1b[31mhello\x1b[0m").unwrap();
/// writer.flush().unwrap();
/// assert_eq!(buf, b"hello");
/// ```
pub struct StripWriter<W> {
    inner: W,
    stream: StripStream,
}

impl<W: Write> StripWriter<W> {
    /// Wrap a writer with ANSI stripping.
    #[inline]
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            stream: StripStream::new(),
        }
    }

    /// Reset the parser to ground state, discarding any
    /// incomplete escape sequence.
    #[inline]
    pub fn reset(&mut self) {
        self.stream.reset();
    }

    /// Consume the writer, returning the inner `W`.
    ///
    /// Any incomplete escape sequence is silently discarded.
    #[inline]
    pub fn into_inner(self) -> W {
        self.inner
    }

    /// Borrow the inner writer.
    #[inline]
    pub fn get_ref(&self) -> &W {
        &self.inner
    }

    /// Mutably borrow the inner writer.
    #[inline]
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.inner
    }
}

impl<W: Write> Write for StripWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        for slice in self.stream.strip_slices(buf) {
            self.inner.write_all(slice)?;
        }
        // Report all input bytes as consumed — the caller
        // must not retry with a suffix of `buf`.
        Ok(buf.len())
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
