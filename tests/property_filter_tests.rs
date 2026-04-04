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
    SeqKind::CsiQuery,
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

// ── try_filter_strip_str tests ──────────────────────────────────────

use std::borrow::Cow;

#[test]
fn try_filter_strip_str_clean() {
    let config = FilterConfig::strip_all();
    let result = strip_ansi::try_filter_strip_str("hello", &config);
    assert_eq!(result, Some(Cow::Borrowed("hello")));
}

#[test]
fn try_filter_strip_str_with_ansi() {
    let config = FilterConfig::strip_all();
    let result = strip_ansi::try_filter_strip_str("\x1b[31mred\x1b[0m", &config);
    assert_eq!(result.as_deref(), Some("red"));
}

#[test]
fn try_filter_strip_str_pass_all() {
    let config = FilterConfig::pass_all();
    let input = "\x1b[31mred\x1b[0m";
    let result = strip_ansi::try_filter_strip_str(input, &config);
    assert_eq!(result.as_deref(), Some(input));
}

#[test]
fn try_filter_strip_str_preserves_utf8() {
    let config = FilterConfig::strip_all();
    let result = strip_ansi::try_filter_strip_str("\x1b[1m日本語\x1b[0m", &config);
    assert_eq!(result.as_deref(), Some("日本語"));
}

#[test]
fn try_filter_strip_str_eq_filter_strip_str() {
    let config = FilterConfig::strip_all().no_strip_kind(SeqKind::CsiSgr);
    let input = "\x1b[31mred\x1b[0m \x1b[5Acursor";
    let expected = strip_ansi::filter_strip_str(input, &config);
    let result = strip_ansi::try_filter_strip_str(input, &config);
    assert_eq!(result.as_deref(), Some(expected.as_ref()));
}

#[test]
fn try_filter_strip_str_empty() {
    let config = FilterConfig::strip_all();
    let result = strip_ansi::try_filter_strip_str("", &config);
    assert_eq!(result, Some(Cow::Borrowed("")));
}

// ── Task 5 unit tests ───────────────────────────────────────────────

use strip_ansi::{OscType, SgrContent};

// ── 5.9: SGR mask filtering ─────────────────────────────────────────

/// Build a CSI SGR sequence from raw bytes.
fn sgr(params: &[u8]) -> Vec<u8> {
    let mut v = vec![0x1B, b'['];
    v.extend_from_slice(params);
    v.push(b'm');
    v
}

/// Build an OSC sequence with the given number and body, BEL-terminated.
fn osc(number: u16, body: &[u8]) -> Vec<u8> {
    let mut v = vec![0x1B, b']'];
    v.extend_from_slice(number.to_string().as_bytes());
    v.push(b';');
    v.extend_from_slice(body);
    v.push(0x07);
    v
}

#[test]
fn sgr_basic_only_mask_strips_pure_truecolor() {
    // Config: preserve only BASIC SGR content.
    // A pure truecolor sequence (38;2;255;0;0) has no BASIC bits → stripped.
    let config = FilterConfig::strip_all()
        .no_strip_kind(SeqKind::CsiSgr)
        .with_sgr_mask(SgrContent::BASIC);

    let truecolor = sgr(b"38;2;255;0;0");
    let result = filter_strip(&truecolor, &config);
    assert!(
        result.is_empty(),
        "pure truecolor SGR should be stripped by BASIC-only mask, got {:?}",
        result
    );
}

#[test]
fn sgr_basic_only_mask_preserves_mixed_basic_truecolor() {
    // A sequence with both BASIC and TRUECOLOR bits (e.g. "1;38;2;255;0;0")
    // intersects the BASIC mask → preserved.
    let config = FilterConfig::strip_all()
        .no_strip_kind(SeqKind::CsiSgr)
        .with_sgr_mask(SgrContent::BASIC);

    let mixed = sgr(b"1;38;2;255;0;0");
    let result = filter_strip(&mixed, &config);
    assert_eq!(
        &*result, &mixed[..],
        "mixed basic+truecolor SGR should be preserved by BASIC-only mask"
    );
}

#[test]
fn sgr_basic_only_mask_preserves_pure_basic() {
    // A pure basic sequence (e.g. "1") intersects the BASIC mask → preserved.
    let config = FilterConfig::strip_all()
        .no_strip_kind(SeqKind::CsiSgr)
        .with_sgr_mask(SgrContent::BASIC);

    let basic = sgr(b"1");
    let result = filter_strip(&basic, &config);
    assert_eq!(
        &*result, &basic[..],
        "pure basic SGR should be preserved by BASIC-only mask"
    );
}

