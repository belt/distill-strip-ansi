#![cfg(feature = "filter")]

use proptest::prelude::*;
use strip_ansi::{filter_strip, strip, FilterConfig, SeqGroup, SeqKind};

// ── Generators ──────────────────────────────────────────────────────

/// All SeqGroup variants for random selection.
const ALL_GROUPS: &[SeqGroup] = &[
    SeqGroup::Csi,
    SeqGroup::Osc,
    SeqGroup::Dcs,
    SeqGroup::Apc,
    SeqGroup::Pm,
    SeqGroup::Sos,
    SeqGroup::Ss2,
    SeqGroup::Ss3,
    SeqGroup::Fe,
];

/// All SeqKind variants for random sub-preserved selection.
const ALL_KINDS: &[SeqKind] = &[
    SeqKind::CsiSgr,
    SeqKind::CsiCursor,
    SeqKind::CsiErase,
    SeqKind::CsiScroll,
    SeqKind::CsiMode,
    SeqKind::CsiDeviceStatus,
    SeqKind::CsiWindow,
    SeqKind::CsiOther,
    SeqKind::Osc,
    SeqKind::Dcs,
    SeqKind::Apc,
    SeqKind::Pm,
    SeqKind::Sos,
    SeqKind::Ss2,
    SeqKind::Ss3,
    SeqKind::Fe,
];

/// Generate an arbitrary [`FilterConfig`] with random mode, preserved
/// group bits, and sub-preserved SeqKind values (0–4 items).
fn arb_filter_config() -> impl Strategy<Value = FilterConfig> {
    (
        // mode: true = StripAll, false = StripExcept
        any::<bool>(),
        // preserved group bits (0..9 groups → bitmask over u16)
        prop::collection::vec(prop::sample::select(ALL_GROUPS), 0..=9),
        // sub-preserved kinds (0..4 items)
        prop::collection::vec(prop::sample::select(ALL_KINDS), 0..=4),
    )
        .prop_map(|(strip_all, groups, kinds)| {
            if strip_all {
                FilterConfig::strip_all()
            } else {
                let mut cfg = FilterConfig::strip_all();
                for &g in &groups {
                    cfg = cfg.no_strip_group(g);
                }
                for &k in &kinds {
                    cfg = cfg.no_strip_kind(k);
                }
                cfg
            }
        })
}


// ── Property 1: Backward compatibility ──────────────────────────────
// **Validates: Requirements 4.1, 10.2**

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]
    #[test]
    fn p1_backward_compatibility(input in prop::collection::vec(any::<u8>(), 0..8192)) {
        let filter_result = filter_strip(&input, &FilterConfig::strip_all());
        let strip_result = strip(&input);
        prop_assert_eq!(
            &*filter_result, &*strip_result,
            "filter_strip(x, strip_all) must be byte-identical to strip(x)"
        );
    }
}

// ── Property 2: Pass-all identity ───────────────────────────────────
// **Validates: Requirement 4.2**

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]
    #[test]
    fn p2_pass_all_identity(input in prop::collection::vec(any::<u8>(), 0..8192)) {
        let result = filter_strip(&input, &FilterConfig::pass_all());
        prop_assert_eq!(
            &*result, &*input,
            "filter_strip(x, pass_all) must return x unchanged"
        );
    }
}

// ── Property 3: Idempotency ─────────────────────────────────────────
// **Validates: Requirement 4.7**

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]
    #[test]
    fn p3_idempotency(
        (input, config) in (prop::collection::vec(any::<u8>(), 0..8192), arb_filter_config())
    ) {
        let once = filter_strip(&input, &config);
        let twice = filter_strip(&once, &config);
        prop_assert_eq!(
            &*twice, &*once,
            "filter_strip(filter_strip(x, c), c) must equal filter_strip(x, c)"
        );
    }
}

// ── Property 4: Never grows ─────────────────────────────────────────
// **Validates: Requirement 4.6**

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]
    #[test]
    fn p4_never_grows(
        (input, config) in (prop::collection::vec(any::<u8>(), 0..8192), arb_filter_config())
    ) {
        let result = filter_strip(&input, &config);
        prop_assert!(
            result.len() <= input.len(),
            "filter_strip output length {} exceeds input length {}",
            result.len(),
            input.len()
        );
    }
}


// ── Additional imports for streaming and classification tests ────────
use strip_ansi::{ClassifyingParser, FilterStream, SeqAction};

// ── Sequence generators (adapted from property_classifier_tests.rs) ──

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

// ── Helper: determine SeqGroup from well-formed sequence bytes ──────

