use std::io::{self, BufRead, Write};

use bstr::io::BufReadExt;
use memchr::memchr;

/// Strip mode: process each line, write stripped output.
///
/// Uses a two-tier fast path:
/// 1. `memchr` scan for ESC (0x1B) — clean lines write directly (zero alloc)
/// 2. `strip_ansi_escapes::Writer` wraps output for dirty lines (no intermediate Vec)
///
/// Returns Ok(()) on success, or the first I/O error.
pub fn run_strip<R: BufRead, W: Write>(mut reader: R, writer: &mut W) -> io::Result<()> {
    reader.for_byte_line_with_terminator(|line| {
        if memchr(0x1B, line).is_some() {
            // Dirty line: strip inline through Writer filter
            strip_ansi_escapes::Writer::new(&mut *writer).write_all(line)?;
        } else {
            // Clean line: direct pass-through, zero allocation
            writer.write_all(line)?;
        }
        Ok(true)
    })
}

/// Check mode: scan for ANSI presence via raw buffer chunks.
///
/// Skips line iteration entirely — only needs to find a single `0x1B` byte
/// anywhere in the stream. Uses `fill_buf` + `memchr` on each chunk for
/// minimal overhead.
///
/// Returns Ok(true) if ANSI found, Ok(false) if clean.
pub fn run_check<R: BufRead>(mut reader: R) -> io::Result<bool> {
    loop {
        let buf = reader.fill_buf()?;
        if buf.is_empty() {
            return Ok(false);
        }
        if memchr(0x1B, buf).is_some() {
            return Ok(true);
        }
        let len = buf.len();
        reader.consume(len);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn run_strip_clean_lines_passthrough() {
        let input = b"hello world\nno ansi here\n";
        let mut output = Vec::new();
        run_strip(Cursor::new(input), &mut output).unwrap();
        assert_eq!(output, input);
    }

    #[test]
    fn run_strip_dirty_lines_stripped() {
        let input = b"\x1b[32mgreen\x1b[0m\n";
        let mut output = Vec::new();
        run_strip(Cursor::new(input), &mut output).unwrap();
        assert_eq!(output, b"green\n");
    }

    #[test]
    fn run_strip_mixed_clean_and_dirty() {
        let input = b"clean line\n\x1b[1mbold\x1b[0m\nanother clean\n";
        let mut output = Vec::new();
        run_strip(Cursor::new(input), &mut output).unwrap();
        assert_eq!(output, b"clean line\nbold\nanother clean\n");
    }

    #[test]
    fn run_check_detects_ansi() {
        let input = b"normal\n\x1b[31mred\x1b[0m\n";
        assert!(run_check(Cursor::new(input)).unwrap());
    }

    #[test]
    fn run_check_clean_returns_false() {
        let input = b"no ansi at all\njust plain text\n";
        assert!(!run_check(Cursor::new(input)).unwrap());
    }

    #[test]
    fn run_check_empty_returns_false() {
        assert!(!run_check(Cursor::new(b"")).unwrap());
    }

    #[test]
    fn run_check_multi_chunk_finds_ansi_later() {
        // First chunk is clean, second chunk has ESC — exercises the
        // consume-and-continue loop body (line 34).
        let mut data = vec![b'A'; 8192]; // clean chunk
        data.extend_from_slice(b"\x1b[31mred\x1b[0m");
        assert!(run_check(Cursor::new(&data)).unwrap());
    }

    #[test]
    fn run_check_multi_chunk_all_clean() {
        // Multiple clean chunks, no ESC anywhere.
        let data = vec![b'X'; 16384];
        assert!(!run_check(Cursor::new(&data)).unwrap());
    }

}
