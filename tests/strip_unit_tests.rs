use std::borrow::Cow;

// --- Cow variant tests ---

#[test]
fn strip_clean_returns_borrowed() {
    let input = b"hello world";
    let result = strip_ansi::strip(input);
    assert!(matches!(result, Cow::Borrowed(_)));
    assert_eq!(&*result, input);
}

#[test]
fn strip_trailing_esc_returns_borrowed_prefix() {
    let input = b"hello\x1b[31m";
    let result = strip_ansi::strip(input);
    assert!(matches!(result, Cow::Borrowed(_)));
    assert_eq!(&*result, b"hello");
}

#[test]
fn strip_leading_esc_returns_borrowed_suffix() {
    let input = b"\x1b[31mhello";
    let result = strip_ansi::strip(input);
    assert!(matches!(result, Cow::Borrowed(_)));
    assert_eq!(&*result, b"hello");
}

#[test]
fn strip_mixed_returns_owned() {
    let input = b"a\x1b[31mb\x1b[0mc";
    let result = strip_ansi::strip(input);
    assert!(matches!(result, Cow::Owned(_)));
    assert_eq!(&*result, b"abc");
}

#[test]
fn strip_all_escape_returns_borrowed_empty() {
    let input = b"\x1b[31m";
    let result = strip_ansi::strip(input);
    // Trailing escapes only → Borrowed(prefix) where prefix is empty
    assert!(matches!(result, Cow::Borrowed(_)));
    assert_eq!(&*result, b"");
}

// --- strip_str tests ---

#[test]
fn strip_str_clean() {
    let result = strip_ansi::strip_str("hello");
    assert!(matches!(result, Cow::Borrowed(_)));
    assert_eq!(&*result, "hello");
}

#[test]
fn strip_str_with_ansi() {
    let result = strip_ansi::strip_str("\x1b[31mred\x1b[0m");
    assert_eq!(&*result, "red");
}

#[test]
fn strip_str_utf8_preserved() {
    let result = strip_ansi::strip_str("\x1b[1m日本語\x1b[0m");
    assert_eq!(&*result, "日本語");
}

#[test]
fn strip_str_emoji() {
    let result = strip_ansi::strip_str("\x1b[32m🦀 Rust\x1b[0m");
    assert_eq!(&*result, "🦀 Rust");
}

#[test]
fn strip_str_borrowed_trailing() {
    let result = strip_ansi::strip_str("text\x1b[0m");
    assert!(matches!(result, Cow::Borrowed(_)));
    assert_eq!(&*result, "text");
}

#[test]
fn strip_str_borrowed_leading() {
    let result = strip_ansi::strip_str("\x1b[31mtext");
    assert!(matches!(result, Cow::Borrowed(_)));
    assert_eq!(&*result, "text");
}

// --- strip_into tests ---

#[test]
fn strip_into_clean() {
    let mut out = Vec::new();
    strip_ansi::strip_into(b"hello", &mut out);
    assert_eq!(out, b"hello");
}

#[test]
fn strip_into_with_ansi() {
    let mut out = Vec::new();
    strip_ansi::strip_into(b"\x1b[31mred\x1b[0m", &mut out);
    assert_eq!(out, b"red");
}

#[test]
fn strip_into_appends() {
    let mut out = b"prefix:".to_vec();
    strip_ansi::strip_into(b"\x1b[1mbold\x1b[0m", &mut out);
    assert_eq!(out, b"prefix:bold");
}

#[test]
fn strip_into_eq_strip() {
    let input = b"a\x1b[31mb\x1b[0mc\x1b]8;;url\x07link\x1b]8;;\x07d";
    let mut out = Vec::new();
    strip_ansi::strip_into(input, &mut out);
    assert_eq!(out, &*strip_ansi::strip(input));
}

// --- strip_in_place tests ---

#[test]
fn strip_in_place_clean() {
    let mut buf = b"hello".to_vec();
    let len = strip_ansi::strip_in_place(&mut buf);
    assert_eq!(len, 5);
    assert_eq!(buf, b"hello");
}

