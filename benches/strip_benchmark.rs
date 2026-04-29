use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

// --- Test data ---

fn clean_input(size: usize) -> Vec<u8> {
    (0..size).map(|i| b'A' + (i % 26) as u8).collect()
}

fn dirty_input(size: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(size);
    let mut i = 0;
    while v.len() < size {
        // ~20% escape sequences
        if i % 5 == 0 && v.len() + 5 <= size {
            v.extend_from_slice(b"\x1b[31m");
        } else {
            v.push(b'A' + (i % 26) as u8);
        }
        i += 1;
    }
    v.truncate(size);
    v
}

fn real_world_cargo() -> Vec<u8> {
    let mut v = Vec::new();
    for _ in 0..100 {
        v.extend_from_slice(
            b"\x1b[0m\x1b[1m\x1b[32m   Compiling\x1b[0m memchr v2.7.1\n",
        );
    }
    v
}

fn real_world_osc8() -> Vec<u8> {
    let mut v = Vec::new();
    for _ in 0..50 {
        v.extend_from_slice(
            b"\x1b]8;;https://docs.rs/memchr/2.7.1\x07memchr\x1b]8;;\x07 v2.7.1\n",
        );
    }
    v
}

// --- Benchmarks ---

fn bench_strip(c: &mut Criterion) {
    let mut group = c.benchmark_group("strip");

    for size in [256, 1024, 4096, 16384] {
        let clean = clean_input(size);
        let dirty = dirty_input(size);

        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("clean", size), &clean, |b, input| {
            b.iter(|| strip_ansi::strip(black_box(input)));
        });

        group.bench_with_input(BenchmarkId::new("dirty", size), &dirty, |b, input| {
            b.iter(|| strip_ansi::strip(black_box(input)));
        });
    }

    let cargo = real_world_cargo();
    group.throughput(Throughput::Bytes(cargo.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("real_cargo", cargo.len()),
        &cargo,
        |b, input| {
            b.iter(|| strip_ansi::strip(black_box(input)));
        },
    );

    let osc8 = real_world_osc8();
    group.throughput(Throughput::Bytes(osc8.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("real_osc8", osc8.len()),
        &osc8,
        |b, input| {
            b.iter(|| strip_ansi::strip(black_box(input)));
        },
    );

    group.finish();
}

fn bench_strip_in_place(c: &mut Criterion) {
    let mut group = c.benchmark_group("strip_in_place");

    for size in [1024, 4096, 16384] {
        let dirty = dirty_input(size);
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("dirty", size), &dirty, |b, input| {
            b.iter_batched(
                || input.clone(),
                |mut buf| strip_ansi::strip_in_place(black_box(&mut buf)),
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_contains_ansi(c: &mut Criterion) {
    let mut group = c.benchmark_group("contains_ansi");

    for size in [1024, 4096, 16384] {
        let clean = clean_input(size);
        let dirty = dirty_input(size);
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("clean", size), &clean, |b, input| {
            b.iter(|| strip_ansi::contains_ansi(black_box(input)));
        });

        group.bench_with_input(BenchmarkId::new("dirty", size), &dirty, |b, input| {
            b.iter(|| strip_ansi::contains_ansi(black_box(input)));
        });
    }

    group.finish();
}

fn bench_stream(c: &mut Criterion) {
    let mut group = c.benchmark_group("stream");

    for size in [1024, 4096, 16384] {
        let dirty = dirty_input(size);
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(
            BenchmarkId::new("strip_slices", size),
            &dirty,
            |b, input| {
                b.iter(|| {
                    let mut stream = strip_ansi::StripStream::new();
                    let mut out = Vec::with_capacity(input.len());
                    stream.push(black_box(input), &mut out);
                    stream.finish();
                    out
                });
            },
        );
    }

    let cargo = real_world_cargo();
    group.throughput(Throughput::Bytes(cargo.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("real_cargo", cargo.len()),
        &cargo,
        |b, input| {
            b.iter(|| {
                let mut stream = strip_ansi::StripStream::new();
                let mut out = Vec::with_capacity(input.len());
                stream.push(black_box(input), &mut out);
                stream.finish();
                out
            });
        },
    );

    group.finish();
}

fn bench_ecosystem_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("ecosystem");

    let dirty = dirty_input(4096);
    let cargo = real_world_cargo();

    group.throughput(Throughput::Bytes(4096));

    group.bench_with_input(
        BenchmarkId::new("ours_strip", 4096),
        &dirty,
        |b, input| {
            b.iter(|| strip_ansi::strip(black_box(input)));
        },
    );

    group.bench_with_input(
        BenchmarkId::new("strip_ansi_escapes", 4096),
        &dirty,
        |b, input| {
            b.iter(|| strip_ansi_escapes::strip(black_box(input)));
        },
    );

    group.bench_with_input(
        BenchmarkId::new("console_strip", 4096),
        &dirty,
        |b, input| {
            let s = String::from_utf8_lossy(input);
            b.iter(|| console::strip_ansi_codes(black_box(&s)));
        },
    );

    group.throughput(Throughput::Bytes(cargo.len() as u64));

    group.bench_with_input(
        BenchmarkId::new("ours_cargo", cargo.len()),
        &cargo,
        |b, input| {
            b.iter(|| strip_ansi::strip(black_box(input)));
        },
    );

    group.bench_with_input(
        BenchmarkId::new("strip_ansi_escapes_cargo", cargo.len()),
        &cargo,
        |b, input| {
            b.iter(|| strip_ansi_escapes::strip(black_box(input)));
        },
    );

    group.finish();
}

criterion_group!(
    benches,
    bench_strip,
    bench_strip_in_place,
    bench_contains_ansi,
    bench_stream,
    bench_ecosystem_comparison,
    bench_classifier,
    bench_filter_detail,
    bench_check_threats,
    bench_unicode_normalize,
);
criterion_main!(benches);

// --- Task 12: Security-aware filtering benchmarks ---

fn bench_classifier(c: &mut Criterion) {
    use strip_ansi::{ClassifyingParser, SeqAction};

    let mut group = c.benchmark_group("classifier");

    let cargo = real_world_cargo();
    let osc8 = real_world_osc8();

    // 12.1: ClassifyingParser overhead on real-world input.
    group.throughput(Throughput::Bytes(cargo.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("cargo_classify", cargo.len()),
        &cargo,
        |b, input| {
            b.iter(|| {
                let mut cp = ClassifyingParser::new();
                for &byte in black_box(input) {
                    let _ = cp.feed(byte);
                }
            });
        },
    );

    group.throughput(Throughput::Bytes(osc8.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("osc8_classify", osc8.len()),
        &osc8,
        |b, input| {
            b.iter(|| {
                let mut cp = ClassifyingParser::new();
                for &byte in black_box(input) {
                    let _ = cp.feed(byte);
                }
            });
        },
    );

    // Classify + detail() snapshot at EndSeq.
    group.throughput(Throughput::Bytes(cargo.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("cargo_classify_detail", cargo.len()),
        &cargo,
        |b, input| {
            b.iter(|| {
                let mut cp = ClassifyingParser::new();
                let mut count = 0u32;
                for &byte in black_box(input) {
                    if cp.feed(byte) == SeqAction::EndSeq {
                        let _ = black_box(cp.detail());
                        count += 1;
                    }
                }
                count
            });
        },
    );

    group.finish();
}

fn bench_filter_detail(c: &mut Criterion) {
    use strip_ansi::{filter_strip, FilterConfig, SeqKind, OscType, SgrContent};

    let mut group = c.benchmark_group("filter_detail");

    let cargo = real_world_cargo();

    // 12.2: Extended strip decision vs existing should_strip.
    // Baseline: should_strip(kind) only (no SGR/OSC masks).
    group.throughput(Throughput::Bytes(cargo.len() as u64));

    let config_kind_only = FilterConfig::strip_all()
        .no_strip_kind(SeqKind::CsiSgr);

    group.bench_with_input(
        BenchmarkId::new("kind_only", cargo.len()),
        &cargo,
        |b, input| {
            b.iter(|| filter_strip(black_box(input), &config_kind_only));
        },
    );

    // With SGR mask (triggers should_strip_detail path).
    let config_sgr_mask = FilterConfig::strip_all()
        .no_strip_kind(SeqKind::CsiSgr)
        .with_sgr_mask(SgrContent::BASIC);

    group.bench_with_input(
        BenchmarkId::new("sgr_mask", cargo.len()),
        &cargo,
        |b, input| {
            b.iter(|| filter_strip(black_box(input), &config_sgr_mask));
        },
    );

    // With OSC preserve (triggers should_strip_detail path).
    let config_osc_preserve = FilterConfig::strip_all()
        .no_strip_kind(SeqKind::CsiSgr)
        .no_strip_osc_type(OscType::Title)
        .no_strip_osc_type(OscType::Hyperlink);

    let osc8 = real_world_osc8();
    group.throughput(Throughput::Bytes(osc8.len() as u64));

    group.bench_with_input(
        BenchmarkId::new("osc_preserve", osc8.len()),
        &osc8,
        |b, input| {
            b.iter(|| filter_strip(black_box(input), &config_osc_preserve));
        },
    );

    // Sanitize preset (full detail path).
    let config_sanitize = strip_ansi::TerminalPreset::Sanitize.to_filter_config();
    group.throughput(Throughput::Bytes(cargo.len() as u64));

    group.bench_with_input(
        BenchmarkId::new("sanitize_preset", cargo.len()),
        &cargo,
        |b, input| {
            b.iter(|| filter_strip(black_box(input), &config_sanitize));
        },
    );

    group.finish();
}

fn bench_check_threats(c: &mut Criterion) {
    use strip_ansi::{ClassifyingParser, SeqAction, SeqKind};

    let mut group = c.benchmark_group("check_threats");

    // 12.3: Threat scanning throughput.
    // Build input with embedded threats.
    let mut threat_input = Vec::new();
    for _ in 0..100 {
        threat_input.extend_from_slice(
            b"\x1b[0m\x1b[1m\x1b[32m   Compiling\x1b[0m memchr v2.7.1\n",
        );
    }
    // Inject a few threats.
    threat_input.extend_from_slice(b"\x1b[21t"); // CSI 21t
    threat_input.extend_from_slice(b"\x1b]50;?\x07"); // OSC 50
    threat_input.extend_from_slice(b"\x1bP$qm\x1b\\"); // DECRQSS

    group.throughput(Throughput::Bytes(threat_input.len() as u64));

    // Scan-only (no filtering, just classify + match).
    group.bench_with_input(
        BenchmarkId::new("scan_only", threat_input.len()),
        &threat_input,
        |b, input| {
            b.iter(|| {
                let mut cp = ClassifyingParser::new();
                let mut threats = 0u32;
                for &byte in black_box(input) {
                    if cp.feed(byte) == SeqAction::EndSeq {
                        let d = cp.detail();
                        if matches!(
                            d.kind,
                            SeqKind::Dcs | SeqKind::CsiQuery
                        ) || (d.kind == SeqKind::Osc && d.osc_number == 50)
                        {
                            threats += 1;
                        }
                    }
                }
                threats
            });
        },
    );

    // Clean input (no threats) — measures overhead of scanning.
    let clean_cargo = real_world_cargo();
    group.throughput(Throughput::Bytes(clean_cargo.len() as u64));

    group.bench_with_input(
        BenchmarkId::new("scan_clean", clean_cargo.len()),
        &clean_cargo,
        |b, input| {
            b.iter(|| {
                let mut cp = ClassifyingParser::new();
                let mut threats = 0u32;
                for &byte in black_box(input) {
                    if cp.feed(byte) == SeqAction::EndSeq {
                        let d = cp.detail();
                        if matches!(
                            d.kind,
                            SeqKind::Dcs | SeqKind::CsiQuery
                        ) || (d.kind == SeqKind::Osc && d.osc_number == 50)
                        {
                            threats += 1;
                        }
                    }
                }
                threats
            });
        },
    );

    group.finish();
}

// --- Unicode normalization benchmarks ---

/// Pure ASCII input — fast path should skip entirely.
fn unicode_clean_ascii(size: usize) -> Vec<u8> {
    (0..size).map(|i| b'A' + (i % 26) as u8).collect()
}

/// Input with ~20% fullwidth ASCII characters.
fn unicode_fullwidth_mixed(size: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(size);
    let mut i = 0u32;
    while v.len() < size {
        if i % 5 == 0 {
            // Fullwidth A (U+FF21) = 0xEF 0xBC 0xA1 in UTF-8
            let cp = 0xFF21 + (i % 26);
            let c = char::from_u32(cp).unwrap_or('Ａ');
            let mut buf = [0u8; 3];
            let s = c.encode_utf8(&mut buf);
            if v.len() + s.len() <= size {
                v.extend_from_slice(s.as_bytes());
            }
        } else {
            v.push(b'A' + (i % 26) as u8);
        }
        i += 1;
    }
    v.truncate(size);
    v
}

/// Input with ~20% math bold characters (4-byte UTF-8).
fn unicode_math_bold_mixed(size: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(size);
    let mut i = 0u32;
    while v.len() < size {
        if i % 5 == 0 {
            // Math bold A (U+1D400) = 0xF0 0x9D 0x90 0x80 in UTF-8
            let cp = 0x1D400 + (i % 26);
            let c = char::from_u32(cp).unwrap_or('\u{1D400}');
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            if v.len() + s.len() <= size {
                v.extend_from_slice(s.as_bytes());
            }
        } else {
            v.push(b'A' + (i % 26) as u8);
        }
        i += 1;
    }
    v.truncate(size);
    v
}

/// Simulated real-world: cargo output with fullwidth homographs injected.
fn unicode_real_world() -> Vec<u8> {
    let mut v = Vec::new();
    for _ in 0..50 {
        // Normal cargo line
        v.extend_from_slice(b"   Compiling memchr v2.7.1\n");
        // Line with fullwidth homographs
        v.extend_from_slice("   Ｃompiling ｆake-crate v０.１.０\n".as_bytes());
    }
    v
}

fn bench_unicode_normalize(c: &mut Criterion) {
    use strip_ansi::unicode_map::UnicodeMap;

    let mut group = c.benchmark_group("unicode_normalize");
    let map = UnicodeMap::builtin();

    // Fast path: pure ASCII (should be near-zero cost).
    for size in [1024, 4096, 16384] {
        let clean = unicode_clean_ascii(size);
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(
            BenchmarkId::new("clean_ascii", size),
            &clean,
            |b, input| {
                b.iter(|| {
                    let s = std::str::from_utf8(black_box(input)).unwrap();
                    let mut out = Vec::with_capacity(input.len());
                    let mut char_buf = Vec::new();
                    for ch in s.chars() {
                        char_buf.clear();
                        if map.lookup_into(ch, &mut char_buf) {
                            for &tc in &char_buf {
                                let mut enc = [0u8; 4];
                                out.extend_from_slice(tc.encode_utf8(&mut enc).as_bytes());
                            }
                        } else {
                            let mut enc = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut enc).as_bytes());
                        }
                    }
                    out
                });
            },
        );
    }

    // Fullwidth mixed (~20% fullwidth, 3-byte UTF-8).
    for size in [1024, 4096, 16384] {
        let mixed = unicode_fullwidth_mixed(size);
        group.throughput(Throughput::Bytes(mixed.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("fullwidth_mixed", size),
            &mixed,
            |b, input| {
                b.iter(|| {
                    let s = std::str::from_utf8(black_box(input)).unwrap();
                    let mut out = Vec::with_capacity(input.len());
                    let mut char_buf = Vec::new();
                    for ch in s.chars() {
                        char_buf.clear();
                        if map.lookup_into(ch, &mut char_buf) {
                            for &tc in &char_buf {
                                let mut enc = [0u8; 4];
                                out.extend_from_slice(tc.encode_utf8(&mut enc).as_bytes());
                            }
                        } else {
                            let mut enc = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut enc).as_bytes());
                        }
                    }
                    out
                });
            },
        );
    }

    // Math bold mixed (~20% math bold, 4-byte UTF-8).
    let math = unicode_math_bold_mixed(4096);
    group.throughput(Throughput::Bytes(math.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("math_bold_mixed", math.len()),
        &math,
        |b, input| {
            b.iter(|| {
                let s = std::str::from_utf8(black_box(input)).unwrap();
                let mut out = Vec::with_capacity(input.len());
                let mut char_buf = Vec::new();
                for ch in s.chars() {
                    char_buf.clear();
                    if map.lookup_into(ch, &mut char_buf) {
                        for &tc in &char_buf {
                            let mut enc = [0u8; 4];
                            out.extend_from_slice(tc.encode_utf8(&mut enc).as_bytes());
                        }
                    } else {
                        let mut enc = [0u8; 4];
                        out.extend_from_slice(ch.encode_utf8(&mut enc).as_bytes());
                    }
                }
                out
            });
        },
    );

    // Real-world: cargo output with homographs.
    let real = unicode_real_world();
    group.throughput(Throughput::Bytes(real.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("real_world_cargo", real.len()),
        &real,
        |b, input| {
            b.iter(|| {
                let s = std::str::from_utf8(black_box(input)).unwrap();
                let mut out = Vec::with_capacity(input.len());
                let mut char_buf = Vec::new();
                for ch in s.chars() {
                    char_buf.clear();
                    if map.lookup_into(ch, &mut char_buf) {
                        for &tc in &char_buf {
                            let mut enc = [0u8; 4];
                            out.extend_from_slice(tc.encode_utf8(&mut enc).as_bytes());
                        }
                    } else {
                        let mut enc = [0u8; 4];
                        out.extend_from_slice(ch.encode_utf8(&mut enc).as_bytes());
                    }
                }
                out
            });
        },
    );

    // Lookup-only: measure just the map lookup cost per char.
    let fullwidth_chars: Vec<char> = (0xFF01..=0xFF5Eu32)
        .map(|cp| char::from_u32(cp).unwrap())
        .collect();
    group.bench_function("lookup_fullwidth_94", |b| {
        b.iter(|| {
            for &c in black_box(&fullwidth_chars) {
                let _ = black_box(map.lookup_char(c));
            }
        });
    });

    let ascii_chars: Vec<char> = (0x20..=0x7Eu32)
        .map(|cp| char::from_u32(cp).unwrap())
        .collect();
    group.bench_function("lookup_ascii_miss_95", |b| {
        b.iter(|| {
            for &c in black_box(&ascii_chars) {
                let _ = black_box(map.lookup_char(c));
            }
        });
    });

    group.finish();
}
