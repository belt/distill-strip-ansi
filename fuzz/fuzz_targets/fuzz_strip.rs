#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Stateless strip must not panic.
    let stripped = strip_ansi::strip(data);

    // Idempotency: strip(strip(x)) == strip(x).
    let double = strip_ansi::strip(&stripped);
    assert_eq!(&*stripped, &*double);

    // Never grows.
    assert!(stripped.len() <= data.len());

    // contains_ansi must not panic.
    let _ = strip_ansi::contains_ansi(data);
});
