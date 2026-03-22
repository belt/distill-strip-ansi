//! Sequence classifier for ANSI escape sequences.
//!
//! Wraps [`Parser`] to track sequence boundaries and identify the
//! [`SeqKind`] of each escape sequence without changing the underlying
//! state machine. Zero heap allocations.

#![forbid(unsafe_code)]

use crate::parser::{Action, Parser};

// ── SeqGroup ────────────────────────────────────────────────────────

/// Top-level escape sequence groups.
///
/// Each variant maps to a single bit in the [`FilterConfig`] preserved
/// bit-field (see design doc for layout).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SeqGroup {
    /// CSI — Control Sequence Introducer (`ESC [`)
    Csi = 0,
    /// OSC — Operating System Command (`ESC ]`)
    Osc = 1,
    /// DCS — Device Control String (`ESC P`)
    Dcs = 2,
    /// APC — Application Program Command (`ESC _`)
    Apc = 3,
    /// PM — Privacy Message (`ESC ^`)
    Pm = 4,
    /// SOS — Start of String (`ESC X`)
    Sos = 5,
    /// SS2 — Single Shift Two (`ESC N`)
    Ss2 = 6,
    /// SS3 — Single Shift Three (`ESC O`)
    Ss3 = 7,
    /// Fe — Other escape sequences in the 0x40..0x5F range
    Fe = 8,
}

// ── SeqKind ─────────────────────────────────────────────────────────

/// Specific escape sequence type, including CSI sub-groups.
///
/// Stored as a single byte (`#[repr(u8)]`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SeqKind {
    // CSI sub-groups (classified by final byte)
    /// `m` — Select Graphic Rendition
    CsiSgr = 0,
    /// `A`..`H`, `f` — cursor movement
    CsiCursor = 1,
    /// `J`, `K` — erase in display/line
    CsiErase = 2,
    /// `S`, `T` — scroll up/down
    CsiScroll = 3,
    /// `h`, `l` — set/reset mode
    CsiMode = 4,
    /// `n`, `c` — device status report
    CsiDeviceStatus = 5,
    /// `t` — window manipulation
    CsiWindow = 6,
    /// Any other CSI final byte
    CsiOther = 7,

    // Top-level groups (no sub-classification)
    /// OSC — Operating System Command
    Osc = 8,
    /// DCS — Device Control String
    Dcs = 9,
    /// APC — Application Program Command
    Apc = 10,
    /// PM — Privacy Message
    Pm = 11,
    /// SOS — Start of String
    Sos = 12,
    /// SS2 — Single Shift Two
    Ss2 = 13,
    /// SS3 — Single Shift Three
    Ss3 = 14,
    /// Fe — Other escape sequences
    Fe = 15,

    /// Unknown or malformed sequence
    Unknown = 16,
}

impl SeqKind {
    /// Returns the parent [`SeqGroup`] for this kind.
    #[inline]
    #[must_use]
    pub const fn group(self) -> SeqGroup {
        match self {
            Self::CsiSgr
            | Self::CsiCursor
            | Self::CsiErase
            | Self::CsiScroll
            | Self::CsiMode
            | Self::CsiDeviceStatus
            | Self::CsiWindow
            | Self::CsiOther => SeqGroup::Csi,

            Self::Osc => SeqGroup::Osc,
            Self::Dcs => SeqGroup::Dcs,
            Self::Apc => SeqGroup::Apc,
            Self::Pm => SeqGroup::Pm,
            Self::Sos => SeqGroup::Sos,
            Self::Ss2 => SeqGroup::Ss2,
            Self::Ss3 => SeqGroup::Ss3,
            Self::Fe => SeqGroup::Fe,

            // Unknown defaults to Fe (most conservative group)
            Self::Unknown => SeqGroup::Fe,
        }
    }
}

// ── SeqAction ───────────────────────────────────────────────────────

/// Action returned by [`ClassifyingParser::feed`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SeqAction {
    /// Content byte — emit it.
    Emit,
    /// ESC byte entering a new sequence.
    StartSeq,
    /// Body byte inside a sequence.
    InSeq,
    /// Final byte — sequence complete.
    EndSeq,
}

// ── ClassifyingParser ───────────────────────────────────────────────

/// Wraps [`Parser`] to track sequence type at boundaries.
///
/// Size: 3 bytes (1-byte `Parser` + 1-byte `SeqKind` + 1-byte `bool`).
/// Zero heap allocations.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ClassifyingParser {
    parser: Parser,
    kind: SeqKind,
    in_seq: bool,
}

