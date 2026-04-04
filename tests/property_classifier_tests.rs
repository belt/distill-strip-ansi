#![cfg(feature = "filter")]

use proptest::prelude::*;
use strip_ansi::{ClassifyingParser, OscType, SeqAction, SeqKind, SgrContent, map_osc_number};

// ── Generators ──────────────────────────────────────────────────────

/// Generate a well-formed CSI sequence: ESC [ params final_byte
fn arb_ansi_csi() -> impl Strategy<Value = Vec<u8>> {
    (
        0u8..50,
        prop::collection::vec(0x30u8..=0x3F, 0..4),
        0x40u8..=0x7E,
    )
        .prop_map(|(code, params, final_byte)| {
            let mut seq = vec![0x1B, b'['];
            seq.extend_from_slice(code.to_string().as_bytes());
            for p in params {
                seq.push(b';');
                seq.push(p);
            }
            seq.push(final_byte);
            seq
        })
}

/// Generate a well-formed OSC sequence (BEL terminated).
fn arb_ansi_osc() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(0x20u8..=0x7E, 0..32).prop_map(|body| {
        let mut seq = vec![0x1B, b']'];
        seq.extend_from_slice(&body);
        seq.push(0x07);
        seq
    })
}

/// Generate a well-formed DCS sequence: ESC P body ESC \
fn arb_ansi_dcs() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(0x20u8..=0x7E, 0..32).prop_map(|body| {
        let mut seq = vec![0x1B, b'P'];
        seq.extend_from_slice(&body);
        seq.push(0x1B);
        seq.push(b'\\');
        seq
    })
}

/// Generate a well-formed APC sequence: ESC _ body ESC \
fn arb_ansi_apc() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(0x20u8..=0x7E, 0..32).prop_map(|body| {
        let mut seq = vec![0x1B, b'_'];
        seq.extend_from_slice(&body);
        seq.push(0x1B);
        seq.push(b'\\');
        seq
    })
}

/// Generate a well-formed PM sequence: ESC ^ body ESC \
fn arb_ansi_pm() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(0x20u8..=0x7E, 0..32).prop_map(|body| {
        let mut seq = vec![0x1B, b'^'];
        seq.extend_from_slice(&body);
        seq.push(0x1B);
        seq.push(b'\\');
        seq
    })
}

/// Generate a well-formed SOS sequence: ESC X body ESC \
fn arb_ansi_sos() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(0x20u8..=0x7E, 0..32).prop_map(|body| {
        let mut seq = vec![0x1B, b'X'];
        seq.extend_from_slice(&body);
        seq.push(0x1B);
        seq.push(b'\\');
        seq
    })
}

/// Generate an arbitrary well-formed ANSI sequence.
fn arb_ansi_sequence() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        arb_ansi_csi(),
        arb_ansi_osc(),
        arb_ansi_dcs(),
        arb_ansi_apc(),
        arb_ansi_pm(),
        arb_ansi_sos(),
        // SS2: ESC N <one byte>
        any::<u8>().prop_map(|b| vec![0x1B, b'N', b]),
        // SS3: ESC O <one byte>
        any::<u8>().prop_map(|b| vec![0x1B, b'O', b]),
        // Fe: ESC + single final byte (excluding multi-byte introducers)
        (0x40u8..=0x5F)
            .prop_filter("exclude multi-byte Fe introducers", |&b| {
                !matches!(b, b'[' | b']' | b'P' | b'N' | b'O' | b'_' | b'^' | b'X')
            })
            .prop_map(|b| vec![0x1B, b]),
    ]
}

// ── Property 7: Classification completeness ─────────────────────────
// **Validates: Requirement 2.8**

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]
    #[test]
    fn p7_classification_completeness(seq in arb_ansi_sequence()) {
        let mut cp = ClassifyingParser::new();
        let mut start_count = 0u32;
        let mut end_count = 0u32;
        let mut kind_at_end = SeqKind::Unknown;

        for &byte in &seq {
            match cp.feed(byte) {
                SeqAction::StartSeq => start_count += 1,
                SeqAction::EndSeq => {
                    end_count += 1;
                    kind_at_end = cp.current_kind();
                }
                SeqAction::InSeq | SeqAction::Emit => {}
            }
        }

        prop_assert_eq!(
            start_count, 1,
            "expected exactly 1 StartSeq, got {} for sequence {:?}",
            start_count, seq
        );
        prop_assert_eq!(
            end_count, 1,
            "expected exactly 1 EndSeq, got {} for sequence {:?}",
            end_count, seq
        );
        prop_assert_ne!(
            kind_at_end,
            SeqKind::Unknown,
            "expected current_kind() != Unknown at EndSeq for sequence {:?}",
            seq
        );
    }
}

