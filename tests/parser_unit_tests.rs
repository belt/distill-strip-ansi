use strip_ansi::{Action, Parser, State};

fn feed_all(input: &[u8]) -> Vec<Action> {
    let mut p = Parser::new();
    input.iter().map(|&b| p.feed(b)).collect()
}

fn feed_state(input: &[u8]) -> State {
    let mut p = Parser::new();
    for &b in input {
        p.feed(b);
    }
    p.state()
}

// --- Ground state ---

#[test]
fn ground_emits_printable() {
    let mut p = Parser::new();
    assert_eq!(p.feed(b'A'), Action::Emit);
    assert_eq!(p.state(), State::Ground);
}

#[test]
fn ground_emits_null() {
    let mut p = Parser::new();
    assert_eq!(p.feed(0x00), Action::Emit);
}

#[test]
fn ground_emits_high_byte() {
    let mut p = Parser::new();
    assert_eq!(p.feed(0xFF), Action::Emit);
}

#[test]
fn ground_esc_transitions() {
    let mut p = Parser::new();
    assert_eq!(p.feed(0x1B), Action::Skip);
    assert_eq!(p.state(), State::EscapeStart);
}

// --- CSI sequences ---

#[test]
fn csi_sgr_full() {
    // ESC [ 3 1 m  → all Skip, back to Ground
    let actions = feed_all(b"\x1b[31m");
    assert!(actions.iter().all(|a| *a == Action::Skip));
    assert_eq!(feed_state(b"\x1b[31m"), State::Ground);
}

#[test]
fn csi_cursor_up() {
    // ESC [ 5 A
    let actions = feed_all(b"\x1b[5A");
    assert!(actions.iter().all(|a| *a == Action::Skip));
    assert_eq!(feed_state(b"\x1b[5A"), State::Ground);
}

#[test]
fn csi_with_intermediate() {
    // ESC [ 0 SP q  (DECSCA with intermediate 0x20)
    let actions = feed_all(b"\x1b[0 q");
    assert!(actions.iter().all(|a| *a == Action::Skip));
    assert_eq!(feed_state(b"\x1b[0 q"), State::Ground);
}

#[test]
fn csi_multiple_params() {
    // ESC [ 3 8 ; 5 ; 1 9 6 m
    let actions = feed_all(b"\x1b[38;5;196m");
    assert!(actions.iter().all(|a| *a == Action::Skip));
    assert_eq!(feed_state(b"\x1b[38;5;196m"), State::Ground);
}

#[test]
fn csi_no_params() {
    // ESC [ m  (reset SGR)
    let actions = feed_all(b"\x1b[m");
    assert!(actions.iter().all(|a| *a == Action::Skip));
    assert_eq!(feed_state(b"\x1b[m"), State::Ground);
}

// --- OSC sequences ---

#[test]
fn osc_bel_terminated() {
    // ESC ] 0 ; t i t l e BEL
    let actions = feed_all(b"\x1b]0;title\x07");
    assert!(actions.iter().all(|a| *a == Action::Skip));
    assert_eq!(feed_state(b"\x1b]0;title\x07"), State::Ground);
}

#[test]
fn osc_st_terminated() {
    // ESC ] 0 ; t i t l e ESC backslash
    let actions = feed_all(b"\x1b]0;title\x1b\\");
    assert!(actions.iter().all(|a| *a == Action::Skip));
    assert_eq!(feed_state(b"\x1b]0;title\x1b\\"), State::Ground);
}

#[test]
fn osc8_hyperlink() {
    // ESC ] 8 ; ; h t t p : / / e x . c o m BEL
    let actions = feed_all(b"\x1b]8;;http://ex.com\x07");
    assert!(actions.iter().all(|a| *a == Action::Skip));
}

// --- DCS sequences ---

#[test]
fn dcs_with_params_and_passthrough() {
    // ESC P 1 $ r ... ESC backslash
    let actions = feed_all(b"\x1bP1$rdata\x1b\\");
    assert!(actions.iter().all(|a| *a == Action::Skip));
    assert_eq!(feed_state(b"\x1bP1$rdata\x1b\\"), State::Ground);
}

#[test]
fn dcs_immediate_passthrough() {
    // ESC P q ... ESC backslash  (sixel)
    let actions = feed_all(b"\x1bPqpixels\x1b\\");
    assert!(actions.iter().all(|a| *a == Action::Skip));
    assert_eq!(feed_state(b"\x1bPqpixels\x1b\\"), State::Ground);
}

// --- APC / PM / SOS (StringPassthrough) ---

#[test]
fn apc_sequence() {
    // ESC _ body ESC backslash
    let actions = feed_all(b"\x1b_app-data\x1b\\");
    assert!(actions.iter().all(|a| *a == Action::Skip));
    assert_eq!(feed_state(b"\x1b_app-data\x1b\\"), State::Ground);
}

#[test]
fn pm_sequence() {
    // ESC ^ body ESC backslash
    let actions = feed_all(b"\x1b^private\x1b\\");
    assert!(actions.iter().all(|a| *a == Action::Skip));
    assert_eq!(feed_state(b"\x1b^private\x1b\\"), State::Ground);
}

