use strip_ansi::StripStream;

fn collect_slices(stream: &mut StripStream, input: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for slice in stream.strip_slices(input) {
        out.extend_from_slice(slice);
    }
    out
}

fn stream_chunks(chunks: &[&[u8]]) -> Vec<u8> {
    let mut stream = StripStream::new();
    let mut out = Vec::new();
    for chunk in chunks {
        out.extend(collect_slices(&mut stream, chunk));
    }
    stream.finish();
    out
}

// --- Clean input ---

#[test]
fn clean_chunk_single_slice() {
    let mut stream = StripStream::new();
    let slices: Vec<&[u8]> = stream.strip_slices(b"hello world").collect();
    assert_eq!(slices.len(), 1);
    assert_eq!(slices[0], b"hello world");
}

#[test]
fn clean_multiple_chunks() {
    let result = stream_chunks(&[b"hello ", b"world"]);
    assert_eq!(result, b"hello world");
}

// --- Cross-chunk CSI ---

#[test]
fn csi_split_after_esc() {
    // ESC | [ 3 1 m
    let result = stream_chunks(&[b"a\x1b", b"[31mb"]);
    assert_eq!(result, b"ab");
}

#[test]
fn csi_split_after_bracket() {
    let result = stream_chunks(&[b"a\x1b[", b"31mb"]);
    assert_eq!(result, b"ab");
}

#[test]
fn csi_split_mid_params() {
    let result = stream_chunks(&[b"a\x1b[3", b"1mb"]);
    assert_eq!(result, b"ab");
}

#[test]
fn csi_split_before_final() {
    let result = stream_chunks(&[b"a\x1b[31", b"mb"]);
    assert_eq!(result, b"ab");
}

// --- Cross-chunk OSC ---

#[test]
fn osc_split_in_body() {
    let result = stream_chunks(&[b"a\x1b]0;ti", b"tle\x07b"]);
    assert_eq!(result, b"ab");
}

#[test]
fn osc_split_before_bel() {
    let result = stream_chunks(&[b"a\x1b]0;title", b"\x07b"]);
    assert_eq!(result, b"ab");
}

#[test]
fn osc_split_st_esc_backslash() {
    // OSC terminated by ESC \ split across chunks
    let result = stream_chunks(&[b"a\x1b]0;title\x1b", b"\\b"]);
    assert_eq!(result, b"ab");
}

// --- Cross-chunk DCS ---

#[test]
fn dcs_split_in_passthrough() {
    let result = stream_chunks(&[b"a\x1bPqpix", b"els\x1b\\b"]);
    assert_eq!(result, b"ab");
}

// --- Cross-chunk StringPassthrough (APC) ---

#[test]
fn apc_split_in_body() {
    let result = stream_chunks(&[b"a\x1b_da", b"ta\x1b\\b"]);
    assert_eq!(result, b"ab");
}

// --- Cross-chunk SS2/SS3 ---

#[test]
fn ss2_split_after_n() {
    // ESC N | <byte>
    let result = stream_chunks(&[b"a\x1bN", b"Xb"]);
    assert_eq!(result, b"ab");
}

#[test]
fn ss3_split_after_o() {
    let result = stream_chunks(&[b"a\x1bO", b"Xb"]);
    assert_eq!(result, b"ab");
}

// --- Finish discards incomplete ---

#[test]
fn finish_discards_incomplete_csi() {
    let mut stream = StripStream::new();
    let out = collect_slices(&mut stream, b"text\x1b[31");
    assert_eq!(out, b"text");
    assert!(!stream.is_ground());
    stream.finish();
    assert!(stream.is_ground());
}

#[test]
fn finish_discards_incomplete_osc() {
    let mut stream = StripStream::new();
    let out = collect_slices(&mut stream, b"text\x1b]0;title");
    assert_eq!(out, b"text");
    stream.finish();
    assert!(stream.is_ground());
}

#[test]
fn finish_discards_lone_esc() {
    let mut stream = StripStream::new();
    let out = collect_slices(&mut stream, b"text\x1b");
    assert_eq!(out, b"text");
    stream.finish();
    assert!(stream.is_ground());
}

// --- push() equivalence ---

#[test]
fn push_eq_strip_slices() {
    let input = b"a\x1b[31mb\x1b[0mc";
    let mut stream1 = StripStream::new();
    let mut stream2 = StripStream::new();

    let slices_out = collect_slices(&mut stream1, input);

    let mut push_out = Vec::new();
    stream2.push(input, &mut push_out);

    assert_eq!(slices_out, push_out);
}