/// Determine the SeqGroup of a well-formed ANSI sequence by feeding
/// it through ClassifyingParser.
fn classify_seq_group(seq: &[u8]) -> SeqGroup {
    let mut cp = ClassifyingParser::new();
    let mut kind = SeqKind::Unknown;
    for &byte in seq {
        if let SeqAction::EndSeq = cp.feed(byte) {
            kind = cp.current_kind();
        }
    }
    kind.group()
}

// ── Generator: random SeqGroup ──────────────────────────────────────

fn arb_seq_group() -> impl Strategy<Value = SeqGroup> {
    prop::sample::select(ALL_GROUPS)
}

// ── Generator: split points for streaming equivalence ───────────────

/// Split input into chunks at the given sorted split points.
fn chunks_at_splits<'a>(input: &'a [u8], splits: &[usize]) -> Vec<&'a [u8]> {
    let mut chunks = Vec::new();
    let mut start = 0;
    for &split in splits {
        let split = split.min(input.len());
        if split > start {
            chunks.push(&input[start..split]);
            start = split;
        }
    }
    if start < input.len() {
        chunks.push(&input[start..]);
    }
    if chunks.is_empty() {
        chunks.push(input);
    }
    chunks
}

// ── Generator: CSI sequence with specific final byte ────────────────

/// Generate a CSI sequence with a specific final byte (for sub-kind testing).
fn arb_csi_with_final(final_byte: u8) -> impl Strategy<Value = Vec<u8>> {
    (0u8..50, prop::collection::vec(0x30u8..=0x3F, 0..3))
        .prop_map(move |(code, params)| {
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

// ── CSI sub-kind variants for sub-kind testing ──────────────────────

const CSI_SUBKINDS: &[(SeqKind, u8)] = &[
    (SeqKind::CsiSgr, b'm'),
    (SeqKind::CsiCursor, b'A'),
    (SeqKind::CsiErase, b'J'),
    (SeqKind::CsiScroll, b'S'),
    (SeqKind::CsiMode, b'h'),
    (SeqKind::CsiDeviceStatus, b'n'),
    (SeqKind::CsiWindow, b't'),
];

// ── Generator: mixed ANSI input with multiple sequence types ────────

/// Generate input with interleaved clean bytes and ANSI sequences
/// from multiple groups/sub-kinds.
fn arb_mixed_ansi_input() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(
        prop_oneof![
            // Clean bytes (no ESC)
            prop::collection::vec(0x20u8..=0x7E, 1..16)
                .prop_map(|v| v),
            // CSI SGR
            arb_csi_with_final(b'm'),
            // CSI Cursor
            arb_csi_with_final(b'A'),
            // CSI Erase
            arb_csi_with_final(b'J'),
            // CSI Scroll
            arb_csi_with_final(b'S'),
            // OSC
            arb_ansi_osc(),
        ],
        2..8,
    )
    .prop_map(|segments| segments.into_iter().flatten().collect())
}

// ── Generator: FilterConfig with at least one preserved sub-kind ────

/// Generate a (FilterConfig, SeqKind) pair where the config preserves
/// at least the returned sub-kind via `no_strip_kind()`.
fn arb_filter_config_with_preserved() -> impl Strategy<Value = (FilterConfig, SeqKind)> {
    prop::sample::select(CSI_SUBKINDS).prop_map(|pair| {
        let config = FilterConfig::strip_all().no_strip_kind(pair.0);
        (config, pair.0)
    })
}

// ── Property 6: Streaming equivalence ───────────────────────────────
// **Validates: Requirements 5.1, 5.2, 5.3, 5.4**

/// Find safe split points in input — positions where the parser is in
/// ground state (not inside an escape sequence). Splitting at these
/// points guarantees no sequence spans a chunk boundary.
fn safe_split_points(input: &[u8]) -> Vec<usize> {
    let mut cp = ClassifyingParser::new();
    let mut safe = Vec::new();
    for (i, &byte) in input.iter().enumerate() {
        // Check if parser is in ground state BEFORE feeding this byte
        if cp.is_ground() {
            safe.push(i);
        }
        cp.feed(byte);
    }
    // Position after last byte is also safe if parser is in ground state
    if cp.is_ground() {
        safe.push(input.len());
    }
    safe
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]
    #[test]
    fn p6_streaming_equivalence(
        input in prop::collection::vec(any::<u8>(), 0..4096),
        split_selector in prop::collection::vec(any::<prop::sample::Index>(), 0..8),
        config in arb_filter_config(),
    ) {
        let safe = safe_split_points(&input);

        // Select split points from safe positions
        let mut splits: Vec<usize> = split_selector.iter()
            .map(|idx| safe[idx.index(safe.len())])
            .collect();
        splits.sort_unstable();
        splits.dedup();

        // Feed chunks through FilterStream
        let mut stream = FilterStream::new();
        let mut streaming_output = Vec::new();
        let chunks = chunks_at_splits(&input, &splits);
        for chunk in &chunks {
            stream.push(chunk, &config, &mut streaming_output);
        }

        let stateless_output = filter_strip(&input, &config);
        prop_assert_eq!(
            &streaming_output, &*stateless_output,
            "streaming output must equal stateless filter_strip when split at safe points"
        );
    }
}

// ── Property 8: Group preservation correctness ──────────────────────
// **Validates: Requirement 3.7**

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]
    #[test]
    fn p8_group_preservation(
        seq in arb_ansi_sequence(),
        preserved_group in arb_seq_group(),
    ) {
        let actual_group = classify_seq_group(&seq);
        let config = FilterConfig::strip_all().no_strip_group(preserved_group);
        let output = filter_strip(&seq, &config);

        if actual_group == preserved_group {
            // Sequence's group is preserved → output should contain the sequence
            prop_assert_eq!(
                &*output, &seq[..],
                "sequence of preserved group {:?} should appear in output",
                preserved_group
            );
        } else {
            // Sequence's group is not preserved → should be stripped
            // A pure well-formed ANSI sequence has no content bytes,
            // so output should contain no ESC bytes.
            prop_assert!(
                !output.contains(&0x1B),
                "sequence of group {:?} should be stripped when only {:?} is preserved, \
                 but output contains ESC: {:?}",
                actual_group, preserved_group, output
            );
        }
    }
}

