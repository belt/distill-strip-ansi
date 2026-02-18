#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Streaming must not panic and must equal stateless.
    let stateless = strip_ansi::strip(data);

    let mut stream = strip_ansi::StripStream::new();
    let mut out = Vec::new();
    stream.push(data, &mut out);
    stream.finish();

    assert_eq!(out, &*stateless);

    // Also test with a split in the middle.
    if data.len() >= 2 {
        let mid = data.len() / 2;
        let mut stream2 = strip_ansi::StripStream::new();
        let mut out2 = Vec::new();
        stream2.push(&data[..mid], &mut out2);
        stream2.push(&data[mid..], &mut out2);
        stream2.finish();
        assert_eq!(out2, &*stateless);
    }
});