#[test]
fn sgr_extended_mask_strips_basic_only() {
    // Config: preserve only EXTENDED SGR content.
    // A pure basic sequence has no EXTENDED bits → stripped.
    let config = FilterConfig::strip_all()
        .no_strip_kind(SeqKind::CsiSgr)
        .with_sgr_mask(SgrContent::EXTENDED);

    let basic = sgr(b"1");
    let result = filter_strip(&basic, &config);
    assert!(
        result.is_empty(),
        "pure basic SGR should be stripped by EXTENDED-only mask"
    );
}

// ── 5.10: OSC preserve filtering ───────────────────────────────────

#[test]
fn osc_preserve_title_preserved_clipboard_stripped() {
    // Config: preserve OSC Title only.
    let config = FilterConfig::strip_all()
        .no_strip_osc_type(OscType::Title);

    let title_seq = osc(0, b"My Window");
    let clipboard_seq = osc(52, b"c;dGVzdA==");

    // Title (OSC 0) → preserved
    let result_title = filter_strip(&title_seq, &config);
    assert_eq!(
        &*result_title, &title_seq[..],
        "OSC Title should be preserved"
    );

    // Clipboard (OSC 52) → stripped
    let result_clipboard = filter_strip(&clipboard_seq, &config);
    assert!(
        result_clipboard.is_empty(),
        "OSC Clipboard should be stripped when only Title is preserved"
    );
}

#[test]
fn osc_preserve_multiple_types() {
    // Config: preserve Title and Hyperlink.
    let config = FilterConfig::strip_all()
        .no_strip_osc_type(OscType::Title)
        .no_strip_osc_type(OscType::Hyperlink);

    let title_seq = osc(2, b"title");
    let hyperlink_seq = osc(8, b";https://example.com");
    let clipboard_seq = osc(52, b"c;dGVzdA==");

    assert_eq!(&*filter_strip(&title_seq, &config), &title_seq[..]);
    assert_eq!(&*filter_strip(&hyperlink_seq, &config), &hyperlink_seq[..]);
    assert!(filter_strip(&clipboard_seq, &config).is_empty());
}

// ── 5.11: Fast-path — empty masks degrade to should_strip ──────────

#[test]
fn empty_masks_degrade_to_should_strip_for_sgr() {
    // With empty sgr_preserve_mask, should_strip_detail == should_strip(kind).
    let config_detail = FilterConfig::strip_all().no_strip_kind(SeqKind::CsiSgr);
    let config_kind = FilterConfig::strip_all().no_strip_kind(SeqKind::CsiSgr);

    let seq = sgr(b"38;2;255;0;0");
    assert_eq!(
        &*filter_strip(&seq, &config_detail),
        &*filter_strip(&seq, &config_kind),
        "empty sgr_preserve_mask must degrade to should_strip(kind)"
    );
}

#[test]
fn empty_masks_degrade_to_should_strip_for_osc() {
    // With empty osc_preserve, should_strip_detail == should_strip(kind).
    let config_detail = FilterConfig::strip_all().no_strip_group(SeqGroup::Osc);
    let config_kind = FilterConfig::strip_all().no_strip_group(SeqGroup::Osc);

    let seq = osc(52, b"c;dGVzdA==");
    assert_eq!(
        &*filter_strip(&seq, &config_detail),
        &*filter_strip(&seq, &config_kind),
        "empty osc_preserve must degrade to should_strip(kind)"
    );
}

#[test]
fn empty_masks_strip_all_unchanged() {
    // strip_all with no masks: should_strip_detail always returns true.
    let config = FilterConfig::strip_all();
    let seq = sgr(b"1");
    assert!(
        filter_strip(&seq, &config).is_empty(),
        "strip_all with empty masks must strip everything"
    );
}

