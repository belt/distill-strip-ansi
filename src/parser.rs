/// Action returned by [`Parser::feed`]: emit the byte as content or skip it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Action {
    /// Byte is content — emit it.
    Emit = 0,
    /// Byte is part of an escape sequence — skip it.
    Skip = 1,
}

/// ECMA-48 parser states.
///
/// 13 variants covering CSI, OSC, DCS, APC/PM/SOS (collapsed),
/// SS2, SS3, Fe, and intermediate bytes. SOS/PM/APC share
/// `StringPassthrough` since their behavior is identical.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum State {
    Ground = 0,
    EscapeStart,
    CsiParam,
    CsiIntermediate,
    OscString,
    OscStEsc,
    DcsEntry,
    DcsParam,
    DcsPassthrough,
    DcsStEsc,
    /// SOS, PM, APC collapsed — all consume until ST.
    StringPassthrough,
    StringStEsc,
    Ss2,
    Ss3,
    EscIntermediate,
}

/// Minimal ECMA-48 state machine for classifying bytes as
/// escape-sequence or content. Does not interpret sequences.
///
/// Size: 1 byte (just the [`State`] enum).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Parser {
    state: State,
}

// Compile-time assertions: Send + Sync.
const _: () = {
    const fn assert_send<T: Send>() {}
    const fn assert_sync<T: Sync>() {}
    assert_send::<Parser>();
    assert_sync::<Parser>();
};

impl Parser {
    /// Create a new parser in the ground state.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: State::Ground,
        }
    }

    /// Reset the parser to the ground state.
    #[inline]
    pub fn reset(&mut self) {
        self.state = State::Ground;
    }

    /// Returns `true` if the parser is in the ground state.
    #[inline]
    #[must_use]
    pub const fn is_ground(&self) -> bool {
        matches!(self.state, State::Ground)
    }

    /// Returns the current parser state.
    #[inline]
    #[must_use]
    pub const fn state(&self) -> State {
        self.state
    }

    /// Feed a single byte through the state machine.
    ///
    /// Returns [`Action::Emit`] if the byte is content,
    /// [`Action::Skip`] if it is part of an escape sequence.
    ///
    /// The StEsc re-entry loop runs at most twice (no recursion).
    #[inline]
    pub fn feed(&mut self, byte: u8) -> Action {
        // StEsc states may re-enter EscapeStart once.
        // The loop runs at most 2 iterations.
        let b = byte;
        loop {
            match self.state {
                State::Ground => {
                    if b == 0x1B {
                        self.state = State::EscapeStart;
                        return Action::Skip;
                    }
                    return Action::Emit;
                }
                State::EscapeStart => {
                    match b {
                        b'[' => self.state = State::CsiParam,
                        b']' => self.state = State::OscString,
                        b'P' => self.state = State::DcsEntry,
                        b'X' | b'^' | b'_' => self.state = State::StringPassthrough,
                        b'N' => self.state = State::Ss2,
                        b'O' => self.state = State::Ss3,
                        0x20..=0x2F => self.state = State::EscIntermediate,
                        0x30..=0x7E => self.state = State::Ground,
                        _ => self.state = State::Ground,
                    }
                    return Action::Skip;
                }
                State::CsiParam => match b {
                    0x30..=0x3F => return Action::Skip,
                    0x20..=0x2F => {
                        self.state = State::CsiIntermediate;
                        return Action::Skip;
                    }
                    0x40..=0x7E => {
                        self.state = State::Ground;
                        return Action::Skip;
                    }
                    0x1B => {
                        self.state = State::EscapeStart;
                        return Action::Skip;
                    }
                    _ => {
                        self.state = State::Ground;
                        return Action::Skip;
                    }
                },
                State::CsiIntermediate => match b {
                    0x20..=0x2F => return Action::Skip,
                    0x40..=0x7E => {
                        self.state = State::Ground;
                        return Action::Skip;
                    }
                    0x1B => {
                        self.state = State::EscapeStart;
                        return Action::Skip;
                    }
                    _ => {
                        self.state = State::Ground;
                        return Action::Skip;
                    }
                },
                State::OscString => match b {
                    0x07 => {
                        self.state = State::Ground;
                        return Action::Skip;
                    }
                    0x1B => {
                        self.state = State::OscStEsc;
                        return Action::Skip;
                    }
                    _ => return Action::Skip,
                },
                State::OscStEsc => {
                    if b == b'\\' {
                        self.state = State::Ground;
                        return Action::Skip;
                    }
                    // Not ST — re-enter as new escape.
                    self.state = State::EscapeStart;
                    continue;
                }
                State::DcsEntry => match b {
                    0x30..=0x3F => {
                        self.state = State::DcsParam;
                        return Action::Skip;
                    }
                    0x40..=0x7E => {
                        self.state = State::DcsPassthrough;
                        return Action::Skip;
                    }
                    0x1B => {
                        self.state = State::DcsStEsc;
                        return Action::Skip;
                    }
                    _ => {
                        self.state = State::DcsPassthrough;
                        return Action::Skip;
                    }
                },
                State::DcsParam => match b {
                    0x30..=0x3F => return Action::Skip,
                    0x40..=0x7E => {
                        self.state = State::DcsPassthrough;
                        return Action::Skip;
                    }
                    0x1B => {
                        self.state = State::DcsStEsc;
                        return Action::Skip;
                    }
                    _ => {
                        self.state = State::DcsPassthrough;
                        return Action::Skip;
                    }
                },
                State::DcsPassthrough => match b {
                    0x1B => {
                        self.state = State::DcsStEsc;
                        return Action::Skip;
                    }
                    _ => return Action::Skip,
                },
                State::DcsStEsc => {
                    if b == b'\\' {
                        self.state = State::Ground;
                        return Action::Skip;
                    }
                    self.state = State::EscapeStart;
                    continue;
                }
                State::StringPassthrough => match b {
                    0x1B => {
                        self.state = State::StringStEsc;
                        return Action::Skip;
                    }
                    _ => return Action::Skip,
                },
                State::StringStEsc => {
                    if b == b'\\' {
                        self.state = State::Ground;
                        return Action::Skip;
                    }
                    self.state = State::EscapeStart;
                    continue;
                }
                State::Ss2 | State::Ss3 => {
                    self.state = State::Ground;
                    return Action::Skip;
                }
                State::EscIntermediate => match b {
                    0x20..=0x2F => return Action::Skip,
                    0x30..=0x7E => {
                        self.state = State::Ground;
                        return Action::Skip;
                    }
                    0x1B => {
                        self.state = State::EscapeStart;
                        return Action::Skip;
                    }
                    _ => {
                        self.state = State::Ground;
                        return Action::Skip;
                    }
                },
            }
        }
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}