#[test]
fn sos_sequence() {
    // ESC X body ESC backslash
    let actions = feed_all(b"\x1bXstring\x1b\\");
    assert!(actions.iter().all(|a| *a == Action::Skip));
    assert_eq!(feed_state(b"\x1bXstring\x1b\\"), State::Ground);
}

// --- SS2 / SS3 ---

#[test]
fn ss2_skips_next_byte() {
    // ESC N <byte>
    let mut p = Parser::new();
    assert_eq!(p.feed(0x1B), Action::Skip);
    assert_eq!(p.state(), State::EscapeStart);
    assert_eq!(p.feed(b'N'), Action::Skip);
    assert_eq!(p.state(), State::Ss2);
    assert_eq!(p.feed(b'A'), Action::Skip);
    assert_eq!(p.state(), State::Ground);
}

#[test]
fn ss3_skips_next_byte() {
    let mut p = Parser::new();
    p.feed(0x1B);
    assert_eq!(p.feed(b'O'), Action::Skip);
    assert_eq!(p.state(), State::Ss3);
    assert_eq!(p.feed(b'B'), Action::Skip);
    assert_eq!(p.state(), State::Ground);
}

// --- Fe escape sequences ---

#[test]
fn fe_ris() {
    // ESC c  (RIS — Reset to Initial State)
    let actions = feed_all(b"\x1bc");
    assert!(actions.iter().all(|a| *a == Action::Skip));
    assert_eq!(feed_state(b"\x1bc"), State::Ground);
}

#[test]
fn fe_nel() {
    // ESC E  (NEL — Next Line)
    let actions = feed_all(b"\x1bE");
    assert!(actions.iter().all(|a| *a == Action::Skip));
    assert_eq!(feed_state(b"\x1bE"), State::Ground);
}

#[test]
fn fe_decsc() {
    // ESC 7  (DECSC — Save Cursor)
    let actions = feed_all(b"\x1b7");
    assert!(actions.iter().all(|a| *a == Action::Skip));
    assert_eq!(feed_state(b"\x1b7"), State::Ground);
}

// --- EscIntermediate ---

#[test]
fn esc_intermediate_charset() {
    // ESC ( B  (designate G0 charset)
    let actions = feed_all(b"\x1b(B");
    assert!(actions.iter().all(|a| *a == Action::Skip));
    assert_eq!(feed_state(b"\x1b(B"), State::Ground);
}

#[test]
fn esc_intermediate_multiple() {
    // ESC SP F  (S7C1T)
    let actions = feed_all(b"\x1b F");
    assert!(actions.iter().all(|a| *a == Action::Skip));
    assert_eq!(feed_state(b"\x1b F"), State::Ground);
}

// --- C1 pass-through ---

#[test]
fn c1_bytes_emitted() {
    let mut p = Parser::new();
    for b in 0x80..=0x9F {
        assert_eq!(p.feed(b), Action::Emit, "C1 byte 0x{b:02X} should emit");
        assert_eq!(p.state(), State::Ground);
    }
}

#[test]
fn utf8_multibyte_preserved() {
    // ñ = 0xC3 0xB1 — 0xC3 is NOT in C1 range but 0x9B is CSI in C1.
    // Ensure no corruption.
    let mut p = Parser::new();
    assert_eq!(p.feed(0xC3), Action::Emit);
    assert_eq!(p.feed(0xB1), Action::Emit);
}

// --- Malformed sequences ---

#[test]
fn lone_esc_at_eof() {
    let actions = feed_all(b"\x1b");
    assert_eq!(actions, vec![Action::Skip]);
    // Parser stuck in EscapeStart — not ground.
    assert_eq!(feed_state(b"\x1b"), State::EscapeStart);
}

#[test]
fn esc_invalid_introducer() {
    // ESC followed by 0xFF (invalid) → Ground, skip both
    let mut p = Parser::new();
    assert_eq!(p.feed(0x1B), Action::Skip);
    assert_eq!(p.feed(0xFF), Action::Skip);
    assert_eq!(p.state(), State::Ground);
}

#[test]
fn csi_aborted_by_invalid_byte() {
    // ESC [ then 0x01 (invalid param) → abort to Ground
    let mut p = Parser::new();
    p.feed(0x1B);
    p.feed(b'[');
    assert_eq!(p.state(), State::CsiParam);
    assert_eq!(p.feed(0x01), Action::Skip);
    assert_eq!(p.state(), State::Ground);
}

#[test]
fn unterminated_osc_at_eof() {
    // ESC ] body — no terminator
    let state = feed_state(b"\x1b]unterminated");
    assert_eq!(state, State::OscString);
}