#[test]
fn seq_detail_accessor_snapshot() {
    // Verify detail() returns correct snapshot at EndSeq.
    use strip_ansi::{ClassifyingParser, SeqAction};

    let mut cp = ClassifyingParser::new();
    let seq = sgr(b"1;38;2;255;0;0");
    let mut detail = None;
    for &b in &seq {
        let action = cp.feed(b);
        if action == SeqAction::EndSeq {
            detail = Some(cp.detail());
        }
    }
    let d = detail.expect("EndSeq must be reached");
    assert_eq!(d.kind, SeqKind::CsiSgr);
    assert!(d.sgr_content.contains(SgrContent::BASIC));
    assert!(d.sgr_content.contains(SgrContent::TRUECOLOR));
    assert!(!d.dcs_is_query, "dcs_is_query must be false for CSI SGR sequences");
}

#[test]
fn debug_idempotency_regression() {
    // Regression case from p3_idempotency
    // Config: mode: StripExcept, preserved: 295, sub_preserved: [CsiMode]
    // preserved: 295 = 0b100100111 = bits 0(Csi), 1(Osc), 2(Dcs), 5(Sos), 8(Fe)
    let config = FilterConfig::strip_all()
        .no_strip_group(SeqGroup::Csi)
        .no_strip_group(SeqGroup::Osc)
        .no_strip_group(SeqGroup::Dcs)
        .no_strip_group(SeqGroup::Sos)
        .no_strip_group(SeqGroup::Fe)
        .no_strip_kind(SeqKind::CsiMode);
    
    // The regression input (first 200 bytes of the regression case)
    let input: &[u8] = &[103, 103, 114, 210, 11, 118, 139, 3, 123, 138, 198, 36, 190, 130, 52, 202, 198, 141, 135, 90, 48, 190, 9, 106, 57, 50, 182, 23, 190, 162, 118, 102, 8, 54, 242, 128, 186, 227, 255, 81, 87, 191, 211, 165, 142, 200, 85, 18, 205, 156, 49, 22, 47, 33, 184, 142, 78, 81, 157, 226, 63, 63, 121, 208, 121, 205, 173, 75, 119, 222, 100, 188, 211, 69, 102, 23, 121, 20, 187, 75, 133, 113, 185, 175, 35, 123, 43, 205, 210, 137, 38, 187, 234, 74, 179, 89, 123, 224, 193, 55, 74, 133, 21, 184, 152, 198, 33, 217, 46, 146, 219, 125, 55, 135, 31, 205, 175, 218, 223, 21, 126, 59, 92, 153, 121, 62, 241, 213, 114, 115, 43, 87, 106, 3, 41, 85, 138, 230, 167, 254, 142, 147, 40, 67, 143, 112, 226, 204, 187, 158, 237, 246, 88, 243, 150, 137, 56, 104, 217, 14, 81, 88, 220, 185, 78, 54, 153, 238, 55, 180, 165, 180, 76, 153, 33, 242, 115, 86, 24, 45, 129, 134, 55, 21, 67, 230, 21, 197, 182, 94, 211, 128, 166, 204, 19, 84, 172, 38, 61, 46];
    
    let once = filter_strip(input, &config);
    let twice = filter_strip(&once, &config);
    
    if once != twice {
        // Find first difference
        let min_len = once.len().min(twice.len());
        for i in 0..min_len {
            if once[i] != twice[i] {
                let start = if i >= 10 { i - 10 } else { 0 };
                let end = (i + 20).min(min_len);
                panic!(
                    "First diff at index {}: once={} twice={}\nonce[{}..{}]={:?}\ntwice[{}..{}]={:?}",
                    i, once[i], twice[i],
                    start, end, &once[start..end],
                    start, end, &twice[start..end]
                );
            }
        }
        if once.len() != twice.len() {
            panic!("Same up to {} but lengths differ: {} vs {}", min_len, once.len(), twice.len());
        }
    }
}

#[test]
fn debug_streaming_regression() {
    use strip_ansi::{filter_strip, FilterConfig, FilterStream};
    
    // Regression case from p6_streaming_equivalence
    // Config: mode: StripAll, preserved: 0, sub_preserved: []
    let config = FilterConfig::strip_all();
    
    // Use a simple test case that might trigger the issue
    // ESC [ ... ESC 0x82 (ESC inside CSI sequence)
    let input: &[u8] = &[0x1B, b'[', b'1', b'm', 0x1B, b'[', b'2', b'm'];
    
    let stateless = filter_strip(input, &config);
    
    let mut stream = FilterStream::new();
    let mut streaming = Vec::new();
    stream.push(input, &config, &mut streaming);
    
    assert_eq!(&streaming, &*stateless, "streaming must equal stateless for strip_all");
}