// ── Property 9: Sub-kind preservation correctness ───────────────────
// **Validates: Requirement 3.6**

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]
    #[test]
    fn p9_sub_kind_preservation(
        mixed_input in arb_mixed_ansi_input(),
        (config, preserved_kind) in arb_filter_config_with_preserved(),
    ) {
        let output = filter_strip(&mixed_input, &config);

        // Parse the input to find all sequences and their kinds
        let input_sequences = extract_sequences(&mixed_input);
        let output_sequences = extract_sequences(&output);

        // Sequences of the preserved kind should appear in output
        let preserved_in_input: Vec<&[u8]> = input_sequences.iter()
            .filter(|(_, kind)| *kind == preserved_kind)
            .map(|(bytes, _)| bytes.as_slice())
            .collect();

        let preserved_in_output: Vec<&[u8]> = output_sequences.iter()
            .filter(|(_, kind)| *kind == preserved_kind)
            .map(|(bytes, _)| bytes.as_slice())
            .collect();

        prop_assert_eq!(
            preserved_in_input.len(), preserved_in_output.len(),
            "all sequences of preserved kind {:?} should appear in output \
             (input had {}, output had {})",
            preserved_kind, preserved_in_input.len(), preserved_in_output.len()
        );

        for (inp, out) in preserved_in_input.iter().zip(preserved_in_output.iter()) {
            prop_assert_eq!(
                inp, out,
                "preserved sequence bytes should match"
            );
        }

        // Sequences from groups NOT in the config's preserved set should be stripped.
        // Since no_strip_kind also sets the group bit, sequences of the same group
        // as preserved_kind are also preserved. Sequences from OTHER groups should
        // be stripped.
        let preserved_group = preserved_kind.group();
        let other_group_in_output: Vec<&(Vec<u8>, SeqKind)> = output_sequences.iter()
            .filter(|(_, kind)| kind.group() != preserved_group)
            .collect();

        prop_assert!(
            other_group_in_output.is_empty(),
            "sequences from non-preserved groups should be stripped, \
             but found {:?} in output",
            other_group_in_output.iter().map(|(_, k)| k).collect::<Vec<_>>()
        );
    }
}

/// Extract all ANSI escape sequences from input, returning (bytes, kind) pairs.
fn extract_sequences(input: &[u8]) -> Vec<(Vec<u8>, SeqKind)> {
    let mut cp = ClassifyingParser::new();
    let mut sequences = Vec::new();
    let mut current_seq = Vec::new();
    let mut in_seq = false;

    for &byte in input {
        let action = cp.feed(byte);
        match action {
            SeqAction::StartSeq => {
                in_seq = true;
                current_seq.clear();
                current_seq.push(byte);
            }
            SeqAction::InSeq => {
                if in_seq {
                    current_seq.push(byte);
                }
            }
            SeqAction::EndSeq => {
                if in_seq {
                    current_seq.push(byte);
                    sequences.push((current_seq.clone(), cp.current_kind()));
                    in_seq = false;
                    current_seq.clear();
                }
            }
            SeqAction::Emit => {
                // Content byte — if we were in a sequence, it was aborted
                in_seq = false;
                current_seq.clear();
            }
        }
    }

    sequences
}