// ── Generators for SGR sequences ────────────────────────────────────

/// A single SGR parameter value (0–107, covering all meaningful ranges).
#[allow(dead_code)]
fn arb_sgr_param() -> impl Strategy<Value = u16> {
    prop_oneof![
        // Basic range: 0-29
        (0u16..=29u16),
        // 38 (extended fg trigger)
        Just(38u16),
        // 39 (basic: default fg)
        Just(39u16),
        // 48 (extended bg trigger)
        Just(48u16),
        // 49 (basic: default bg)
        Just(49u16),
        // Bright fg: 90-97
        (90u16..=97u16),
        // Bright bg: 100-107
        (100u16..=107u16),
        // Unknown/other params (50-89, 108-200)
        (50u16..=89u16),
        (108u16..=200u16),
    ]
}

/// Classify a single SGR param value independently (reference implementation).
///
/// Returns the SgrContent bits that this param contributes on its own
/// (ignoring sub-parameter context like 38;5;N).
#[allow(dead_code)]
fn classify_single_param(v: u16) -> SgrContent {
    match v {
        0..=29 | 39 | 49 | 90..=97 | 100..=107 => SgrContent::BASIC,
        _ => SgrContent::empty(),
    }
}

/// Build a well-formed SGR byte sequence from a list of param values.
///
/// Handles 38;5;N and 48;5;N (extended) and 38;2;R;G;B (truecolor)
/// by encoding them as proper sub-parameter sequences.
fn build_sgr_sequence(params: &[SgrParam]) -> Vec<u8> {
    let mut seq = vec![0x1B, b'['];
    let mut first = true;
    for p in params {
        if !first {
            seq.push(b';');
        }
        first = false;
        match p {
            SgrParam::Basic(v) => {
                seq.extend_from_slice(v.to_string().as_bytes());
            }
            SgrParam::Extended(idx) => {
                seq.extend_from_slice(b"38;5;");
                seq.extend_from_slice(idx.to_string().as_bytes());
            }
            SgrParam::ExtendedBg(idx) => {
                seq.extend_from_slice(b"48;5;");
                seq.extend_from_slice(idx.to_string().as_bytes());
            }
            SgrParam::Truecolor(r, g, b) => {
                seq.extend_from_slice(b"38;2;");
                seq.extend_from_slice(r.to_string().as_bytes());
                seq.push(b';');
                seq.extend_from_slice(g.to_string().as_bytes());
                seq.push(b';');
                seq.extend_from_slice(b.to_string().as_bytes());
            }
            SgrParam::TruecolorBg(r, g, b) => {
                seq.extend_from_slice(b"48;2;");
                seq.extend_from_slice(r.to_string().as_bytes());
                seq.push(b';');
                seq.extend_from_slice(g.to_string().as_bytes());
                seq.push(b';');
                seq.extend_from_slice(b.to_string().as_bytes());
            }
            SgrParam::Other(v) => {
                seq.extend_from_slice(v.to_string().as_bytes());
            }
        }
    }
    seq.push(b'm');
    seq
}

/// A typed SGR parameter for property testing.
#[derive(Debug, Clone)]
enum SgrParam {
    /// Basic param (0-29, 39, 49, 90-97, 100-107)
    Basic(u16),
    /// 38;5;N — extended fg
    Extended(u8),
    /// 48;5;N — extended bg
    ExtendedBg(u8),
    /// 38;2;R;G;B — truecolor fg
    Truecolor(u8, u8, u8),
    /// 48;2;R;G;B — truecolor bg
    TruecolorBg(u8, u8, u8),
    /// Unknown/other param (no bits)
    Other(u16),
}

impl SgrParam {
    /// The SgrContent bits this param contributes.
    fn content(&self) -> SgrContent {
        match self {
            SgrParam::Basic(_) => SgrContent::BASIC,
            SgrParam::Extended(_) | SgrParam::ExtendedBg(_) => SgrContent::EXTENDED,
            SgrParam::Truecolor(_, _, _) | SgrParam::TruecolorBg(_, _, _) => SgrContent::TRUECOLOR,
            SgrParam::Other(_) => SgrContent::empty(),
        }
    }
}