#[test]
fn debug_streaming_esc_inside_seq() {
    use strip_ansi::{filter_strip, FilterConfig, FilterStream};
    
    let config = FilterConfig::strip_all();
    
    // ESC 0x82 ESC 0xE6 — two ESC bytes where the first starts a sequence
    // that ends when 0x82 is fed (since 0x82 > 0x7E, EscapeStart → Ground)
    let input: &[u8] = &[0x1B, 0x82, 0x1B, 0xE6];
    
    let stateless = filter_strip(input, &config);
    
    let mut stream = FilterStream::new();
    let mut streaming = Vec::new();
    stream.push(input, &config, &mut streaming);
    
    println!("stateless: {:?}", stateless);
    println!("streaming: {:?}", streaming);
    
    assert_eq!(&streaming, &*stateless, "streaming must equal stateless for strip_all");
}

#[test]
fn debug_p6_regression_exact() {
    use strip_ansi::{filter_strip, FilterConfig, FilterStream};
    
    let config = FilterConfig::strip_all();
    
    // Exact regression case input (first 300 bytes)
    let input: &[u8] = &[240, 94, 114, 144, 251, 36, 145, 37, 75, 39, 225, 191, 21, 128, 185, 202, 223, 130, 150, 153, 147, 114, 2, 174, 17, 232, 242, 103, 79, 100, 124, 3, 110, 129, 169, 97, 186, 137, 149, 201, 104, 118, 29, 64, 169, 92, 151, 231, 242, 252, 158, 130, 181, 122, 102, 127, 235, 138, 89, 71, 178, 170, 114, 5, 68, 46, 52, 65, 223, 105, 103, 112, 250, 154, 2, 146, 9, 212, 32, 175, 12, 245, 250, 199, 199, 10, 42, 242, 254, 6, 124, 167, 232, 102, 44, 182, 180, 68, 165, 174, 218, 221, 243, 20, 61, 193, 255, 99, 15, 26, 157, 95, 183, 186, 16, 107, 95, 95, 198, 90, 87, 89, 34, 6, 212, 117, 230, 173, 206, 122, 54, 46, 89, 134, 42, 95, 253, 25, 107, 16, 146, 121, 23, 198, 80, 82, 148, 244, 213, 123, 4, 23, 184, 47, 157, 225, 18, 238, 205, 46, 155, 231, 189, 6, 251, 78, 79, 159, 231, 227, 126, 66, 231, 51, 110, 79, 75, 130, 88, 22, 91, 72, 103, 185, 55, 169, 170, 199, 91, 35, 49, 192, 32, 80, 213, 172, 251, 93, 182, 179, 94, 46, 229, 217, 99, 131, 128, 34, 246, 123, 211, 50, 230, 105, 241, 141, 191, 128, 120, 91, 43, 201, 182, 255, 56, 6, 200, 40, 85, 73, 128, 80, 76, 136, 133, 142, 71, 238, 235, 239, 28, 131, 83, 2, 117, 231, 11, 125, 122, 97, 29, 103, 23, 21, 71, 73, 55, 67, 175, 25, 214, 73, 19, 18, 8, 52, 15, 159, 80, 171, 217, 129, 0, 117, 52, 162, 33, 28, 176, 57, 120, 197, 34, 64, 212, 77, 196, 89, 0, 186, 147, 227, 253, 62, 190, 87, 155, 121, 198, 249];
    
    let stateless = filter_strip(input, &config);
    
    let mut stream = FilterStream::new();
    let mut streaming = Vec::new();
    stream.push(input, &config, &mut streaming);
    
    if streaming != *stateless {
        // Find first difference
        let min_len = streaming.len().min(stateless.len());
        for i in 0..min_len {
            if streaming[i] != stateless[i] {
                let start = if i >= 5 { i - 5 } else { 0 };
                let end = (i + 10).min(min_len);
                panic!(
                    "First diff at index {}: streaming={} stateless={}\nstreaming[{}..{}]={:?}\nstateless[{}..{}]={:?}",
                    i, streaming[i], stateless[i],
                    start, end, &streaming[start..end],
                    start, end, &stateless[start..end]
                );
            }
        }
        if streaming.len() != stateless.len() {
            panic!("Same up to {} but lengths differ: {} vs {}", min_len, streaming.len(), stateless.len());
        }
    }
}
