//! Sequence classifier for ANSI escape sequences.
//!
//! Wraps [`Parser`] to track sequence boundaries and identify the
//! [`SeqKind`] of each escape sequence without changing the underlying
//! state machine. Zero heap allocations.

#![forbid(unsafe_code)]

use crate::parser::{Action, Parser};

// ── SgrContent ──────────────────────────────────────────────────────

/// Bitfield describing which SGR color depths a sequence contains.
///
/// This is a set membership type, not a ranking. A single SGR sequence
/// can set multiple bits simultaneously (e.g. `1;38;2;255;0;0;4m`
/// yields `BASIC | TRUECOLOR`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub struct SgrContent(pub u8);

impl SgrContent {
    /// Basic SGR params: 0–29, 39, 49, 90–97, 100–107.
    pub const BASIC: Self = Self(0b001);
    /// Extended 256-color: `38;5;N` or `48;5;N`.
    pub const EXTENDED: Self = Self(0b010);
    /// Truecolor RGB: `38;2;R;G;B` or `48;2;R;G;B`.
    pub const TRUECOLOR: Self = Self(0b100);

    /// Returns an empty (no bits set) `SgrContent`.
    #[inline]
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Returns `true` if no bits are set.
    #[inline]
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns `true` if `other`'s bits are all set in `self`.
    #[inline]
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Bitwise OR — accumulate bits from another `SgrContent`.
    #[inline]
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl core::ops::BitOr for SgrContent {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for SgrContent {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

// ── OscType ─────────────────────────────────────────────────────────

/// OSC sequence sub-type, classified by the first numeric parameter.
///
/// Determined by the numeric value before the first `;` separator
/// (or at the terminator for sequences with no `;`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
#[repr(u8)]
pub enum OscType {
    /// Unknown — not yet classified (initial state).
    #[default]
    Unknown = 0,
    /// Title — OSC 0, 1, or 2 (icon name / window title).
    Title = 1,
    /// WorkingDir — OSC 7 (current working directory).
    WorkingDir = 2,
    /// Hyperlink — OSC 8.
    Hyperlink = 3,
    /// Notify — OSC 9 or 777.
    Notify = 4,
    /// Clipboard — OSC 52.
    Clipboard = 5,
    /// ShellInteg — OSC 133 (shell integration / semantic prompts).
    ShellInteg = 6,
    /// ITerm2 — OSC 1337 (iTerm2 proprietary).
    ITerm2 = 7,
    /// Other — any OSC number not listed above.
    Other = 8,
}

/// Map a raw OSC number to an [`OscType`] variant.
#[inline]
#[must_use]
pub fn map_osc_number(n: u16) -> OscType {
    match n {
        0 | 1 | 2 => OscType::Title,
        7 => OscType::WorkingDir,
        8 => OscType::Hyperlink,
        9 | 777 => OscType::Notify,
        52 => OscType::Clipboard,
        133 => OscType::ShellInteg,
        1337 => OscType::ITerm2,
        _ => OscType::Other,
    }
}

// ── SeqDetail ───────────────────────────────────────────────────────

/// Snapshot of all classifier outputs at [`SeqAction::EndSeq`].
///
/// Returned by [`ClassifyingParser::detail()`]. Carries the full
/// per-sequence classification in one value so callers don't need to
/// call multiple accessors.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SeqDetail {
    /// The classified sequence kind.
    pub kind: SeqKind,
    /// SGR content bitfield (non-empty only for [`SeqKind::CsiSgr`]).
    pub sgr_content: SgrContent,
    /// OSC sub-type (non-Unknown only for [`SeqKind::Osc`]).
    pub osc_type: OscType,
    /// Raw OSC number (first numeric param; 0 for non-OSC sequences).
    pub osc_number: u16,
    /// First CSI parameter value (0 for non-CSI or no params).
    pub first_param: u16,
    /// Whether this DCS sequence is a DECRQSS query (`ESC P $ q`).
    pub dcs_is_query: bool,
}

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

    // (8–16 are non-CSI and Unknown — see below)

    /// `t` with first_param=21 or `n` with first_param=6 — dangerous
    /// query sequences (CVE-2003-0063 title report, cursor position report)
    CsiQuery = 17,

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
            | Self::CsiQuery
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

// ── ParamState ──────────────────────────────────────────────────────

/// State of the CSI parameter accumulator.
///
/// Stored as a single byte (`#[repr(u8)]`).
/// Only activates to `Normal` for CSI sequences; stays `Inactive` for
/// all other sequence types.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ParamState {
    /// Accumulator is inactive (non-CSI sequence or ground state).
    Inactive = 0,
    /// Accumulating a CSI parameter digit.
    Normal = 1,
    /// Awaiting a mode sub-parameter (after 38 or 48).
    AwaitMode = 2,
    /// Skipping 1 sub-parameter.
    Skip1 = 3,
    /// Skipping 2 sub-parameters.
    Skip2 = 4,
    /// Skipping 3 sub-parameters.
    Skip3 = 5,
}

// ── ClassifyingParser ───────────────────────────────────────────────

/// Wraps [`Parser`] to track sequence type at boundaries.
///
/// Zero heap allocations.
///
/// Layout (12 bytes):
/// ```text
///   parser       : Parser      1B
///   kind         : SeqKind     1B
///   flags        : u8          1B  (in_seq|seen_semicolon|osc_number_finalized|osc_accumulating)
///   sgr_content  : SgrContent  1B
///   param_value  : u16         2B
///   param_state  : ParamState  1B
///   osc_type     : OscType     1B
///   first_param  : u16         2B
///   osc_number   : u16         2B
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ClassifyingParser {
    parser: Parser,
    kind: SeqKind,
    /// Packed boolean flags:
    /// - bit 0: `in_seq`
    /// - bit 1: `seen_semicolon`
    /// - bit 2: `osc_number_finalized`
    /// - bit 3: `osc_accumulating`
    /// - bit 4: `dcs_is_query`
    /// - bits 5-6: DCS body inspection phase (0–3)
    flags: u8,
    /// Accumulated SGR content bits for the current CSI SGR sequence.
    ///
    /// Reset on `StartSeq` and CAN/SUB abort. OR-ed across all params.
    sgr_content: SgrContent,
    /// Shared digit accumulator for the current parameter.
    ///
    /// Used for CSI param accumulation and OSC number accumulation.
    /// CSI and OSC are mutually exclusive parser states, so sharing is safe.
    param_value: u16,
    /// State of the parameter accumulator (active only for CSI).
    param_state: ParamState,
    /// Classified OSC type for the current OSC sequence.
    ///
    /// Set when the OSC number is finalized (first `;` or terminator).
    /// Reset on `StartSeq` and CAN/SUB abort.
    osc_type: OscType,
    /// The first parameter value of the current CSI sequence.
    ///
    /// Set from `param_value` on the first `;` (multi-param), or from
    /// `param_value` at `EndSeq` (single-param, no `;` seen).
    first_param: u16,
    /// Raw OSC number (first numeric param) for the current OSC sequence.
    ///
    /// Copied from `param_value` when finalized. Retained even for
    /// `OscType::Other` so consumers can inspect the exact number.
    osc_number: u16,
}

// Flag bit positions.
const FLAG_IN_SEQ: u8 = 0b0000_0001;
const FLAG_SEEN_SEMICOLON: u8 = 0b0000_0010;
const FLAG_OSC_NUMBER_FINALIZED: u8 = 0b0000_0100;
const FLAG_OSC_ACCUMULATING: u8 = 0b0000_1000;
const FLAG_DCS_IS_QUERY: u8 = 0b0001_0000;

/// Mask and shift for the 2-bit DCS body inspection counter (bits 5-6).
///
/// States: 0 = initial (skip introducer), 1 = awaiting first body byte,
/// 2 = awaiting second body byte (saw `$`), 3 = done.
const DCS_PHASE_MASK: u8 = 0b0110_0000;
const DCS_PHASE_SHIFT: u8 = 5;

// Compile-time assertions: Send + Sync + size ≤ 12 bytes.
const _: () = {
    const fn assert_send<T: Send>() {}
    const fn assert_sync<T: Sync>() {}
    assert_send::<ClassifyingParser>();
    assert_sync::<ClassifyingParser>();
    // P7: ClassifyingParser ≤ 12 bytes.
    assert!(core::mem::size_of::<ClassifyingParser>() <= 12);
};

impl ClassifyingParser {
    /// Create a new classifying parser in the ground state.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            parser: Parser::new(),
            kind: SeqKind::Unknown,
            flags: 0,
            sgr_content: SgrContent::empty(),
            param_value: 0,
            param_state: ParamState::Inactive,
            osc_type: OscType::Unknown,
            first_param: 0,
            osc_number: 0,
        }
    }

    /// Reset the parser to the ground state.
    #[inline]
    pub fn reset(&mut self) {
        self.parser.reset();
        self.kind = SeqKind::Unknown;
        self.flags = 0;
        self.sgr_content = SgrContent::empty();
        self.param_value = 0;
        self.param_state = ParamState::Inactive;
        self.osc_type = OscType::Unknown;
        self.first_param = 0;
        self.osc_number = 0;
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

    /// Returns the current accumulated parameter value.
    #[inline]
    #[must_use]
    pub fn param_value(&self) -> u16 {
        self.param_value
    }

    /// Returns the current parameter accumulator state.
    #[inline]
    #[must_use]
    pub fn param_state(&self) -> ParamState {
        self.param_state
    }

    /// Returns the first parameter of the current CSI sequence.
    ///
    /// Valid after [`SeqAction::EndSeq`] for CSI sequences.
    #[inline]
    #[must_use]
    pub fn first_param(&self) -> u16 {
        self.first_param
    }

    /// Returns the accumulated SGR content bitfield for the current sequence.
    ///
    /// Valid after [`SeqAction::EndSeq`] for CSI SGR sequences.
    /// Returns [`SgrContent::empty()`] for non-SGR sequences.
    #[inline]
    #[must_use]
    pub fn sgr_content(&self) -> SgrContent {
        self.sgr_content
    }

    /// Returns the OSC type for the current OSC sequence.
    ///
    /// Valid after [`SeqAction::EndSeq`] for OSC sequences.
    /// Returns [`OscType::Unknown`] for non-OSC sequences.
    #[inline]
    #[must_use]
    pub fn osc_type(&self) -> OscType {
        self.osc_type
    }

    /// Returns the raw OSC number (first numeric param) for the current OSC sequence.
    ///
    /// Valid after [`SeqAction::EndSeq`] for OSC sequences.
    /// Returns `0` for non-OSC sequences.
    #[inline]
    #[must_use]
    pub fn osc_number(&self) -> u16 {
        self.osc_number
    }

    /// Returns `true` if the current DCS sequence is a DECRQSS query (`ESC P $ q`).
    ///
    /// Valid after [`SeqAction::EndSeq`] for DCS sequences.
    /// Returns `false` for non-DCS sequences.
    #[inline]
    #[must_use]
    pub fn dcs_is_query(&self) -> bool {
        (self.flags & FLAG_DCS_IS_QUERY) != 0
    }

    /// Returns the 2-bit DCS body inspection phase (0–3).
    #[inline]
    fn dcs_phase(&self) -> u8 {
        (self.flags & DCS_PHASE_MASK) >> DCS_PHASE_SHIFT
    }

    /// Sets the 2-bit DCS body inspection phase (0–3).
    #[inline]
    fn set_dcs_phase(&mut self, phase: u8) {
        self.flags = (self.flags & !DCS_PHASE_MASK) | (phase << DCS_PHASE_SHIFT);
    }

    /// Snapshot all classifier outputs into a [`SeqDetail`].
    ///
    /// Call immediately after receiving [`SeqAction::EndSeq`] to
    /// capture the complete classification of the finished sequence.
    #[inline]
    #[must_use]
    pub fn detail(&self) -> SeqDetail {
        SeqDetail {
            kind: self.kind,
            sgr_content: self.sgr_content,
            osc_type: self.osc_type,
            osc_number: self.osc_number,
            first_param: self.first_param,
            dcs_is_query: self.dcs_is_query(),
        }
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
            self.flags &= !FLAG_IN_SEQ;
            return SeqAction::Emit;
        }

        // ESC byte leaving ground → start of new sequence.
        if prev_ground && action == Action::Skip && !now_ground {
            self.kind = SeqKind::Unknown;
            self.flags = FLAG_IN_SEQ;
            // Reset all accumulator fields on StartSeq.
            self.sgr_content = SgrContent::empty();
            self.param_value = 0;
            self.param_state = ParamState::Inactive;
            self.first_param = 0;
            self.osc_type = OscType::Unknown;
            self.osc_number = 0;
            return SeqAction::StartSeq;
        }

        // Body byte inside sequence (still not ground).
        if !prev_ground && action == Action::Skip && !now_ground {
            self.classify_if_introducer(byte);
            self.accumulate_param(byte);
            return SeqAction::InSeq;
        }

        // Final byte — sequence complete, parser returns to ground.
        if !prev_ground && action == Action::Skip && now_ground {
            // CAN (0x18) / SUB (0x1A) abort the sequence — treat as Emit
            // and reset all accumulator fields (independent of StartSeq).
            // Exception: SS2/SS3 legitimately consume any next byte
            // (including CAN/SUB) as the single character byte.
            if (byte == 0x18 || byte == 0x1A)
                && !matches!(self.kind, SeqKind::Ss2 | SeqKind::Ss3)
            {
                self.sgr_content = SgrContent::empty();
                self.param_value = 0;
                self.param_state = ParamState::Inactive;
                self.first_param = 0;
                self.osc_type = OscType::Unknown;
                self.osc_number = 0;
                self.flags = 0;
                return SeqAction::Emit;
            }
            // For Fe sequences (ESC + single byte in 0x40..0x5F), the
            // introducer IS the final byte — classify_if_introducer was
            // never called because there was no InSeq step.
            if self.kind == SeqKind::Unknown {
                self.classify_if_introducer(byte);
            }
            // Single-param case: no `;` was seen, capture first_param now.
            if self.param_state == ParamState::Normal
                && (self.flags & FLAG_SEEN_SEMICOLON) == 0
            {
                self.first_param = self.param_value;
            }
            // Finalize the last SGR parameter (the final byte acts as a
            // terminator for the last param, equivalent to a semicolon).
            if self.param_state != ParamState::Inactive {
                self.finalize_sgr_param();
            }
            // Finalize OSC number if not yet done (no `;` was seen before terminator).
            if self.kind == SeqKind::Osc
                && (self.flags & FLAG_OSC_NUMBER_FINALIZED) == 0
            {
                self.finalize_osc_number();
            }
            self.classify_csi_final(byte);
            self.flags &= !FLAG_IN_SEQ;
            return SeqAction::EndSeq;
        }

        // Content byte emitted while inside a sequence (e.g. CAN/SUB abort).
        if !prev_ground && action == Action::Emit {
            // Reset accumulator fields on CAN/SUB abort path.
            self.sgr_content = SgrContent::empty();
            self.param_value = 0;
            self.param_state = ParamState::Inactive;
            self.first_param = 0;
            self.osc_type = OscType::Unknown;
            self.osc_number = 0;
            self.flags = if now_ground { 0 } else { FLAG_IN_SEQ };
            return SeqAction::Emit;
        }

        // Fallback — treat as content.
        self.flags &= !FLAG_IN_SEQ;
        SeqAction::Emit
    }

    /// Accumulate CSI parameter bytes, or OSC number digits.
    ///
    /// Called during `InSeq` after the introducer has been classified.
    /// For CSI: only active when `param_state != Inactive`.
    /// For OSC: accumulates digits into `param_value` until first `;`.
    #[inline]
    fn accumulate_param(&mut self, byte: u8) {
        // Activate on the first body byte of a CSI sequence.
        if self.param_state == ParamState::Inactive
            && self.kind == SeqKind::CsiOther
        {
            self.param_state = ParamState::Normal;
        }

        // Activate OSC accumulation on the first body byte of an OSC sequence.
        if self.kind == SeqKind::Osc
            && (self.flags & FLAG_OSC_NUMBER_FINALIZED) == 0
            && (self.flags & FLAG_OSC_ACCUMULATING) == 0
        {
            self.flags |= FLAG_OSC_ACCUMULATING;
            // The introducer byte (']') is not part of the OSC number.
            // Return here; the next byte will be the first digit.
            return;
        }

        // OSC number accumulation path.
        if (self.flags & FLAG_OSC_ACCUMULATING) != 0 {
            match byte {
                // Digit: accumulate into param_value.
                0x30..=0x39 => {
                    let digit = u16::from(byte - b'0');
                    self.param_value = self
                        .param_value
                        .saturating_mul(10)
                        .saturating_add(digit);
                }
                // Semicolon: finalize OSC number.
                0x3B => {
                    self.finalize_osc_number();
                }
                _ => {
                    // Non-digit, non-semicolon: finalize with what we have.
                    self.finalize_osc_number();
                }
            }
            return;
        }

        // DCS body inspection: detect DECRQSS ($q) pattern.
        //
        // Phase 0: introducer byte (skip it).
        // Phase 1: first body byte — expect $ (0x24).
        // Phase 2: second body byte — expect q (0x71).
        // Phase 3: done (no more inspection).
        if self.kind == SeqKind::Dcs {
            let phase = self.dcs_phase();
            match phase {
                0 => {
                    // Skip the introducer byte ('P').
                    self.set_dcs_phase(1);
                }
                1 => {
                    if byte == 0x24 {
                        // Saw '$' — advance to phase 2.
                        self.set_dcs_phase(2);
                    } else {
                        // Not '$' — done, not a query.
                        self.set_dcs_phase(3);
                    }
                }
                2 => {
                    if byte == 0x71 {
                        // Saw 'q' after '$' — this is DECRQSS.
                        self.flags |= FLAG_DCS_IS_QUERY;
                    }
                    // Done inspecting either way.
                    self.set_dcs_phase(3);
                }
                _ => {} // Phase 3: done, no-op.
            }
            return;
        }

        if self.param_state == ParamState::Inactive {
            return;
        }

        match byte {
            // Digit: accumulate into param_value, saturating at u16::MAX.
            0x30..=0x39 => {
                let digit = u16::from(byte - b'0');
                self.param_value = self
                    .param_value
                    .saturating_mul(10)
                    .saturating_add(digit);
            }
            // Semicolon: finalize current param, reset accumulator.
            0x3B => {
                // Capture first_param on the first semicolon.
                if (self.flags & FLAG_SEEN_SEMICOLON) == 0 {
                    self.first_param = self.param_value;
                    self.flags |= FLAG_SEEN_SEMICOLON;
                }
                self.finalize_sgr_param();
                self.param_value = 0;
            }
            _ => {}
        }
    }

    /// Finalize the OSC number from `param_value`.
    ///
    /// Called on the first `;` or at `EndSeq` for OSC sequences.
    #[inline]
    fn finalize_osc_number(&mut self) {
        self.osc_number = self.param_value;
        self.osc_type = map_osc_number(self.osc_number);
        self.flags |= FLAG_OSC_NUMBER_FINALIZED;
        self.flags &= !FLAG_OSC_ACCUMULATING;
    }

    /// Finalize the current SGR parameter value and update state machine.
    ///
    /// Called on `;` (mid-sequence) and at `EndSeq` for the final param.
    /// Only meaningful when `param_state != Inactive`.
    #[inline]
    fn finalize_sgr_param(&mut self) {
        let v = self.param_value;
        match self.param_state {
            ParamState::Normal => {
                match v {
                    // Transition to AwaitMode — do NOT set any bit yet.
                    38 | 48 => {
                        self.param_state = ParamState::AwaitMode;
                    }
                    // Basic SGR ranges: set BASIC bit, stay Normal.
                    0..=29 | 39 | 49 | 90..=97 | 100..=107 => {
                        self.sgr_content |= SgrContent::BASIC;
                    }
                    // Unknown param — stay Normal, ignore.
                    _ => {}
                }
            }
            ParamState::AwaitMode => {
                match v {
                    // 38;5;N or 48;5;N — set EXTENDED, skip 1 param.
                    5 => {
                        self.sgr_content |= SgrContent::EXTENDED;
                        self.param_state = ParamState::Skip1;
                    }
                    // 38;2;R;G;B or 48;2;R;G;B — set TRUECOLOR, skip 3 params.
                    2 => {
                        self.sgr_content |= SgrContent::TRUECOLOR;
                        self.param_state = ParamState::Skip3;
                    }
                    // Malformed — recover to Normal, no bit set.
                    _ => {
                        self.param_state = ParamState::Normal;
                    }
                }
            }
            // Skip states: decrement toward Normal.
            ParamState::Skip3 => {
                self.param_state = ParamState::Skip2;
            }
            ParamState::Skip2 => {
                self.param_state = ParamState::Skip1;
            }
            ParamState::Skip1 => {
                self.param_state = ParamState::Normal;
            }
            ParamState::Inactive => {}
        }
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
            b'n' => {
                // CSI 6n → cursor position report (echoback vector)
                // CSI 5n → device status report (benign)
                if self.first_param == 6 {
                    SeqKind::CsiQuery
                } else {
                    SeqKind::CsiDeviceStatus
                }
            }
            b'c' => SeqKind::CsiDeviceStatus,
            b't' => {
                // CSI 21t → title report (CVE-2003-0063, echoback vector)
                // CSI 8;H;Wt and others → window manipulation (benign)
                if self.first_param == 21 {
                    SeqKind::CsiQuery
                } else {
                    SeqKind::CsiWindow
                }
            }
            _ => SeqKind::CsiOther,
        };
    }
}