// Compile-time size assertion: 3 bytes.
const _: () = assert!(core::mem::size_of::<ClassifyingParser>() == 3);

// Compile-time assertions: Send + Sync.
const _: () = {
    const fn assert_send<T: Send>() {}
    const fn assert_sync<T: Sync>() {}
    assert_send::<ClassifyingParser>();
    assert_sync::<ClassifyingParser>();
};

impl ClassifyingParser {
    /// Create a new classifying parser in the ground state.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            parser: Parser::new(),
            kind: SeqKind::Unknown,
            in_seq: false,
        }
    }

    /// Reset the parser to the ground state.
    #[inline]
    pub fn reset(&mut self) {
        self.parser.reset();
        self.kind = SeqKind::Unknown;
        self.in_seq = false;
    }

    /// Returns `true` if the inner parser is in the ground state.
    #[inline]
    #[must_use]
    pub fn is_ground(&self) -> bool {
        self.parser.is_ground()
    }

    /// Returns the current sequence kind.
    #[inline]
    #[must_use]
    pub fn current_kind(&self) -> SeqKind {
        self.kind
    }

    /// Feed a single byte through the classifying parser.
    ///
    /// Returns a [`SeqAction`] indicating the role of the byte.
    /// The inner [`Parser`] is always advanced.
    #[inline]
    pub fn feed(&mut self, byte: u8) -> SeqAction {
        let prev_ground = self.parser.is_ground();
        let action = self.parser.feed(byte);
        let now_ground = self.parser.is_ground();

        // Content byte while in ground state.
        if prev_ground && action == Action::Emit && now_ground {
            self.in_seq = false;
            return SeqAction::Emit;
        }

        // ESC byte leaving ground → start of new sequence.
        if prev_ground && action == Action::Skip && !now_ground {
            self.kind = SeqKind::Unknown;
            self.in_seq = true;
            return SeqAction::StartSeq;
        }

        // Body byte inside sequence (still not ground).
        if !prev_ground && action == Action::Skip && !now_ground {
            self.classify_if_introducer(byte);
            return SeqAction::InSeq;
        }

        // Final byte — sequence complete, parser returns to ground.
        if !prev_ground && action == Action::Skip && now_ground {
            // For Fe sequences (ESC + single byte in 0x40..0x5F), the
            // introducer IS the final byte — classify_if_introducer was
            // never called because there was no InSeq step.
            if self.kind == SeqKind::Unknown {
                self.classify_if_introducer(byte);
            }
            self.classify_csi_final(byte);
            self.in_seq = false;
            return SeqAction::EndSeq;
        }

        // Content byte emitted while inside a sequence (e.g. CAN/SUB abort).
        if !prev_ground && action == Action::Emit {
            self.in_seq = !now_ground;
            return SeqAction::Emit;
        }

        // Fallback — treat as content.
        self.in_seq = false;
        SeqAction::Emit
    }

    /// Classify the introducer byte to determine the top-level group.
    ///
    /// Only runs once per sequence (when `kind == Unknown`).
    #[inline]
    fn classify_if_introducer(&mut self, byte: u8) {
        if self.kind != SeqKind::Unknown {
            return;
        }
        self.kind = match byte {
            b'[' => SeqKind::CsiOther,
            b']' => SeqKind::Osc,
            b'P' => SeqKind::Dcs,
            b'X' => SeqKind::Sos,
            b'^' => SeqKind::Pm,
            b'_' => SeqKind::Apc,
            b'N' => SeqKind::Ss2,
            b'O' => SeqKind::Ss3,
            0x20..=0x7E => SeqKind::Fe,
            _ => SeqKind::Unknown,
        };
    }

    /// Refine CSI kind based on the final byte.
    ///
    /// Only runs when `kind == CsiOther` (i.e. we know it's CSI but
    /// haven't seen the final byte yet).
    #[inline]
    fn classify_csi_final(&mut self, byte: u8) {
        if self.kind != SeqKind::CsiOther {
            return;
        }
        self.kind = match byte {
            b'm' => SeqKind::CsiSgr,
            b'A'..=b'H' | b'f' => SeqKind::CsiCursor,
            b'J' | b'K' => SeqKind::CsiErase,
            b'S' | b'T' => SeqKind::CsiScroll,
            b'h' | b'l' => SeqKind::CsiMode,
            b'n' | b'c' => SeqKind::CsiDeviceStatus,
            b't' => SeqKind::CsiWindow,
            _ => SeqKind::CsiOther,
        };
    }
}

impl Default for ClassifyingParser {
    fn default() -> Self {
        Self::new()
    }
}
