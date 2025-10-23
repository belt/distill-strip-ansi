use std::io::Write;
use strip_ansi::StripWriter;

#[test]
fn write_clean_passthrough() {
    let mut buf = Vec::new();
    let mut w = StripWriter::new(&mut buf);
    w.write_all(b"hello world").unwrap();
    w.flush().unwrap();
    assert_eq!(buf, b"hello world");
}

#[test]
fn write_strips_csi() {
    let mut buf = Vec::new();
    let mut w = StripWriter::new(&mut buf);
    w.write_all(b"\x1b[31mred\x1b[0m").unwrap();
    assert_eq!(buf, b"red");
}

#[test]
fn write_strips_osc8_hyperlink() {
    let mut buf = Vec::new();
    let mut w = StripWriter::new(&mut buf);
    w.write_all(b"\x1b]8;;https://example.com\x07click\x1b]8;;\x07")
        .unwrap();
    assert_eq!(buf, b"click");
}

#[test]
fn write_cross_chunk_csi() {
    let mut buf = Vec::new();
    let mut w = StripWriter::new(&mut buf);
    w.write_all(b"a\x1b[3").unwrap();
    w.write_all(b"1mb").unwrap();
    assert_eq!(buf, b"ab");
}

#[test]
fn write_cross_chunk_osc() {
    let mut buf = Vec::new();
    let mut w = StripWriter::new(&mut buf);
    w.write_all(b"a\x1b]0;ti").unwrap();
    w.write_all(b"tle\x07b").unwrap();
    assert_eq!(buf, b"ab");
}

#[test]
fn write_reports_full_len() {
    let mut buf = Vec::new();
    let mut w = StripWriter::new(&mut buf);
    let n = w.write(b"\x1b[31mhello\x1b[0m").unwrap();
    assert_eq!(n, 14); // all input bytes consumed
    assert_eq!(buf, b"hello");
}

#[test]
fn write_flush_delegates() {
    let mut buf = Vec::new();
    let mut w = StripWriter::new(&mut buf);
    w.write_all(b"data").unwrap();
    w.flush().unwrap();
    assert_eq!(buf, b"data");
}

#[test]
fn write_eq_strip() {
    let input = b"a\x1b[31mb\x1b[0mc\x1b]8;;url\x07link\x1b]8;;\x07d";
    let expected = strip_ansi::strip(input);
    let mut buf = Vec::new();
    let mut w = StripWriter::new(&mut buf);
    w.write_all(input).unwrap();
    assert_eq!(buf, &*expected);
}

#[test]
fn write_eq_strip_chunked() {
    let input = b"a\x1b[31mb\x1b[0mc\x1b]8;;url\x07link\x1b]8;;\x07d";
    let expected = strip_ansi::strip(input);
    // Split at every byte boundary
    for split in 1..input.len() {
        let mut buf = Vec::new();
        let mut w = StripWriter::new(&mut buf);
        w.write_all(&input[..split]).unwrap();
        w.write_all(&input[split..]).unwrap();
        assert_eq!(buf, &*expected, "split at {split}");
    }
}

#[test]
fn into_inner_returns_writer() {
    let buf = Vec::new();
    let w = StripWriter::new(buf);
    let inner: Vec<u8> = w.into_inner();
    assert!(inner.is_empty());
}

#[test]
fn get_ref_borrows() {
    let mut buf = Vec::new();
    let mut w = StripWriter::new(&mut buf);
    w.write_all(b"test").unwrap();
    assert_eq!(w.get_ref().len(), 4);
}

#[test]
fn reset_discards_incomplete() {
    let mut buf = Vec::new();
    let mut w = StripWriter::new(&mut buf);
    w.write_all(b"a\x1b[31").unwrap(); // incomplete CSI
    w.reset();
    w.write_all(b"b").unwrap(); // should emit, not skip
    assert_eq!(buf, b"ab");
}

#[test]
fn write_empty() {
    let mut buf = Vec::new();
    let mut w = StripWriter::new(&mut buf);
    w.write_all(b"").unwrap();
    assert!(buf.is_empty());
}