#[test]
fn strip_in_place_with_ansi() {
    let mut buf = b"\x1b[31mred\x1b[0m text".to_vec();
    let len = strip_ansi::strip_in_place(&mut buf);
    assert_eq!(&buf[..len], b"red text");
    assert_eq!(buf.len(), len);
}

#[test]
fn strip_in_place_all_escape() {
    let mut buf = b"\x1b[31m\x1b[0m".to_vec();
    let len = strip_ansi::strip_in_place(&mut buf);
    assert_eq!(len, 0);
    assert!(buf.is_empty());
}

#[test]
fn strip_in_place_eq_strip() {
    let input = b"a\x1b[31mb\x1b[0mc\x1b]8;;url\x07link\x1b]8;;\x07d";
    let expected = strip_ansi::strip(input).to_vec();
    let mut buf = input.to_vec();
    let _ = strip_ansi::strip_in_place(&mut buf);
    assert_eq!(buf, expected);
}

#[test]
fn strip_in_place_returns_new_length() {
    let mut buf = b"ab\x1b[1mcd\x1b[0mef".to_vec();
    let len = strip_ansi::strip_in_place(&mut buf);
    assert_eq!(len, buf.len());
    assert_eq!(buf, b"abcdef");
}

// --- contains_ansi tests ---

#[test]
fn contains_ansi_clean() {
    assert!(!strip_ansi::contains_ansi(b"hello world"));
}

#[test]
fn contains_ansi_with_csi() {
    assert!(strip_ansi::contains_ansi(b"\x1b[31mred\x1b[0m"));
}

#[test]
fn contains_ansi_with_osc() {
    assert!(strip_ansi::contains_ansi(b"\x1b]0;title\x07"));
}

#[test]
fn contains_ansi_lone_esc_no_introducer() {
    // ESC at end of input — no valid introducer follows
    assert!(!strip_ansi::contains_ansi(b"text\x1b"));
}

#[test]
fn contains_ansi_esc_invalid_introducer() {
    // ESC followed by 0xFF — not a valid introducer
    assert!(!strip_ansi::contains_ansi(b"\x1b\xff"));
}

#[test]
fn contains_ansi_empty() {
    assert!(!strip_ansi::contains_ansi(b""));
}

#[test]
fn contains_ansi_fe_sequence() {
    // ESC c (RIS) — valid Fe
    assert!(strip_ansi::contains_ansi(b"\x1bc"));
}

#[test]
fn contains_ansi_ss2() {
    assert!(strip_ansi::contains_ansi(b"\x1bNA"));
}

#[test]
fn contains_ansi_early_exit() {
    // Should return true on first valid pair, not scan entire input.
    let mut input = vec![b'x'; 10000];
    input[5000] = 0x1B;
    input[5001] = b'[';
    assert!(strip_ansi::contains_ansi(&input));
}

// --- Real-world sequences ---

#[test]
fn real_world_cargo_output() {
    let input = b"\x1b[0m\x1b[1m\x1b[32m   Compiling\x1b[0m strip-ansi v0.2.0\n";
    let result = strip_ansi::strip(input);
    assert_eq!(&*result, b"   Compiling strip-ansi v0.2.0\n");
}

#[test]
fn real_world_osc8_hyperlink() {
    // OSC 8 hyperlink: ESC ] 8 ; ; URL BEL text ESC ] 8 ; ; BEL
    let input = b"\x1b]8;;https://example.com\x07click\x1b]8;;\x07";
    let result = strip_ansi::strip(input);
    assert_eq!(&*result, b"click");
}

#[test]
fn real_world_256_color() {
    let input = b"\x1b[38;5;196mred\x1b[0m";
    let result = strip_ansi::strip(input);
    assert_eq!(&*result, b"red");
}

#[test]
fn real_world_24bit_color() {
    let input = b"\x1b[38;2;255;0;0mred\x1b[0m";
    let result = strip_ansi::strip(input);
    assert_eq!(&*result, b"red");
}