impl Default for ClassifyingParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feed a complete byte sequence through the parser and return the
    /// final `SeqAction` along with the parser state after the last byte.
    fn feed_all(p: &mut ClassifyingParser, bytes: &[u8]) -> SeqAction {
        let mut last = SeqAction::Emit;
        for &b in bytes {
            last = p.feed(b);
        }
        last
    }

    // ── Single-param CSI ────────────────────────────────────────────

    #[test]
    fn single_param_first_param_equals_param_value() {
        // ESC [ 5 m  →  first_param == 5, param_value == 5
        let mut p = ClassifyingParser::new();
        let action = feed_all(&mut p, b"\x1b[5m");
        assert_eq!(action, SeqAction::EndSeq);
        assert_eq!(p.first_param(), 5);
        assert_eq!(p.param_value(), 5);
    }

    #[test]
    fn single_param_zero() {
        // ESC [ 0 m  →  first_param == 0
        let mut p = ClassifyingParser::new();
        let action = feed_all(&mut p, b"\x1b[0m");
        assert_eq!(action, SeqAction::EndSeq);
        assert_eq!(p.first_param(), 0);
    }

    #[test]
    fn single_param_no_digits() {
        // ESC [ m  (no digits, implicit 0)  →  first_param == 0
        let mut p = ClassifyingParser::new();
        let action = feed_all(&mut p, b"\x1b[m");
        assert_eq!(action, SeqAction::EndSeq);
        assert_eq!(p.first_param(), 0);
    }

    // ── Multi-digit accumulation ────────────────────────────────────

    #[test]
    fn multi_digit_param_accumulates_correctly() {
        // ESC [ 3 8 m  →  param_value == 38
        let mut p = ClassifyingParser::new();
        let action = feed_all(&mut p, b"\x1b[38m");
        assert_eq!(action, SeqAction::EndSeq);
        assert_eq!(p.first_param(), 38);
        assert_eq!(p.param_value(), 38);
    }

    #[test]
    fn three_digit_param() {
        // ESC [ 1 0 7 m  →  first_param == 107
        let mut p = ClassifyingParser::new();
        let action = feed_all(&mut p, b"\x1b[107m");
        assert_eq!(action, SeqAction::EndSeq);
        assert_eq!(p.first_param(), 107);
    }

    // ── Multi-param CSI ─────────────────────────────────────────────

    #[test]
    fn multi_param_first_param_captured_at_semicolon() {
        // ESC [ 3 8 ; 5 ; 2 0 0 m  →  first_param == 38, param_value == 200
        let mut p = ClassifyingParser::new();
        let action = feed_all(&mut p, b"\x1b[38;5;200m");
        assert_eq!(action, SeqAction::EndSeq);
        assert_eq!(p.first_param(), 38);
        assert_eq!(p.param_value(), 200);
    }

    #[test]
    fn multi_param_first_param_zero() {
        // ESC [ 0 ; 1 m  →  first_param == 0 (not confused with "not set")
        let mut p = ClassifyingParser::new();
        let action = feed_all(&mut p, b"\x1b[0;1m");
        assert_eq!(action, SeqAction::EndSeq);
        assert_eq!(p.first_param(), 0);
        assert_eq!(p.param_value(), 1);
    }

    #[test]
    fn multi_param_second_semicolon_does_not_overwrite_first_param() {
        // ESC [ 1 ; 2 ; 3 m  →  first_param == 1 (not 2)
        let mut p = ClassifyingParser::new();
        let action = feed_all(&mut p, b"\x1b[1;2;3m");
        assert_eq!(action, SeqAction::EndSeq);
        assert_eq!(p.first_param(), 1);
        assert_eq!(p.param_value(), 3);
    }

    // ── Overflow saturation ─────────────────────────────────────────

    #[test]
    fn param_value_saturates_at_u16_max() {
        // Feed a number larger than u16::MAX (65535): "99999"
        let mut p = ClassifyingParser::new();
        let action = feed_all(&mut p, b"\x1b[99999m");
        assert_eq!(action, SeqAction::EndSeq);
        assert_eq!(p.param_value(), u16::MAX);
        assert_eq!(p.first_param(), u16::MAX);
    }

    #[test]
    fn param_value_does_not_wrap() {
        // 65536 would wrap to 0 without saturation; must stay at u16::MAX
        let mut p = ClassifyingParser::new();
        let action = feed_all(&mut p, b"\x1b[65536m");
        assert_eq!(action, SeqAction::EndSeq);
        assert_eq!(p.param_value(), u16::MAX);
    }

    // ── CAN/SUB abort reset ─────────────────────────────────────────

    #[test]
    fn can_abort_resets_accumulator() {
        // Start a CSI sequence, then send CAN (0x18) to abort.
        // After abort, all accumulator fields must be reset.
        let mut p = ClassifyingParser::new();
        // Feed ESC [ 3 8 (partial CSI)
        p.feed(0x1B);
        p.feed(b'[');
        p.feed(b'3');
        p.feed(b'8');
        // CAN aborts the sequence
        let action = p.feed(0x18);
        assert_eq!(action, SeqAction::Emit);
        assert_eq!(p.param_value(), 0);
        assert_eq!(p.param_state(), ParamState::Inactive);
        assert_eq!(p.first_param(), 0);
    }

    #[test]
    fn sub_abort_resets_accumulator() {
        // SUB (0x1A) also aborts.
        let mut p = ClassifyingParser::new();
        p.feed(0x1B);
        p.feed(b'[');
        p.feed(b'5');
        let action = p.feed(0x1A);
        assert_eq!(action, SeqAction::Emit);
        assert_eq!(p.param_value(), 0);
        assert_eq!(p.param_state(), ParamState::Inactive);
        assert_eq!(p.first_param(), 0);
    }

    #[test]
    fn after_can_abort_next_sequence_starts_clean() {
        // After a CAN abort, the next CSI sequence should accumulate fresh.
        let mut p = ClassifyingParser::new();
        // Partial sequence aborted by CAN
        p.feed(0x1B);
        p.feed(b'[');
        p.feed(b'9');
        p.feed(b'9');
        p.feed(0x18); // CAN
        // New sequence
        let action = feed_all(&mut p, b"\x1b[42m");
        assert_eq!(action, SeqAction::EndSeq);
        assert_eq!(p.first_param(), 42);
        assert_eq!(p.param_value(), 42);
    }

    // ── Non-CSI sequences ───────────────────────────────────────────

    #[test]
    fn osc_sequence_param_state_stays_inactive() {
        // OSC sequences should not activate the param accumulator.
        let mut p = ClassifyingParser::new();
        // ESC ] 0 ; title BEL
        let action = feed_all(&mut p, b"\x1b]0;title\x07");
        assert_eq!(action, SeqAction::EndSeq);
        assert_eq!(p.param_state(), ParamState::Inactive);
    }

    #[test]
    fn fe_sequence_param_state_stays_inactive() {
        // Fe sequences (ESC + single byte) should not activate accumulator.
        let mut p = ClassifyingParser::new();
        let action = feed_all(&mut p, b"\x1b=");
        assert_eq!(action, SeqAction::EndSeq);
        assert_eq!(p.param_state(), ParamState::Inactive);
    }

    // ── StartSeq reset ──────────────────────────────────────────────

    #[test]
    fn start_seq_resets_accumulator_from_previous_sequence() {
        // After a complete CSI sequence, starting a new one resets fields.
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, b"\x1b[38;5;200m");
        assert_eq!(p.first_param(), 38);
        // Start a new sequence — ESC resets everything
        p.feed(0x1B);
        assert_eq!(p.param_value(), 0);
        assert_eq!(p.param_state(), ParamState::Inactive);
        assert_eq!(p.first_param(), 0);
    }

    // ── SGR content: pure basic ─────────────────────────────────────

    #[test]
    fn sgr_basic_bold() {
        // ESC [ 1 m  →  BASIC
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, b"\x1b[1m");
        assert_eq!(p.sgr_content(), SgrContent::BASIC);
    }

    #[test]
    fn sgr_basic_reset() {
        // ESC [ 0 m  →  BASIC (0 is in 0-29 range)
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, b"\x1b[0m");
        assert_eq!(p.sgr_content(), SgrContent::BASIC);
    }

    #[test]
    fn sgr_basic_fg_default() {
        // ESC [ 3 9 m  →  BASIC (39 = default fg)
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, b"\x1b[39m");
        assert_eq!(p.sgr_content(), SgrContent::BASIC);
    }

    #[test]
    fn sgr_basic_bg_default() {
        // ESC [ 4 9 m  →  BASIC (49 = default bg)
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, b"\x1b[49m");
        assert_eq!(p.sgr_content(), SgrContent::BASIC);
    }

    #[test]
    fn sgr_basic_bright_fg() {
        // ESC [ 9 1 m  →  BASIC (90-97 range)
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, b"\x1b[91m");
        assert_eq!(p.sgr_content(), SgrContent::BASIC);
    }

    #[test]
    fn sgr_basic_bright_bg() {
        // ESC [ 1 0 3 m  →  BASIC (100-107 range)
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, b"\x1b[103m");
        assert_eq!(p.sgr_content(), SgrContent::BASIC);
    }

    #[test]
    fn sgr_basic_multiple_params() {
        // ESC [ 1 ; 4 ; 2 9 m  →  BASIC (all in 0-29)
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, b"\x1b[1;4;29m");
        assert_eq!(p.sgr_content(), SgrContent::BASIC);
    }

    // ── SGR content: pure extended ──────────────────────────────────

    #[test]
    fn sgr_extended_fg() {
        // ESC [ 3 8 ; 5 ; 2 0 0 m  →  EXTENDED
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, b"\x1b[38;5;200m");
        assert_eq!(p.sgr_content(), SgrContent::EXTENDED);
    }

    #[test]
    fn sgr_extended_bg() {
        // ESC [ 4 8 ; 5 ; 1 0 0 m  →  EXTENDED
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, b"\x1b[48;5;100m");
        assert_eq!(p.sgr_content(), SgrContent::EXTENDED);
    }

    // ── SGR content: pure truecolor ─────────────────────────────────

    #[test]
    fn sgr_truecolor_fg() {
        // ESC [ 3 8 ; 2 ; 2 5 5 ; 0 ; 0 m  →  TRUECOLOR
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, b"\x1b[38;2;255;0;0m");
        assert_eq!(p.sgr_content(), SgrContent::TRUECOLOR);
    }

    #[test]
    fn sgr_truecolor_bg() {
        // ESC [ 4 8 ; 2 ; 0 ; 1 2 8 ; 2 5 5 m  →  TRUECOLOR
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, b"\x1b[48;2;0;128;255m");
        assert_eq!(p.sgr_content(), SgrContent::TRUECOLOR);
    }

    // ── SGR content: mixed sequences ────────────────────────────────

    #[test]
    fn sgr_mixed_basic_and_truecolor() {
        // ESC [ 1 ; 3 8 ; 2 ; 2 5 5 ; 0 ; 0 ; 4 m  →  BASIC | TRUECOLOR
        // (bold=1 and underline=4 are basic; RGB color is truecolor)
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, b"\x1b[1;38;2;255;0;0;4m");
        assert_eq!(
            p.sgr_content(),
            SgrContent::BASIC | SgrContent::TRUECOLOR
        );
    }

    #[test]
    fn sgr_mixed_basic_and_extended() {
        // ESC [ 1 ; 3 8 ; 5 ; 2 0 0 m  →  BASIC | EXTENDED
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, b"\x1b[1;38;5;200m");
        assert_eq!(
            p.sgr_content(),
            SgrContent::BASIC | SgrContent::EXTENDED
        );
    }

    #[test]
    fn sgr_mixed_all_three() {
        // ESC [ 1 ; 3 8 ; 5 ; 1 0 0 ; 4 8 ; 2 ; 0 ; 0 ; 2 5 5 m
        // → BASIC | EXTENDED | TRUECOLOR
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, b"\x1b[1;38;5;100;48;2;0;0;255m");
        assert_eq!(
            p.sgr_content(),
            SgrContent::BASIC | SgrContent::EXTENDED | SgrContent::TRUECOLOR
        );
    }

    #[test]
    fn sgr_38_alone_sets_no_bit() {
        // ESC [ 3 8 m  →  38 alone (no mode) → no bit set (malformed)
        // 38 transitions to AwaitMode; final byte finalizes with no mode seen
        // AwaitMode + non-{2,5} → recover to Normal, no bit set
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, b"\x1b[38m");
        assert_eq!(p.sgr_content(), SgrContent::empty());
    }

    // ── SGR content: malformed recovery ─────────────────────────────

    #[test]
    fn sgr_malformed_38_no_mode() {
        // ESC [ 3 8 ; m  →  38 then empty param → AwaitMode sees 0 → recover
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, b"\x1b[38;m");
        assert_eq!(p.sgr_content(), SgrContent::empty());
        assert_eq!(p.param_state(), ParamState::Normal);
    }

    #[test]
    fn sgr_malformed_38_5_no_index() {
        // ESC [ 3 8 ; 5 ; m  →  EXTENDED set, Skip1 → Normal on empty param
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, b"\x1b[38;5;m");
        assert_eq!(p.sgr_content(), SgrContent::EXTENDED);
        assert_eq!(p.param_state(), ParamState::Normal);
    }

    #[test]
    fn sgr_malformed_48_2_no_rgb() {
        // ESC [ 4 8 ; 2 ; m  →  TRUECOLOR set, Skip3 → Skip2 on empty param
        // then final byte finalizes remaining skips
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, b"\x1b[48;2;m");
        assert_eq!(p.sgr_content(), SgrContent::TRUECOLOR);
    }

    #[test]
    fn sgr_trailing_semicolon() {
        // ESC [ 1 ; m  →  BASIC (1 is basic), trailing ; has empty param (0)
        // 0 is in 0-29 range → BASIC again (OR is idempotent)
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, b"\x1b[1;m");
        assert_eq!(p.sgr_content(), SgrContent::BASIC);
    }

    #[test]
    fn sgr_can_abort_resets_sgr_content() {
        // Partial SGR sequence aborted by CAN → sgr_content reset to empty
        let mut p = ClassifyingParser::new();
        p.feed(0x1B);
        p.feed(b'[');
        p.feed(b'1');
        p.feed(0x18); // CAN
        assert_eq!(p.sgr_content(), SgrContent::empty());
    }

    #[test]
    fn sgr_content_resets_on_new_sequence() {
        // After a complete SGR sequence, starting a new one resets sgr_content.
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, b"\x1b[1m");
        assert_eq!(p.sgr_content(), SgrContent::BASIC);
        // Start a new sequence
        p.feed(0x1B);
        assert_eq!(p.sgr_content(), SgrContent::empty());
    }

    #[test]
    fn sgr_non_sgr_csi_has_empty_content() {
        // Non-SGR CSI (e.g. cursor up ESC[1A) — the param accumulator runs
        // but sgr_content is only meaningful for CsiSgr sequences.
        // The kind should be CsiCursor, not CsiSgr.
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, b"\x1b[1A");
        assert_eq!(p.current_kind(), SeqKind::CsiCursor);
        // sgr_content is not meaningful for non-SGR sequences;
        // the important invariant is that kind != CsiSgr.
    }

    // ── OSC type mapping ────────────────────────────────────────────

    /// Helper: build an OSC sequence with a given numeric first param.
    fn osc_seq(n: u16) -> alloc::vec::Vec<u8> {
        let mut seq = alloc::vec![0x1B, b']'];
        seq.extend_from_slice(n.to_string().as_bytes());
        seq.push(b';');
        seq.push(0x07); // BEL terminator
        seq
    }

    /// Helper: build an OSC sequence terminated by ST (ESC \).
    fn osc_seq_st(n: u16) -> alloc::vec::Vec<u8> {
        let mut seq = alloc::vec![0x1B, b']'];
        seq.extend_from_slice(n.to_string().as_bytes());
        seq.push(b';');
        seq.push(0x1B);
        seq.push(b'\\');
        seq
    }

    #[test]
    fn osc_0_maps_to_title() {
        let mut p = ClassifyingParser::new();
        let action = feed_all(&mut p, &osc_seq(0));
        assert_eq!(action, SeqAction::EndSeq);
        assert_eq!(p.osc_type(), OscType::Title);
        assert_eq!(p.osc_number(), 0);
    }

    #[test]
    fn osc_1_maps_to_title() {
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, &osc_seq(1));
        assert_eq!(p.osc_type(), OscType::Title);
        assert_eq!(p.osc_number(), 1);
    }

    #[test]
    fn osc_2_maps_to_title() {
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, &osc_seq(2));
        assert_eq!(p.osc_type(), OscType::Title);
        assert_eq!(p.osc_number(), 2);
    }

    #[test]
    fn osc_7_maps_to_working_dir() {
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, &osc_seq(7));
        assert_eq!(p.osc_type(), OscType::WorkingDir);
        assert_eq!(p.osc_number(), 7);
    }

    #[test]
    fn osc_8_maps_to_hyperlink() {
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, &osc_seq(8));
        assert_eq!(p.osc_type(), OscType::Hyperlink);
        assert_eq!(p.osc_number(), 8);
    }

    #[test]
    fn osc_9_maps_to_notify() {
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, &osc_seq(9));
        assert_eq!(p.osc_type(), OscType::Notify);
        assert_eq!(p.osc_number(), 9);
    }

    #[test]
    fn osc_52_maps_to_clipboard() {
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, &osc_seq(52));
        assert_eq!(p.osc_type(), OscType::Clipboard);
        assert_eq!(p.osc_number(), 52);
    }

    #[test]
    fn osc_133_maps_to_shell_integ() {
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, &osc_seq(133));
        assert_eq!(p.osc_type(), OscType::ShellInteg);
        assert_eq!(p.osc_number(), 133);
    }

    #[test]
    fn osc_777_maps_to_notify() {
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, &osc_seq(777));
        assert_eq!(p.osc_type(), OscType::Notify);
        assert_eq!(p.osc_number(), 777);
    }

    #[test]
    fn osc_1337_maps_to_iterm2() {
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, &osc_seq(1337));
        assert_eq!(p.osc_type(), OscType::ITerm2);
        assert_eq!(p.osc_number(), 1337);
    }

    #[test]
    fn osc_50_maps_to_other() {
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, &osc_seq(50));
        assert_eq!(p.osc_type(), OscType::Other);
        assert_eq!(p.osc_number(), 50);
    }

    #[test]
    fn osc_999_maps_to_other() {
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, &osc_seq(999));
        assert_eq!(p.osc_type(), OscType::Other);
        assert_eq!(p.osc_number(), 999);
    }

    // ── OSC raw number preserved for Other ──────────────────────────

    #[test]
    fn osc_other_raw_number_preserved_50() {
        // OSC 50 → OscType::Other, osc_number == 50
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, &osc_seq(50));
        assert_eq!(p.osc_type(), OscType::Other);
        assert_eq!(p.osc_number(), 50);
    }

    #[test]
    fn osc_other_raw_number_preserved_st_terminator() {
        // OSC 50 terminated by ST (ESC \) → same result
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, &osc_seq_st(50));
        assert_eq!(p.osc_type(), OscType::Other);
        assert_eq!(p.osc_number(), 50);
    }

    #[test]
    fn osc_type_resets_on_new_sequence() {
        // After an OSC sequence, starting a new one resets osc_type.
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, &osc_seq(8));
        assert_eq!(p.osc_type(), OscType::Hyperlink);
        // Start a new sequence
        p.feed(0x1B);
        assert_eq!(p.osc_type(), OscType::Unknown);
        assert_eq!(p.osc_number(), 0);
    }

    #[test]
    fn osc_can_abort_resets_osc_fields() {
        // Partial OSC sequence aborted by CAN → osc_type/osc_number reset
        let mut p = ClassifyingParser::new();
        p.feed(0x1B);
        p.feed(b']');
        p.feed(b'8');
        p.feed(0x18); // CAN
        assert_eq!(p.osc_type(), OscType::Unknown);
        assert_eq!(p.osc_number(), 0);
    }

    #[test]
    fn osc_no_semicolon_finalized_at_terminator() {
        // OSC with no semicolon (just a number + BEL) → finalized at EndSeq
        let mut seq = alloc::vec![0x1B, b']'];
        seq.extend_from_slice(b"8");
        seq.push(0x07); // BEL with no semicolon
        let mut p = ClassifyingParser::new();
        let action = feed_all(&mut p, &seq);
        assert_eq!(action, SeqAction::EndSeq);
        assert_eq!(p.osc_type(), OscType::Hyperlink);
        assert_eq!(p.osc_number(), 8);
    }

    // ── CsiQuery sub-kind ───────────────────────────────────────────

    #[test]
    fn csi_21t_classifies_as_csi_query() {
        // ESC [ 2 1 t  →  CsiQuery (title report, CVE-2003-0063)
        let mut p = ClassifyingParser::new();
        let action = feed_all(&mut p, b"\x1b[21t");
        assert_eq!(action, SeqAction::EndSeq);
        assert_eq!(p.current_kind(), SeqKind::CsiQuery);
        assert_eq!(p.first_param(), 21);
    }

    #[test]
    fn csi_8_40_132t_classifies_as_csi_window() {
        // ESC [ 8 ; 4 0 ; 1 3 2 t  →  CsiWindow (resize, benign)
        // first_param=8, NOT 21, so must be CsiWindow
        let mut p = ClassifyingParser::new();
        let action = feed_all(&mut p, b"\x1b[8;40;132t");
        assert_eq!(action, SeqAction::EndSeq);
        assert_eq!(p.current_kind(), SeqKind::CsiWindow);
        assert_eq!(p.first_param(), 8);
    }

    #[test]
    fn csi_0t_classifies_as_csi_window() {
        // ESC [ 0 t  →  CsiWindow (first_param=0, not 21)
        let mut p = ClassifyingParser::new();
        let action = feed_all(&mut p, b"\x1b[0t");
        assert_eq!(action, SeqAction::EndSeq);
        assert_eq!(p.current_kind(), SeqKind::CsiWindow);
        assert_eq!(p.first_param(), 0);
    }

    #[test]
    fn csi_6n_classifies_as_csi_query() {
        // ESC [ 6 n  →  CsiQuery (cursor position report, echoback vector)
        let mut p = ClassifyingParser::new();
        let action = feed_all(&mut p, b"\x1b[6n");
        assert_eq!(action, SeqAction::EndSeq);
        assert_eq!(p.current_kind(), SeqKind::CsiQuery);
        assert_eq!(p.first_param(), 6);
    }

    #[test]
    fn csi_5n_classifies_as_csi_device_status() {
        // ESC [ 5 n  →  CsiDeviceStatus (device status report, benign)
        let mut p = ClassifyingParser::new();
        let action = feed_all(&mut p, b"\x1b[5n");
        assert_eq!(action, SeqAction::EndSeq);
        assert_eq!(p.current_kind(), SeqKind::CsiDeviceStatus);
        assert_eq!(p.first_param(), 5);
    }

    #[test]
    fn csi_query_group_is_csi() {
        // CsiQuery must map to SeqGroup::Csi
        assert_eq!(SeqKind::CsiQuery.group(), SeqGroup::Csi);
    }

    // ── DCS query detection ─────────────────────────────────────────

    #[test]
    fn dcs_dollar_q_sets_dcs_is_query() {
        // ESC P $ q <data> ESC \  →  DECRQSS, dcs_is_query = true
        let mut p = ClassifyingParser::new();
        let action = feed_all(&mut p, b"\x1bP$qm\x1b\\");
        assert_eq!(action, SeqAction::EndSeq);
        assert_eq!(p.current_kind(), SeqKind::Dcs);
        assert!(p.dcs_is_query());
    }

    #[test]
    fn dcs_dollar_q_detail_has_dcs_is_query() {
        // Verify SeqDetail.dcs_is_query is populated from the classifier.
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, b"\x1bP$qm\x1b\\");
        let d = p.detail();
        assert!(d.dcs_is_query);
        assert_eq!(d.kind, SeqKind::Dcs);
    }

    #[test]
    fn dcs_other_body_not_query() {
        // ESC P 0 ; 1 | data ESC \  →  DCS with other body, dcs_is_query = false
        let mut p = ClassifyingParser::new();
        let action = feed_all(&mut p, b"\x1bP0;1|data\x1b\\");
        assert_eq!(action, SeqAction::EndSeq);
        assert_eq!(p.current_kind(), SeqKind::Dcs);
        assert!(!p.dcs_is_query());
    }

    #[test]
    fn dcs_dollar_x_not_query() {
        // ESC P $ x <data> ESC \  →  $ then x (not q), dcs_is_query = false
        let mut p = ClassifyingParser::new();
        let action = feed_all(&mut p, b"\x1bP$x\x1b\\");
        assert_eq!(action, SeqAction::EndSeq);
        assert_eq!(p.current_kind(), SeqKind::Dcs);
        assert!(!p.dcs_is_query());
    }

    #[test]
    fn dcs_empty_body_not_query() {
        // ESC P ESC \  →  empty DCS body, dcs_is_query = false
        let mut p = ClassifyingParser::new();
        let action = feed_all(&mut p, b"\x1bP\x1b\\");
        assert_eq!(action, SeqAction::EndSeq);
        assert_eq!(p.current_kind(), SeqKind::Dcs);
        assert!(!p.dcs_is_query());
    }

    #[test]
    fn dcs_is_query_resets_on_new_sequence() {
        // After a DECRQSS, starting a new sequence resets dcs_is_query.
        let mut p = ClassifyingParser::new();
        feed_all(&mut p, b"\x1bP$qm\x1b\\");
        assert!(p.dcs_is_query());
        // Start a new sequence — ESC resets everything.
        p.feed(0x1B);
        assert!(!p.dcs_is_query());
    }

    #[test]
    fn dcs_is_query_resets_on_can_abort() {
        // Partial DCS $q aborted by CAN → dcs_is_query reset.
        let mut p = ClassifyingParser::new();
        p.feed(0x1B);
        p.feed(b'P');
        p.feed(b'$');
        p.feed(b'q');
        assert!(p.dcs_is_query());
        // CAN aborts the sequence.
        p.feed(0x18);
        assert!(!p.dcs_is_query());
    }

    #[test]
    fn dcs_is_query_resets_on_sub_abort() {
        // Partial DCS $q aborted by SUB → dcs_is_query reset.
        let mut p = ClassifyingParser::new();
        p.feed(0x1B);
        p.feed(b'P');
        p.feed(b'$');
        p.feed(b'q');
        assert!(p.dcs_is_query());
        // SUB aborts the sequence.
        p.feed(0x1A);
        assert!(!p.dcs_is_query());
    }

    #[test]
    fn dcs_q_without_dollar_not_query() {
        // ESC P q <data> ESC \  →  first body byte is q (not $), dcs_is_query = false
        let mut p = ClassifyingParser::new();
        let action = feed_all(&mut p, b"\x1bPq\x1b\\");
        assert_eq!(action, SeqAction::EndSeq);
        assert!(!p.dcs_is_query());
    }

    // ── Compile-time size assertion (P7) ────────────────────────────

    #[test]
    fn classifying_parser_size_le_12_bytes() {
        assert!(
            core::mem::size_of::<ClassifyingParser>() <= 12,
            "ClassifyingParser must be ≤ 12 bytes, got {}",
            core::mem::size_of::<ClassifyingParser>()
        );
    }
}

