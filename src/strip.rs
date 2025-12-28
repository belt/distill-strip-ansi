use std::borrow::Cow;
use std::io::{self, BufRead, Write};

use bstr::io::BufReadExt;

/// Strip mode: process each line, write stripped output.
/// Returns Ok(()) on success, or the first I/O error.
pub fn run_strip<R: BufRead, W: Write>(mut reader: R, writer: &mut W) -> io::Result<()> {
    reader.for_byte_line_with_terminator(|line| {
        let lossy = String::from_utf8_lossy(line);
        let stripped = console::strip_ansi_codes(&lossy);
        match stripped {
            Cow::Borrowed(_) => {
                // No ANSI found — write original bytes (zero-alloc fast path).
                writer.write_all(line)?;
            }
            Cow::Owned(ref s) => {
                writer.write_all(s.as_bytes())?;
            }
        }
        Ok(true)
    })
}

/// Check mode: scan for ANSI presence.
/// Returns Ok(true) if ANSI found, Ok(false) if clean.
pub fn run_check<R: BufRead>(mut reader: R) -> io::Result<bool> {
    let mut found = false;
    reader.for_byte_line_with_terminator(|line| {
        if line.contains(&0x1B) {
            found = true;
            Ok(false) // short-circuit: stop iteration
        } else {
            Ok(true) // continue
        }
    })?;
    Ok(found)
}
