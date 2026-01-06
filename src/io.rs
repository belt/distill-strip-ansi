use std::io::{self, BufWriter, IsTerminal, LineWriter, StdoutLock, Write};

/// Adaptive output buffer that selects buffering strategy based on output target.
///
/// - TTY → `LineWriter` for low-latency line-at-a-time display
/// - Pipe/file → `BufWriter` (32 KB) for throughput
pub enum OutputBuffer<'a> {
    Line(LineWriter<StdoutLock<'a>>),
    Buf(BufWriter<StdoutLock<'a>>),
}

impl<'a> OutputBuffer<'a> {
    /// Create a new `OutputBuffer` from a locked stdout handle.
    ///
    /// Checks `stdout.is_terminal()` to select the buffering strategy:
    /// - TTY → `LineWriter::new(lock)`
    /// - Pipe/file → `BufWriter::with_capacity(32 * 1024, lock)`
    pub fn new(stdout: &'a io::Stdout) -> Self {
        let is_tty = stdout.is_terminal();
        let lock = stdout.lock();
        if is_tty {
            OutputBuffer::Line(LineWriter::new(lock))
        } else {
            OutputBuffer::Buf(BufWriter::with_capacity(32 * 1024, lock))
        }
    }
}

impl Write for OutputBuffer<'_> {
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
