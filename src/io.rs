use std::io::{self, BufWriter, IsTerminal, LineWriter, Stdout, Write};

/// Adaptive output buffer that selects buffering strategy based on output target.
///
/// - TTY → `LineWriter` for low-latency line-at-a-time display
/// - Pipe/file → `BufWriter` (32 KB) for throughput
///
/// Owns the `Stdout` handle so it can be boxed as `dyn Write`.
pub enum OutputBuffer {
    Line(LineWriter<Stdout>),
    Buf(BufWriter<Stdout>),
}

impl OutputBuffer {
    /// Create a new `OutputBuffer` from an owned stdout handle.
    ///
    /// Checks `stdout.is_terminal()` to select the buffering strategy:
    /// - TTY → `LineWriter`
    /// - Pipe/file → `BufWriter` (32 KB)
    pub fn new(stdout: Stdout) -> Self {
        if stdout.is_terminal() {
            OutputBuffer::Line(LineWriter::new(stdout))
        } else {
            OutputBuffer::Buf(BufWriter::with_capacity(32 * 1024, stdout))
        }
    }
}

impl Write for OutputBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            OutputBuffer::Line(w) => w.write(buf),
            OutputBuffer::Buf(w) => w.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            OutputBuffer::Line(w) => w.flush(),
            OutputBuffer::Buf(w) => w.flush(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn output_buffer_new_selects_strategy() {
        let stdout = io::stdout();
        let is_tty = stdout.is_terminal();
        let buf = OutputBuffer::new(io::stdout());
        if is_tty {
            assert!(matches!(buf, OutputBuffer::Line(_)));
        } else {
            assert!(matches!(buf, OutputBuffer::Buf(_)));
        }
    }

    #[test]
    fn output_buffer_write_and_flush() {
        let mut buf = OutputBuffer::new(io::stdout());
        let n = buf.write(b"test data\n").unwrap();
        assert_eq!(n, 10);
        buf.flush().unwrap();
    }
}
