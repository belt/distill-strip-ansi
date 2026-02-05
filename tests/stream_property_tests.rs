use proptest::prelude::*;
use strip_ansi::StripStream;

/// Strategy: arbitrary bytes + a split point for chunking.
fn arb_chunked_input() -> impl Strategy<Value = (Vec<u8>, usize)> {
    prop::collection::vec(any::<u8>(), 0..4096).prop_flat_map(|v| {
        let len = v.len();
        let split = if len == 0 { Just(0).boxed() } else { (0..=len).boxed() };
        (Just(v), split)
    })
}

/// Strategy: arbitrary bytes + multiple split points.
fn arb_multi_chunked_input() -> impl Strategy<Value = (Vec<u8>, Vec<usize>)> {
    prop::collection::vec(any::<u8>(), 0..4096).prop_flat_map(|v| {
        let len = v.len();
        let splits = prop::collection::vec(0..=len.max(1), 0..8);
        (Just(v), splits)
    })
}

// P5: Streaming eq — chunks == strip(concat)
proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]
    #[test]
    fn p5_streaming_eq_stateless((input, split) in arb_chunked_input()) {
        let stateless = strip_ansi::strip(&input);

        let mut stream = StripStream::new();
        let mut streaming_out = Vec::new();

        let (chunk1, chunk2) = input.split_at(split);
        for slice in stream.strip_slices(chunk1) {
            streaming_out.extend_from_slice(slice);
        }
        for slice in stream.strip_slices(chunk2) {
            streaming_out.extend_from_slice(slice);
        }
        stream.finish();

        prop_assert_eq!(&streaming_out, &*stateless,
            "streaming output should equal stateless strip");
    }
}

// P5 variant: multiple splits
proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]
    #[test]
    fn p5_streaming_eq_multi_chunk((input, mut splits) in arb_multi_chunked_input()) {
        let stateless = strip_ansi::strip(&input);

        splits.sort();
        splits.dedup();
        // Clamp splits to input length
        let splits: Vec<usize> = splits.into_iter().filter(|&s| s <= input.len()).collect();

        let mut stream = StripStream::new();
        let mut streaming_out = Vec::new();

        let mut prev = 0;
        for &split in &splits {
            let chunk = &input[prev..split];
            for slice in stream.strip_slices(chunk) {
                streaming_out.extend_from_slice(slice);
            }
            prev = split;
        }
        // Final chunk
        let chunk = &input[prev..];
        for slice in stream.strip_slices(chunk) {
            streaming_out.extend_from_slice(slice);
        }
        stream.finish();

        prop_assert_eq!(&streaming_out, &*stateless);
    }
}

// P8: Slice eq — strip_slices concat == strip()
proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]
    #[test]
    fn p8_slice_eq((input, split) in arb_chunked_input()) {
        let stateless = strip_ansi::strip(&input);

        let mut stream = StripStream::new();
        let mut out = Vec::new();

        let (chunk1, chunk2) = input.split_at(split);
        stream.push(chunk1, &mut out);
        stream.push(chunk2, &mut out);
        stream.finish();

        prop_assert_eq!(&out, &*stateless);
    }
}
