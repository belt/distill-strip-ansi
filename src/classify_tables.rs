//! Lookup tables for ClassifyingParser — eliminates branches in the
//! hot path for sequence kind classification and parameter operations.
//!
//! Layer 1: STATE_TABLE (in state_table.rs) — base parser transitions.
//! Layer 2: These tables — kind classification + param operation dispatch.

use crate::classifier::SeqKind;

/// Maps ESC introducer byte → initial SeqKind.
/// Used on StartSeq (ESC leaving ground).
/// Index: byte value (0..255).
#[rustfmt::skip]
pub(crate) static INTRODUCER_KIND: [SeqKind; 256] = {
    let mut t = [SeqKind::Unknown; 256];
    t[b'[' as usize] = SeqKind::CsiOther;
    t[b']' as usize] = SeqKind::Osc;
    t[b'P' as usize] = SeqKind::Dcs;
    t[b'X' as usize] = SeqKind::Sos;
    t[b'^' as usize] = SeqKind::Pm;
    t[b'_' as usize] = SeqKind::Apc;
    t[b'N' as usize] = SeqKind::Ss2;
    t[b'O' as usize] = SeqKind::Ss3;
    // Fe: 0x20..=0x7E (excluding the specific ones above).
    let mut b: usize = 0x20;
    while b <= 0x7E {
        if matches!(t[b], SeqKind::Unknown) {
            t[b] = SeqKind::Fe;
        }
        b += 1;
    }
    t
};

/// Maps CSI final byte → SeqKind (for non-param-dependent cases).
/// Param-dependent cases (t→21=CsiQuery, n→6=CsiQuery) are handled
/// with a post-lookup fixup — still just one branch instead of many.
/// Index: byte value (0..255).
#[rustfmt::skip]
pub(crate) static CSI_FINAL_KIND: [SeqKind; 256] = {
    let mut t = [SeqKind::CsiOther; 256];
    t[b'm' as usize] = SeqKind::CsiSgr;
    t[b'A' as usize] = SeqKind::CsiCursor;
    t[b'B' as usize] = SeqKind::CsiCursor;
    t[b'C' as usize] = SeqKind::CsiCursor;
    t[b'D' as usize] = SeqKind::CsiCursor;
    t[b'E' as usize] = SeqKind::CsiCursor;
    t[b'F' as usize] = SeqKind::CsiCursor;
    t[b'G' as usize] = SeqKind::CsiCursor;
    t[b'H' as usize] = SeqKind::CsiCursor;
    t[b'f' as usize] = SeqKind::CsiCursor;
    t[b'J' as usize] = SeqKind::CsiErase;
    t[b'K' as usize] = SeqKind::CsiErase;
    t[b'S' as usize] = SeqKind::CsiScroll;
    t[b'T' as usize] = SeqKind::CsiScroll;
    t[b'h' as usize] = SeqKind::CsiMode;
    t[b'l' as usize] = SeqKind::CsiMode;
    t[b'n' as usize] = SeqKind::CsiDeviceStatus; // fixup: 6→CsiQuery
    t[b'c' as usize] = SeqKind::CsiDeviceStatus;
    t[b't' as usize] = SeqKind::CsiWindow;       // fixup: 21→CsiQuery
    t
};

/// Maps CSI final byte → the first_param value that triggers CsiQuery.
/// 0 = no query variant for this final byte.
#[rustfmt::skip]
pub(crate) static CSI_QUERY_PARAM: [u16; 256] = {
    let mut t = [0u16; 256];
    t[b't' as usize] = 21; // CSI 21t → title report
    t[b'n' as usize] = 6;  // CSI 6n → cursor position report
    t
};

/// Parameter operation for the InSeq phase.
/// Tells the classifier what to do with each byte during a sequence body.
#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ParamOp {
    /// No parameter operation (non-CSI body byte, or non-digit).
    Noop = 0,
    /// Digit: accumulate into param_value (param_value = param_value * 10 + digit).
    Digit = 1,
    /// Semicolon: finalize current param, reset accumulator.
    Semicolon = 2,
}

/// Maps byte → ParamOp for CSI parameter accumulation.
/// Index: byte value (0..255).
#[rustfmt::skip]
pub(crate) static PARAM_OP: [ParamOp; 256] = {
    let mut t = [ParamOp::Noop; 256];
    t[b'0' as usize] = ParamOp::Digit;
    t[b'1' as usize] = ParamOp::Digit;
    t[b'2' as usize] = ParamOp::Digit;
    t[b'3' as usize] = ParamOp::Digit;
    t[b'4' as usize] = ParamOp::Digit;
    t[b'5' as usize] = ParamOp::Digit;
    t[b'6' as usize] = ParamOp::Digit;
    t[b'7' as usize] = ParamOp::Digit;
    t[b'8' as usize] = ParamOp::Digit;
    t[b'9' as usize] = ParamOp::Digit;
    t[b';' as usize] = ParamOp::Semicolon;
    t
};