#[test]
fn real_world_cursor_movement() {
    let input = b"\x1b[2J\x1b[Htext\x1b[10;20H";
    let result = strip_ansi::strip(input);
    assert_eq!(&*result, b"text");
}

#[test]
fn real_world_window_title() {
    let input = b"\x1b]0;My Terminal\x07prompt$ ";
    let result = strip_ansi::strip(input);
    assert_eq!(&*result, b"prompt$ ");
}

#[test]
fn real_world_osc8_st_terminated_with_m_in_url() {
    // 'm' in the URL path must not trigger CSI-style termination
    let input = b"\x1b]8;;http://example.com/m/menu\x1b\\Link Text\x1b]8;;\x1b\\";
    let result = strip_ansi::strip(input);
    assert_eq!(&*result, b"Link Text");
}

#[test]
fn real_world_dcs_with_internal_terminators() {
    // DCS body can contain 'm' and ';' — must not end the sequence early
    let input = b"Prefix\x1bP0;1|data-with-m-and-;-chars\x1b\\Suffix";
    let result = strip_ansi::strip(input);
    assert_eq!(&*result, b"PrefixSuffix");
}

// --- Edge cases ---

#[test]
fn strip_empty() {
    let result = strip_ansi::strip(b"");
    assert!(matches!(result, Cow::Borrowed(_)));
    assert!(result.is_empty());
}

#[test]
fn strip_single_esc() {
    let result = strip_ansi::strip(b"\x1b");
    assert_eq!(&*result, b"");
}

#[test]
fn strip_multiple_consecutive_sequences() {
    let input = b"\x1b[1m\x1b[31m\x1b[4mtext\x1b[0m";
    let result = strip_ansi::strip(input);
    assert_eq!(&*result, b"text");
}

#[test]
fn strip_interleaved_content_and_escapes() {
    let input = b"a\x1b[1mb\x1b[2mc\x1b[3md\x1b[0me";
    let result = strip_ansi::strip(input);
    assert_eq!(&*result, b"abcde");
}

#[test]
fn strip_newlines_preserved() {
    let input = b"\x1b[31mline1\n\x1b[32mline2\n\x1b[0m";
    let result = strip_ansi::strip(input);
    assert_eq!(&*result, b"line1\nline2\n");
}

#[test]
fn strip_tabs_preserved() {
    let input = b"\x1b[1m\tcol1\tcol2\x1b[0m";
    let result = strip_ansi::strip(input);
    assert_eq!(&*result, b"\tcol1\tcol2");
}

#[test]
fn strip_idempotent_on_mixed() {
    let input = b"a\x1b[31mb\x1b[0mc";
    let first = strip_ansi::strip(input);
    let second = strip_ansi::strip(&first);
    assert_eq!(&*first, &*second);
}


// --- CAN/SUB abort tests at strip level ---

#[test]
fn strip_osc8_aborted_by_can() {
    // Malformed OSC 8 where CAN appears mid-sequence
    let input = b"\x1b]8;;http://example.com\x18visible text";
    let result = strip_ansi::strip(input);
    assert_eq!(&*result, b"visible text");
}

#[test]
fn strip_dcs_aborted_by_can() {
    let input = b"before\x1bPqbody\x18after";
    let result = strip_ansi::strip(input);
    assert_eq!(&*result, b"beforeafter");
}

#[test]
fn strip_apc_aborted_by_sub() {
    let input = b"before\x1b_apc\x1Aafter";
    let result = strip_ansi::strip(input);
    assert_eq!(&*result, b"beforeafter");
}

#[test]
fn strip_osc8_can_then_normal_close() {
    // CAN aborts the first OSC 8, text is visible, second OSC 8 closes normally
    let input = b"\x1b]8;;url\x18Link\x1b]8;;\x07";
    let result = strip_ansi::strip(input);
    assert_eq!(&*result, b"Link");
}

#[test]
fn strip_can_in_ground_emitted() {
    // CAN in ground state is just a byte — should pass through
    let input = b"hello\x18world";
    let result = strip_ansi::strip(input);
    assert_eq!(&*result, b"hello\x18world");
}


