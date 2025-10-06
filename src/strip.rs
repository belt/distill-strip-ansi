use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use memchr::{memchr, memchr3};

use crate::parser::{Action, Parser, State};

/// Strip ANSI escape sequences from a byte slice.
///
/// Returns `Cow::Borrowed` when no allocation is needed:
/// - No ESC bytes → borrowed input
/// - Only trailing escapes → borrowed prefix
/// - Only leading escapes → borrowed suffix
///
/// Returns `Cow::Owned` when escapes are interleaved with content.
#[must_use]
pub fn strip(input: &[u8]) -> Cow<'_, [u8]> {
    let Some(first_esc) = memchr(0x1B, input) else {
        return Cow::Borrowed(input);
    };

    // Speculative: are all bytes from first ESC onward part of escapes?
    let mut parser = Parser::new();
    let mut first_emit = None;
    for (i, &b) in input[first_esc..].iter().enumerate() {
        if parser.feed(b) == Action::Emit {
            first_emit = Some(first_esc + i);
            break;
        }
    }

    let Some(emit_pos) = first_emit else {
        return Cow::Borrowed(&input[..first_esc]);
    };

    // Leading escapes only?
    if first_esc == 0 && parser.is_ground() && memchr(0x1B, &input[emit_pos..]).is_none() {
        return Cow::Borrowed(&input[emit_pos..]);
    }

    // Full strip: memchr to skip ground bytes, parser for escapes.
    // Adaptive allocation: start at 80% of input (typical ANSI is
    // 10-30% of bytes). The Vec grows if needed but avoids the
    // full-input over-allocation that inflates RSS.
    let mut output = Vec::with_capacity(input.len() * 4 / 5);
    output.extend_from_slice(&input[..first_esc]);

    let mut remaining = &input[first_esc..];
    while !remaining.is_empty() {
        // memchr skip to next ESC — bulk copy ground bytes.
        let esc_pos = memchr(0x1B, remaining).unwrap_or(remaining.len());
        output.extend_from_slice(&remaining[..esc_pos]);
        remaining = &remaining[esc_pos..];
        if remaining.is_empty() {
            break;
        }

        // Parse the escape sequence.
        let mut p = Parser::new();
        let mut i = 0;
        while i < remaining.len() {
            let action = p.feed(remaining[i]);
            i += 1;
            if action == Action::Emit {
                output.push(remaining[i - 1]);
            }
            // After entering a passthrough state (OSC, DCS, SOS/PM/APC),
            // use memchr to skip directly to the terminator instead of
            // feeding each body byte through the state table.
            if !p.is_ground() {
                let skip = match p.state() {
                    // OSC: terminates on BEL(07), ESC(1B), CAN(18), SUB(1A).
                    State::OscString => {
                        // memchr3 only takes 3 needles; find min of two searches.
                        let a = memchr3(0x07, 0x1B, 0x18, &remaining[i..]);
                        let b = memchr(0x1A, &remaining[i..]);
                        match (a, b) {
                            (Some(x), Some(y)) => Some(x.min(y)),
                            (Some(x), None) => Some(x),
                            (None, Some(y)) => Some(y),
                            (None, None) => None,
                        }
                    }
                    // DCS/String: terminates on ESC(1B), CAN(18), SUB(1A).
                    State::DcsPassthrough | State::StringPassthrough => {
                        memchr3(0x1B, 0x18, 0x1A, &remaining[i..])
                    }
                    _ => None,
                };
                if let Some(skip_len) = skip {
                    // Skip body bytes — they're all Skip action, no emit.
                    i += skip_len;
                    // Don't consume the terminator — let the parser handle it.
                }
            }
            if p.is_ground() {
                break;
            }
        }
        remaining = &remaining[i..];
    }

    Cow::Owned(output)
}

/// Strip ANSI escape sequences from a UTF-8 string.
///
/// Equivalent to [`strip`] but operates on `&str` and returns `Cow<str>`.
/// UTF-8 validity is preserved: borrowed paths use pointer arithmetic
/// on the original `&str`, owned path uses safe `String::from_utf8`.
#[must_use]
pub fn strip_str(input: &str) -> Cow<'_, str> {
    match strip(input.as_bytes()) {
        Cow::Borrowed(b) => {
            // b is a subslice of input.as_bytes(), so it's valid UTF-8.
            // Recover the &str via pointer offset.
            let start = b.as_ptr() as usize - input.as_ptr() as usize;
            Cow::Borrowed(&input[start..start + b.len()])
        }
        Cow::Owned(v) => {
            // Input was valid UTF-8, stripping only removes bytes,
            // so output is valid UTF-8.
            Cow::Owned(String::from_utf8(v).expect("strip preserves UTF-8"))
        }
    }
}

/// Fallible variant of [`strip_str`].
///
/// Returns `None` if the stripped output is not valid UTF-8.
/// In practice this cannot happen (stripping only removes complete
/// escape sequence bytes, all ≤ 0x7E, never UTF-8 continuation
/// bytes), but this variant avoids the `expect` panic path for
/// defensive consumers.
#[must_use]
pub fn try_strip_str(input: &str) -> Option<Cow<'_, str>> {
    match strip(input.as_bytes()) {
        Cow::Borrowed(b) => {
            let start = b.as_ptr() as usize - input.as_ptr() as usize;
            Some(Cow::Borrowed(&input[start..start + b.len()]))
        }
        Cow::Owned(v) => String::from_utf8(v).ok().map(Cow::Owned),
    }
}

