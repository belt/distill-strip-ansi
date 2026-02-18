#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // strip_in_place must equal strip.
    let expected = strip_ansi::strip(data).to_vec();

    let mut buf = data.to_vec();
    let len = strip_ansi::strip_in_place(&mut buf);

    assert_eq!(buf.len(), len);
    assert_eq!(buf, expected);

    // strip_into must also equal strip.
    let mut into_out = Vec::new();
    strip_ansi::strip_into(data, &mut into_out);
    assert_eq!(into_out, expected);
});