// --- Drop-in alias tests ---

#[test]
fn strip_ansi_bytes_eq_strip() {
    let input = b"a\x1b[31mb\x1b[0mc";
    assert_eq!(
        &*strip_ansi::strip_ansi_bytes(input),
        &*strip_ansi::strip(input)
    );
}

#[test]
fn strip_ansi_bytes_clean_borrowed() {
    let input = b"no escapes";
    let result = strip_ansi::strip_ansi_bytes(input);
    assert!(matches!(result, Cow::Borrowed(_)));
    assert_eq!(&*result, input);
}

#[test]
fn strip_ansi_escapes_returns_vec() {
    let input = b"\x1b[31mred\x1b[0m";
    let result: Vec<u8> = strip_ansi::strip_ansi_escapes(input);
    assert_eq!(result, b"red");
}

#[test]
fn strip_ansi_escapes_accepts_vec() {
    let input = vec![0x1b, b'[', b'1', b'm', b'h', b'i', 0x1b, b'[', b'0', b'm'];
    let result: Vec<u8> = strip_ansi::strip_ansi_escapes(input);
    assert_eq!(result, b"hi");
}

#[test]
fn strip_ansi_escapes_accepts_string() {
    let input = String::from("\x1b[32mgreen\x1b[0m");
    let result = strip_ansi::strip_ansi_escapes(input);
    assert_eq!(result, b"green");
}

// ── try_strip_str tests ─────────────────────────────────────────────

#[test]
fn try_strip_str_clean() {
    let result = strip_ansi::try_strip_str("hello world");
    assert_eq!(result, Some(Cow::Borrowed("hello world")));
}

#[test]
fn try_strip_str_with_ansi() {
    let result = strip_ansi::try_strip_str("\x1b[31mred\x1b[0m");
    assert_eq!(result.as_deref(), Some("red"));
}

#[test]
fn try_strip_str_empty() {
    let result = strip_ansi::try_strip_str("");
    assert_eq!(result, Some(Cow::Borrowed("")));
}

#[test]
fn try_strip_str_utf8_preserved() {
    let result = strip_ansi::try_strip_str("\x1b[1m日本語\x1b[0m");
    assert_eq!(result.as_deref(), Some("日本語"));
}

#[test]
fn try_strip_str_eq_strip_str() {
    let input = "\x1b[38;2;255;0;0mhello\x1b[0m world";
    let expected = strip_ansi::strip_str(input);
    let result = strip_ansi::try_strip_str(input);
    assert_eq!(result.as_deref(), Some(expected.as_ref()));
}

// ── contains_ansi_c1 tests ──────────────────────────────────────────

#[test]
fn contains_ansi_c1_detects_8bit_csi() {
    // 0x9B is the 8-bit C1 form of CSI (ESC [)
    assert!(strip_ansi::contains_ansi_c1(&[0x9B, b'3', b'1', b'm']));
}

#[test]
fn contains_ansi_c1_detects_8bit_osc() {
    // 0x9D is the 8-bit C1 form of OSC (ESC ])
    assert!(strip_ansi::contains_ansi_c1(&[0x9D, b'0', b';', b'x', 0x07]));
}

#[test]
fn contains_ansi_c1_detects_8bit_dcs() {
    // 0x90 is the 8-bit C1 form of DCS (ESC P)
    assert!(strip_ansi::contains_ansi_c1(&[0x90, b'q', 0x1B, b'\\']));
}

#[test]
fn contains_ansi_c1_clean_ascii() {
    assert!(!strip_ansi::contains_ansi_c1(b"hello world"));
}

#[test]
fn contains_ansi_c1_also_detects_7bit() {
    // Should still detect normal 7-bit ESC sequences
    assert!(strip_ansi::contains_ansi_c1(b"\x1b[31m"));
}

#[test]
fn contains_ansi_c1_empty() {
    assert!(!strip_ansi::contains_ansi_c1(b""));
}