/// Strip ANSI escape sequences into a caller-provided buffer.
///
/// Appends stripped content to `out`. Does not clear `out` first.
pub fn strip_into(input: &[u8], out: &mut Vec<u8>) {
    if memchr(0x1B, input).is_none() {
        out.extend_from_slice(input);
        return;
    }
    match strip(input) {
        Cow::Borrowed(b) => out.extend_from_slice(b),
        Cow::Owned(v) => out.extend_from_slice(&v),
    }
}

/// Strip ANSI escape sequences in place using gap compaction.
///
/// Returns the new length. The buffer is truncated to the new length.
/// Uses `copy_within` for safe bulk moves and `memchr` to skip
/// ground bytes between escapes.
#[must_use]
pub fn strip_in_place(buf: &mut Vec<u8>) -> usize {
    let Some(esc) = memchr(0x1B, buf) else {
        return buf.len();
    };

    let mut dst = esc;
    let len = buf.len();
    let mut src = esc;

    while src < len {
        // Find next ESC from current position.
        let next_esc = memchr(0x1B, &buf[src..]).map(|p| src + p).unwrap_or(len);

        // Copy ground bytes.
        let ground_len = next_esc - src;
        if ground_len > 0 {
            buf.copy_within(src..next_esc, dst);
            dst += ground_len;
        }
        src = next_esc;
        if src >= len {
            break;
        }

        // Feed escape bytes through parser.
        let mut parser = Parser::new();
        while src < len {
            let action = parser.feed(buf[src]);
            if action == Action::Emit {
                buf[dst] = buf[src];
                dst += 1;
            }
            src += 1;
            if parser.is_ground() && action != Action::Emit {
                break;
            }
            if parser.is_ground() {
                break;
            }
        }
    }

    buf.truncate(dst);
    dst
}

/// Check whether a byte slice contains any ANSI escape sequences.
///
/// Uses `memchr` SIMD scan for ESC (0x1B) followed by introducer
/// validation. Returns `true` on the first valid ESC + introducer pair.
#[must_use]
pub fn contains_ansi(input: &[u8]) -> bool {
    let mut remaining = input;
    while let Some(pos) = memchr(0x1B, remaining) {
        // Check if there's a valid introducer after ESC.
        if let Some(&next) = remaining.get(pos + 1) {
            match next {
                b'[' | b']' | b'P' | b'X' | b'^' | b'_' | b'N' | b'O' => return true,
                0x20..=0x7E => return true,
                _ => {}
            }
        }
        remaining = &remaining[pos + 1..];
    }
    false
}

/// Check whether a byte slice contains ANSI escape sequences,
/// including 8-bit C1 control codes (0x80–0x9F).
///
/// Unlike [`contains_ansi`], this also detects raw C1 introducers
/// (`0x9B` = CSI, `0x9D` = OSC, `0x90` = DCS, etc.) used in
/// legacy 8-bit encodings. These collide with valid UTF-8 lead
/// bytes, so this function will false-positive on UTF-8 input
/// containing characters in the U+0080–U+009F range (rare but
/// possible in Latin-1 or Windows-1252 encoded streams).
///
/// Use [`contains_ansi`] for UTF-8 streams (the common case).
/// Use this function for binary or known-8-bit-encoded streams
/// where C1 bypass attacks are a concern.
#[must_use]
pub fn contains_ansi_c1(input: &[u8]) -> bool {
    // Check 7-bit forms first.
    if contains_ansi(input) {
        return true;
    }
    // Check 8-bit C1 control codes.
    // CSI=0x9B, OSC=0x9D, DCS=0x90, SOS=0x98, PM=0x9E, APC=0x9F
    const C1_CODES: [u8; 6] = [0x9B, 0x9D, 0x90, 0x98, 0x9E, 0x9F];
    input.iter().any(|b| C1_CODES.contains(b))
}

// --- Drop-in compatibility aliases ---

/// Drop-in replacement for [`fast_strip_ansi::strip_ansi_bytes`].
///
/// Identical to [`strip`] — returns `Cow::Borrowed` when no
/// allocation is needed, `Cow::Owned` otherwise.
///
/// [`fast_strip_ansi::strip_ansi_bytes`]: https://docs.rs/fast-strip-ansi/latest/fast_strip_ansi/fn.strip_ansi_bytes.html
#[inline]
#[must_use]
pub fn strip_ansi_bytes(input: &[u8]) -> Cow<'_, [u8]> {
    strip(input)
}

/// Drop-in replacement for [`strip_ansi_escapes::strip`].
///
/// Accepts any `AsRef<[u8]>` and always returns `Vec<u8>`,
/// matching the `strip-ansi-escapes` API. Prefer [`strip`]
/// directly when zero-alloc `Cow` semantics are acceptable.
///
/// [`strip_ansi_escapes::strip`]: https://docs.rs/strip-ansi-escapes/latest/strip_ansi_escapes/fn.strip.html
#[inline]
#[must_use]
pub fn strip_ansi_escapes<T: AsRef<[u8]>>(data: T) -> Vec<u8> {
    strip(data.as_ref()).into_owned()
}
