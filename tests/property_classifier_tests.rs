#![cfg(feature = "filter")]

use proptest::prelude::*;
use strip_ansi::{ClassifyingParser, SeqAction, SeqKind};

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