#[test]
fn push_across_chunks() {
    let mut stream = StripStream::new();
    let mut out = Vec::new();
    stream.push(b"a\x1b[31", &mut out);
    stream.push(b"mb\x1b[0mc", &mut out);
    assert_eq!(out, b"abc");
}

// --- push_write() equivalence ---

#[test]
fn push_write_eq_push() {
    let input = b"a\x1b[31mb\x1b[0mc";
    let mut stream1 = StripStream::new();
    let mut stream2 = StripStream::new();

    let mut push_out = Vec::new();
    stream1.push(input, &mut push_out);

    let mut write_out = Vec::new();
    stream2.push_write(input, &mut write_out).unwrap();

    assert_eq!(push_out, write_out);
}

// --- Streaming eq stateless ---

#[test]
fn streaming_eq_stateless_simple() {
    let input = b"a\x1b[31mb\x1b[0mc";
    let stateless = strip_ansi::strip(input);
    let streaming = stream_chunks(&[input]);
    assert_eq!(streaming, &*stateless);
}

#[test]
fn streaming_eq_stateless_chunked() {
    let input = b"a\x1b[31mb\x1b[0mc\x1b]8;;url\x07link\x1b]8;;\x07d";
    let stateless = strip_ansi::strip(input);

    // Split at every byte boundary
    for split in 1..input.len() {
        let result = stream_chunks(&[&input[..split], &input[split..]]);
        assert_eq!(result, &*stateless, "split at {split}");
    }
}

// --- Edge cases ---

#[test]
fn empty_chunk() {
    let mut stream = StripStream::new();
    let slices: Vec<&[u8]> = stream.strip_slices(b"").collect();
    assert!(slices.is_empty());
}

#[test]
fn all_escape_chunk() {
    let mut stream = StripStream::new();
    let out = collect_slices(&mut stream, b"\x1b[31m\x1b[0m");
    assert!(out.is_empty());
}

#[test]
fn stream_reset() {
    let mut stream = StripStream::new();
    collect_slices(&mut stream, b"\x1b[31");
    assert!(!stream.is_ground());
    stream.reset();
    assert!(stream.is_ground());
}

#[test]
fn stream_default() {
    let stream = StripStream::default();
    assert!(stream.is_ground());
}

#[test]
fn stream_is_copy() {
    let s = StripStream::new();
    let s2 = s;
    assert_eq!(s, s2);
}

#[test]
fn stream_size() {
    assert_eq!(std::mem::size_of::<StripStream>(), 1);
}

#[test]
fn multiple_escapes_across_many_chunks() {
    let result = stream_chunks(&[
        b"\x1b[1m",
        b"bold",
        b"\x1b[0m",
        b" normal ",
        b"\x1b[31m",
        b"red",
        b"\x1b[0m",
    ]);
    assert_eq!(result, b"bold normal red");
}

// ── Cross-chunk echoback regression tests ───────────────────────────
// Verify that echoback attack sequences split across chunk boundaries
// are always stripped (never partially emitted). See doc/SECURITY.md.

#[test]
fn cross_chunk_dcs_decrqss_stripped() {
    // DECRQSS: ESC P $ q " p ESC \ (CVE-2008-2383 vector)
    // Split across two chunks at the passthrough boundary.
    let result = stream_chunks(&[
        b"\x1bP$q",   // DCS entry + params
        b"\"p\x1b\\", // passthrough + ST
    ]);
    assert_eq!(result, b"");
}

#[test]
fn cross_chunk_osc50_query_stripped() {
    // OSC 50 font query: ESC ] 50 ; ? BEL (CVE-2022-45063 vector)
    // Split between the OSC number and the query character.
    let result = stream_chunks(&[
        b"before\x1b]50;", // content + OSC start
        b"?\x07after",     // query + BEL + content
    ]);
    assert_eq!(result, b"beforeafter");
}

#[test]
fn cross_chunk_csi_title_report_stripped() {
    // CSI 21 t — title report request (HD Moore 2003 vector)
    // Split between param bytes and final byte.
    let result = stream_chunks(&[
        b"safe\x1b[21", // content + CSI + params
        b"tunsafe",     // final byte 't' + content
    ]);
    assert_eq!(result, b"safeunsafe");
}

#[test]
fn cross_chunk_osc52_clipboard_stripped() {
    // OSC 52 clipboard: ESC ] 52 ; c ; <base64> BEL
    // Split in the middle of the payload.
    let result = stream_chunks(&[
        b"\x1b]52;c;SGVs", // OSC 52 start + partial base64
        b"bG8=\x07done",   // rest of base64 + BEL + content
    ]);
    assert_eq!(result, b"done");
}