/// SGR finalization result: packed (new_param_state, sgr_content_bits).
/// Encoding: bits [7:4] = new ParamState (0-5), bits [3:0] = SgrContent bits to OR.
/// Index: [param_state as usize][param_value.min(255) as usize]
///
/// ParamState: 0=Inactive, 1=Normal, 2=AwaitMode, 3=Skip1, 4=Skip2, 5=Skip3
/// SgrContent: bit 0=BASIC(1), bit 1=EXTENDED(2), bit 2=TRUECOLOR(4)
#[rustfmt::skip]
pub(crate) static SGR_TABLE: [[u8; 256]; 6] = {
    // Helper: pack (state, bits) into u8.
    // state in high nibble, bits in low nibble.
    const fn pack(state: u8, bits: u8) -> u8 {
        (state << 4) | (bits & 0x0F)
    }
    const NORMAL: u8 = 1;
    const AWAIT: u8 = 2;
    const SKIP1: u8 = 3;
    const SKIP2: u8 = 4;
    const SKIP3: u8 = 5;
    const INACTIVE: u8 = 0;
    const BASIC: u8 = 1;
    const EXTENDED: u8 = 2;
    const TRUECOLOR: u8 = 4;

    let mut t = [[pack(NORMAL, 0); 256]; 6]; // default: stay Normal, no bits

    // State 0: Inactive — no-op for all values.
    {
        let mut v = 0;
        while v < 256 {
            t[0][v] = pack(INACTIVE, 0);
            v += 1;
        }
    }

    // State 1: Normal
    {
        // 0-29: BASIC
        let mut v = 0;
        while v <= 29 { t[1][v] = pack(NORMAL, BASIC); v += 1; }
        // 38, 48: transition to AwaitMode, no bits
        t[1][38] = pack(AWAIT, 0);
        t[1][48] = pack(AWAIT, 0);
        // 39, 49: BASIC
        t[1][39] = pack(NORMAL, BASIC);
        t[1][49] = pack(NORMAL, BASIC);
        // 90-97: BASIC
        let mut v = 90;
        while v <= 97 { t[1][v] = pack(NORMAL, BASIC); v += 1; }
        // 100-107: BASIC
        let mut v = 100;
        while v <= 107 { t[1][v] = pack(NORMAL, BASIC); v += 1; }
        // Everything else 30-37, 40-47, 50-89, 98-99, 108-255: stay Normal, no bits
        // (already default)
    }

    // State 2: AwaitMode
    {
        let mut v = 0;
        while v < 256 {
            t[2][v] = pack(NORMAL, 0); // default: malformed, recover to Normal
            v += 1;
        }
        t[2][5] = pack(SKIP1, EXTENDED);   // 38;5;N → EXTENDED
        t[2][2] = pack(SKIP3, TRUECOLOR);  // 38;2;R;G;B → TRUECOLOR
    }

    // State 3: Skip1 → Normal
    {
        let mut v = 0;
        while v < 256 { t[3][v] = pack(NORMAL, 0); v += 1; }
    }

    // State 4: Skip2 → Skip1
    {
        let mut v = 0;
        while v < 256 { t[4][v] = pack(SKIP1, 0); v += 1; }
    }

    // State 5: Skip3 → Skip2
    {
        let mut v = 0;
        while v < 256 { t[5][v] = pack(SKIP2, 0); v += 1; }
    }

    t
};

/// DCS phase transition table.
/// Index: [phase][byte] → new_phase | 0x80 (bit 7 = set DCS_IS_QUERY flag).
/// Phase 0: skip introducer. Phase 1: expect '$'. Phase 2: expect 'q'. Phase 3: done.
#[rustfmt::skip]
pub(crate) static DCS_PHASE: [[u8; 256]; 4] = {
    let mut t = [[3u8; 256]; 4]; // default: phase 3 (done)

    // Phase 0: always advance to phase 1.
    {
        let mut b = 0;
        while b < 256 { t[0][b] = 1; b += 1; }
    }

    // Phase 1: '$' → phase 2, anything else → phase 3.
    t[1][b'$' as usize] = 2;
    // (rest already 3)

    // Phase 2: 'q' → phase 3 + set query flag, anything else → phase 3.
    t[2][b'q' as usize] = 3 | 0x80; // 0x83 = phase 3 + query flag
    // (rest already 3)

    // Phase 3: all done, no-op.
    // (already 3)

    t
};