fn arb_sgr_param_typed() -> impl Strategy<Value = SgrParam> {
    prop_oneof![
        // Basic params
        prop_oneof![
            (0u16..=29u16).prop_map(SgrParam::Basic),
            Just(SgrParam::Basic(39)),
            Just(SgrParam::Basic(49)),
            (90u16..=97u16).prop_map(SgrParam::Basic),
            (100u16..=107u16).prop_map(SgrParam::Basic),
        ],
        // Extended 256-color fg
        any::<u8>().prop_map(SgrParam::Extended),
        // Extended 256-color bg
        any::<u8>().prop_map(SgrParam::ExtendedBg),
        // Truecolor fg
        (any::<u8>(), any::<u8>(), any::<u8>())
            .prop_map(|(r, g, b)| SgrParam::Truecolor(r, g, b)),
        // Truecolor bg
        (any::<u8>(), any::<u8>(), any::<u8>())
            .prop_map(|(r, g, b)| SgrParam::TruecolorBg(r, g, b)),
        // Other/unknown params (no bits)
        (50u16..=89u16).prop_map(SgrParam::Other),
        (108u16..=200u16).prop_map(SgrParam::Other),
    ]
}

// ── P1: SGR classification is pure set membership ────────────────────
// **Validates: Requirements 1.1, 1.2 (AC-1.1, AC-1.2)**

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..Default::default() })]
    #[test]
    fn p1_sgr_classification_is_pure_set_membership(
        params in prop::collection::vec(arb_sgr_param_typed(), 1..=6)
    ) {
        // Compute expected content: union of each param's contribution.
        let expected = params.iter().fold(SgrContent::empty(), |acc, p| acc | p.content());

        // Build the SGR sequence and classify it.
        let seq = build_sgr_sequence(&params);
        let mut cp = ClassifyingParser::new();
        let mut last_action = SeqAction::Emit;
        for &byte in &seq {
            last_action = cp.feed(byte);
        }

        prop_assert_eq!(
            last_action,
            SeqAction::EndSeq,
            "expected EndSeq for sequence {:?} (params: {:?})",
            seq, params
        );
        prop_assert_eq!(
            cp.sgr_content(),
            expected,
            "sgr_content mismatch for sequence {:?} (params: {:?}): got {:?}, expected {:?}",
            seq, params, cp.sgr_content(), expected
        );
    }
}

// ── P2: OSC type determined by first param only ──────────────────────
// **Validates: Requirements 2.1**

/// Build a well-formed OSC sequence with a specific numeric first param.
fn build_osc_seq(n: u16, bel_terminated: bool) -> Vec<u8> {
    let mut seq = vec![0x1B, b']'];
    seq.extend_from_slice(n.to_string().as_bytes());
    seq.push(b';');
    if bel_terminated {
        seq.push(0x07); // BEL
    } else {
        seq.push(0x1B);
        seq.push(b'\\'); // ST
    }
    seq
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..Default::default() })]
    #[test]
    fn p2_osc_type_determined_by_first_param(
        n in any::<u16>(),
        bel_terminated in any::<bool>(),
    ) {
        let seq = build_osc_seq(n, bel_terminated);
        let expected_type = map_osc_number(n);

        let mut cp = ClassifyingParser::new();
        let mut last_action = SeqAction::Emit;
        for &byte in &seq {
            last_action = cp.feed(byte);
        }

        prop_assert_eq!(
            last_action,
            SeqAction::EndSeq,
            "expected EndSeq for OSC {} sequence {:?}",
            n, seq
        );
        prop_assert_eq!(
            cp.current_kind(),
            SeqKind::Osc,
            "expected Osc kind for sequence {:?}",
            seq
        );
        prop_assert_eq!(
            cp.osc_type(),
            expected_type,
            "osc_type mismatch for OSC {}: got {:?}, expected {:?}",
            n, cp.osc_type(), expected_type
        );
        prop_assert_eq!(
            cp.osc_number(),
            n,
            "osc_number mismatch for OSC {}: got {}, expected {}",
            n, cp.osc_number(), n
        );
    }
}