#[test]
fn esc_inside_osc_starts_new_escape() {
    // ESC ] body ESC [ 3 1 m  → OSC aborted, new CSI starts
    let mut p = Parser::new();
    for &b in b"\x1b]body" {
        p.feed(b);
    }
    assert_eq!(p.state(), State::OscString);
    // ESC inside OSC → OscStEsc
    p.feed(0x1B);
    assert_eq!(p.state(), State::OscStEsc);
    // '[' is not backslash → re-enter EscapeStart → CsiParam
    p.feed(b'[');
    assert_eq!(p.state(), State::CsiParam);
    p.feed(b'm');
    assert_eq!(p.state(), State::Ground);
}

#[test]
fn esc_inside_dcs_starts_new_escape() {
    let mut p = Parser::new();
    for &b in b"\x1bPqdata" {
        p.feed(b);
    }
    assert_eq!(p.state(), State::DcsPassthrough);
    p.feed(0x1B);
    assert_eq!(p.state(), State::DcsStEsc);
    // Not backslash → re-enter as new escape
    p.feed(b'[');
    assert_eq!(p.state(), State::CsiParam);
}

#[test]
fn esc_inside_string_passthrough() {
    let mut p = Parser::new();
    for &b in b"\x1b_data" {
        p.feed(b);
    }
    assert_eq!(p.state(), State::StringPassthrough);
    p.feed(0x1B);
    assert_eq!(p.state(), State::StringStEsc);
    p.feed(b']');
    assert_eq!(p.state(), State::OscString);
}

#[test]
fn st_esc_reentry_bounded() {
    // OscStEsc with non-backslash → EscapeStart → processes byte.
    // This tests the loop runs exactly twice (StEsc → EscapeStart → match).
    let mut p = Parser::new();
    for &b in b"\x1b]data" {
        p.feed(b);
    }
    p.feed(0x1B); // → OscStEsc
    assert_eq!(p.state(), State::OscStEsc);
    // Feed 'P' → not backslash → EscapeStart → DcsEntry
    p.feed(b'P');
    assert_eq!(p.state(), State::DcsEntry);
}

// --- Edge cases ---

#[test]
fn empty_input() {
    let actions = feed_all(b"");
    assert!(actions.is_empty());
}

#[test]
fn content_between_escapes() {
    let mut p = Parser::new();
    let input = b"\x1b[31mhello\x1b[0m";
    let actions: Vec<_> = input.iter().map(|&b| p.feed(b)).collect();
    // "hello" bytes should be Emit
    let hello_start = 5; // after ESC [ 3 1 m
    let hello_end = 10; // before ESC [ 0 m
    for (i, action) in actions.iter().enumerate() {
        if (hello_start..hello_end).contains(&i) {
            assert_eq!(*action, Action::Emit, "byte {i} should emit");
        } else {
            assert_eq!(*action, Action::Skip, "byte {i} should skip");
        }
    }
}

#[test]
fn parser_reset() {
    let mut p = Parser::new();
    p.feed(0x1B);
    assert_eq!(p.state(), State::EscapeStart);
    p.reset();
    assert_eq!(p.state(), State::Ground);
    assert!(p.is_ground());
}

#[test]
fn parser_default() {
    let p = Parser::default();
    assert!(p.is_ground());
}

#[test]
fn parser_is_copy() {
    let p = Parser::new();
    let p2 = p; // Copy
    assert_eq!(p, p2);
}

#[test]
fn parser_size() {
    assert_eq!(std::mem::size_of::<Parser>(), 1);
}

#[test]
fn action_size() {
    assert_eq!(std::mem::size_of::<Action>(), 1);
}

#[test]
fn state_size() {
    assert_eq!(std::mem::size_of::<State>(), 1);
}

#[test]
fn rapid_esc_toggling() {
    // Many ESC bytes in a row — parser should handle without accumulating state.
    // ESC in EscapeStart: 0x1B doesn't match any introducer → Ground, Skip.
    // Next ESC from Ground → EscapeStart, Skip. So pairs alternate.
    let mut p = Parser::new();
    for _ in 0..100 {
        assert_eq!(p.feed(0x1B), Action::Skip);
    }
    // 100 ESCs: even count. ESC→EscapeStart, ESC→Ground, ESC→EscapeStart, ...
    // After 100 (even), last pair ended at Ground.
    assert_eq!(p.state(), State::Ground);
}

#[test]
fn csi_intermediate_aborted_by_esc() {
    let mut p = Parser::new();
    for &b in b"\x1b[0 " {
        p.feed(b);
    }
    assert_eq!(p.state(), State::CsiIntermediate);
    p.feed(0x1B);
    assert_eq!(p.state(), State::EscapeStart);
}

#[test]
fn esc_intermediate_aborted_by_esc() {
    let mut p = Parser::new();
    p.feed(0x1B);
    p.feed(b'('); // intermediate
    assert_eq!(p.state(), State::EscIntermediate);
    p.feed(0x1B);
    assert_eq!(p.state(), State::EscapeStart);
}

#[test]
fn dcs_param_to_passthrough() {
    let mut p = Parser::new();
    for &b in b"\x1bP1" {
        p.feed(b);
    }
    assert_eq!(p.state(), State::DcsParam);
    p.feed(b'q'); // final → DcsPassthrough
    assert_eq!(p.state(), State::DcsPassthrough);
}
