use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use std::sync::OnceLock;
use distill_bench_harness::{
    BenchConfig, CacheInfo, CapturePoint, FlushParams, ResourceTracker,
    clean_input, dirty_input, flush_resources,
};

// ── Shared resource tracker (spans all benchmark functions) ─────────

fn tracker() -> &'static ResourceTracker {
    static T: OnceLock<ResourceTracker> = OnceLock::new();
    T.get_or_init(ResourceTracker::new)
}

fn capture(name: &str, size: usize) -> CapturePoint<'_> {
    CapturePoint { crate_name: name, size }
}

// ── Test data ───────────────────────────────────────────────────────

fn real_world_cargo() -> Vec<u8> {
    let mut v = Vec::new();
    for _ in 0..100 {
        v.extend_from_slice(b"\x1b[0m\x1b[1m\x1b[32m   Compiling\x1b[0m memchr v2.7.1\n");
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

// ── Benchmarks ──────────────────────────────────────────────────────

fn bench_strip(c: &mut Criterion) {
    let mut group = c.benchmark_group("strip");
    BenchConfig::from_env(usize::MAX).apply(&mut group);

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
        |b, input| { b.iter(|| strip_ansi::strip(black_box(input))); },
    );

    let osc8 = real_world_osc8();
    group.throughput(Throughput::Bytes(osc8.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("real_osc8", osc8.len()),
        &osc8,
        |b, input| { b.iter(|| strip_ansi::strip(black_box(input))); },
    );

    group.finish();
}

fn bench_strip_in_place(c: &mut Criterion) {
    let mut group = c.benchmark_group("strip_in_place");
    BenchConfig::from_env(usize::MAX).apply(&mut group);

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
    BenchConfig::from_env(usize::MAX).apply(&mut group);

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
    BenchConfig::from_env(usize::MAX).apply(&mut group);

    let cache = CacheInfo::detect();
    let sizes: Vec<usize> = vec![
        1024,
        cache.l1d as usize,
        cache.l2 as usize,
        cache.l3 as usize,
        cache.l3 as usize * 2,
    ];

    for &size in &sizes {
        let dirty = dirty_input(size);
        group.throughput(Throughput::Bytes(size as u64));

        let t = tracker();
        t.before(capture("stream/strip_slices", size));
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
        t.after(capture("stream/strip_slices", size));
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

fn bench_classifier(c: &mut Criterion) {
    use strip_ansi::{ClassifyingParser, SeqAction};

    let mut group = c.benchmark_group("classifier");
    BenchConfig::from_env(usize::MAX).apply(&mut group);
    let t = tracker();

    let cargo = real_world_cargo();
    let osc8 = real_world_osc8();

    group.throughput(Throughput::Bytes(cargo.len() as u64));
    t.before(capture("classifier/cargo_classify", cargo.len()));
    group.bench_with_input(
        BenchmarkId::new("cargo_classify", cargo.len()),
        &cargo,
        |b, input| {
            b.iter(|| {
                let mut cp = ClassifyingParser::new();
                for &byte in black_box(input) { let _ = cp.feed(byte); }
            });
        },
    );
    t.after(capture("classifier/cargo_classify", cargo.len()));

    group.throughput(Throughput::Bytes(osc8.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("osc8_classify", osc8.len()),
        &osc8,
        |b, input| {
            b.iter(|| {
                let mut cp = ClassifyingParser::new();
                for &byte in black_box(input) { let _ = cp.feed(byte); }
            });
        },
    );

    group.throughput(Throughput::Bytes(cargo.len() as u64));
    t.before(capture("classifier/cargo_classify_detail", cargo.len()));
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
    t.after(capture("classifier/cargo_classify_detail", cargo.len()));

    group.finish();
}

fn bench_filter_detail(c: &mut Criterion) {
    use strip_ansi::{FilterConfig, OscType, SeqKind, SgrContent, filter_strip};

    let mut group = c.benchmark_group("filter_detail");
    BenchConfig::from_env(usize::MAX).apply(&mut group);
    let t = tracker();

    let cargo = real_world_cargo();
    group.throughput(Throughput::Bytes(cargo.len() as u64));

    let config_kind_only = FilterConfig::strip_all().no_strip_kind(SeqKind::CsiSgr);
    t.before(capture("filter_detail/kind_only", cargo.len()));
    group.bench_with_input(
        BenchmarkId::new("kind_only", cargo.len()),
        &cargo,
        |b, input| { b.iter(|| filter_strip(black_box(input), &config_kind_only)); },
    );
    t.after(capture("filter_detail/kind_only", cargo.len()));

    let config_sgr_mask = FilterConfig::strip_all()
        .no_strip_kind(SeqKind::CsiSgr)
        .with_sgr_mask(SgrContent::BASIC);
    t.before(capture("filter_detail/sgr_mask", cargo.len()));
    group.bench_with_input(
        BenchmarkId::new("sgr_mask", cargo.len()),
        &cargo,
        |b, input| { b.iter(|| filter_strip(black_box(input), &config_sgr_mask)); },
    );
    t.after(capture("filter_detail/sgr_mask", cargo.len()));

    let config_osc_preserve = FilterConfig::strip_all()
        .no_strip_kind(SeqKind::CsiSgr)
        .no_strip_osc_type(OscType::Title)
        .no_strip_osc_type(OscType::Hyperlink);
    let osc8 = real_world_osc8();
    group.throughput(Throughput::Bytes(osc8.len() as u64));
    t.before(capture("filter_detail/osc_preserve", osc8.len()));
    group.bench_with_input(
        BenchmarkId::new("osc_preserve", osc8.len()),
        &osc8,
        |b, input| { b.iter(|| filter_strip(black_box(input), &config_osc_preserve)); },
    );
    t.after(capture("filter_detail/osc_preserve", osc8.len()));

    let config_sanitize = strip_ansi::TerminalPreset::Sanitize.to_filter_config();
    group.throughput(Throughput::Bytes(cargo.len() as u64));
    t.before(capture("filter_detail/sanitize_preset", cargo.len()));
    group.bench_with_input(
        BenchmarkId::new("sanitize_preset", cargo.len()),
        &cargo,
        |b, input| { b.iter(|| filter_strip(black_box(input), &config_sanitize)); },
    );
    t.after(capture("filter_detail/sanitize_preset", cargo.len()));

    group.finish();
}

fn bench_check_threats(c: &mut Criterion) {
    use strip_ansi::{ClassifyingParser, SeqAction, SeqKind};

    let mut group = c.benchmark_group("check_threats");
    BenchConfig::from_env(usize::MAX).apply(&mut group);
    let t = tracker();

    let mut threat_input = Vec::new();
    for _ in 0..100 {
        threat_input
            .extend_from_slice(b"\x1b[0m\x1b[1m\x1b[32m   Compiling\x1b[0m memchr v2.7.1\n");
    }
    threat_input.extend_from_slice(b"\x1b[21t");
    threat_input.extend_from_slice(b"\x1b]50;?\x07");
    threat_input.extend_from_slice(b"\x1bP$qm\x1b\\");

    group.throughput(Throughput::Bytes(threat_input.len() as u64));
    t.before(capture("check_threats/scan_only", threat_input.len()));
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
                        if matches!(d.kind, SeqKind::Dcs | SeqKind::CsiQuery)
                            || (d.kind == SeqKind::Osc && d.osc_number == 50)
                        { threats += 1; }
                    }
                }
                threats
            });
        },
    );
    t.after(capture("check_threats/scan_only", threat_input.len()));

    let clean_cargo = real_world_cargo();
    group.throughput(Throughput::Bytes(clean_cargo.len() as u64));
    t.before(capture("check_threats/scan_clean", clean_cargo.len()));
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
                        if matches!(d.kind, SeqKind::Dcs | SeqKind::CsiQuery)
                            || (d.kind == SeqKind::Osc && d.osc_number == 50)
                        { threats += 1; }
                    }
                }
                threats
            });
        },
    );
    t.after(capture("check_threats/scan_clean", clean_cargo.len()));

    group.finish();
}

fn bench_transform_pipeline(c: &mut Criterion) {
    use strip_ansi::downgrade::ColorDepth;
    use strip_ansi::{TransformConfig, TransformStream};

    let mut group = c.benchmark_group("transform");
    BenchConfig::from_env(usize::MAX).apply(&mut group);
    let t = tracker();

    let mut truecolor_input = Vec::new();
    for i in 0..100u8 {
        truecolor_input.extend_from_slice(
            format!("\x1b[38;2;{};{};{}m   Compiling\x1b[0m crate v0.{}.0\n", i, 255 - i, 128, i)
                .as_bytes(),
        );
    }
    let mut color256_input = Vec::new();
    for i in 0..100u8 {
        color256_input.extend_from_slice(
            format!("\x1b[38;5;{}m   Compiling\x1b[0m crate v0.{}.0\n", i, i).as_bytes(),
        );
    }
    let basic_input = real_world_cargo();

    macro_rules! bench_xform {
        ($name:expr, $input:expr, $depth:expr) => {
            let config = TransformConfig::new($depth);
            group.throughput(Throughput::Bytes($input.len() as u64));
            t.before(capture(concat!("transform/", $name), $input.len()));
            group.bench_with_input(
                BenchmarkId::new($name, $input.len()),
                &$input,
                |b, input| {
                    b.iter(|| {
                        let mut stream = TransformStream::new();
                        let mut out = Vec::with_capacity(input.len());
                        stream.push(black_box(input), &config, &mut out);
                        stream.finish();
                        out
                    });
                },
            );
            t.after(capture(concat!("transform/", $name), $input.len()));
        };
    }

    bench_xform!("truecolor_to_mono", truecolor_input, ColorDepth::Mono);
    bench_xform!("truecolor_to_greyscale", truecolor_input, ColorDepth::Greyscale);
    bench_xform!("truecolor_to_16", truecolor_input, ColorDepth::Color16);
    bench_xform!("truecolor_to_256", truecolor_input, ColorDepth::Color256);
    bench_xform!("256_to_mono", color256_input, ColorDepth::Mono);
    bench_xform!("256_to_greyscale", color256_input, ColorDepth::Greyscale);
    bench_xform!("256_to_16", color256_input, ColorDepth::Color16);
    bench_xform!("basic_to_mono", basic_input, ColorDepth::Mono);
    bench_xform!("passthrough", basic_input, ColorDepth::Truecolor);

    group.finish();
}

fn bench_augment_color(c: &mut Criterion) {
    use strip_ansi::palette::{
        DEUTERANOPIA_VIENOT, PROTANOPIA_VIENOT, PaletteTransform, linear_to_srgb, srgb_to_linear,
    };

    let mut group = c.benchmark_group("augment_color");
    BenchConfig::from_env(usize::MAX).apply(&mut group);
    let t = tracker();

    let rgb_values: Vec<(u8, u8, u8)> = (0..256u16)
        .map(|i| (i as u8, (255 - i) as u8, 128))
        .collect();

    let proto = PaletteTransform::from_matrix(PROTANOPIA_VIENOT);
    let deuter = PaletteTransform::from_matrix(DEUTERANOPIA_VIENOT);

    group.throughput(Throughput::Elements(rgb_values.len() as u64));

    t.before(capture("augment_color/protanopia_256", 768));
    group.bench_function("protanopia_256", |b| {
        b.iter(|| {
            for &(r, g, bl) in black_box(&rgb_values) {
                let _ = black_box(proto.transform(r, g, bl));
            }
        });
    });
    t.after(capture("augment_color/protanopia_256", 768));

    t.before(capture("augment_color/deuteranopia_256", 768));
    group.bench_function("deuteranopia_256", |b| {
        b.iter(|| {
            for &(r, g, bl) in black_box(&rgb_values) {
                let _ = black_box(deuter.transform(r, g, bl));
            }
        });
    });
    t.after(capture("augment_color/deuteranopia_256", 768));

    t.before(capture("augment_color/srgb_roundtrip_256", 256));
    group.bench_function("srgb_roundtrip_256", |b| {
        b.iter(|| {
            for i in 0..=255u8 {
                let lin = srgb_to_linear(black_box(i));
                let _ = black_box(linear_to_srgb(lin));
            }
        });
    });
    t.after(capture("augment_color/srgb_roundtrip_256", 256));

    group.finish();
}

// ── Unicode normalization benchmarks ────────────────────────────────

fn unicode_clean_ascii(size: usize) -> Vec<u8> {
    (0..size).map(|i| b'A' + (i % 26) as u8).collect()
}

fn unicode_fullwidth_mixed(size: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(size);
    let mut i = 0u32;
    while v.len() < size {
        if i % 5 == 0 {
            let cp = 0xFF21 + (i % 26);
            let c = char::from_u32(cp).unwrap_or('Ａ');
            let mut buf = [0u8; 3];
            let s = c.encode_utf8(&mut buf);
            if v.len() + s.len() <= size { v.extend_from_slice(s.as_bytes()); }
        } else {
            v.push(b'A' + (i % 26) as u8);
        }
        i += 1;
    }
    v.truncate(size);
    v
}

fn unicode_math_bold_mixed(size: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(size);
    let mut i = 0u32;
    while v.len() < size {
        if i % 5 == 0 {
            let cp = 0x1D400 + (i % 26);
            let c = char::from_u32(cp).unwrap_or('\u{1D400}');
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            if v.len() + s.len() <= size { v.extend_from_slice(s.as_bytes()); }
        } else {
            v.push(b'A' + (i % 26) as u8);
        }
        i += 1;
    }
    v.truncate(size);
    v
}

fn unicode_real_world() -> Vec<u8> {
    let mut v = Vec::new();
    for _ in 0..50 {
        v.extend_from_slice(b"   Compiling memchr v2.7.1\n");
        v.extend_from_slice("   Ｃompiling ｆake-crate v０.１.０\n".as_bytes());
    }
    v
}

fn bench_unicode_normalize(c: &mut Criterion) {
    use strip_ansi::unicode_map::UnicodeMap;

    let mut group = c.benchmark_group("unicode_normalize");
    BenchConfig::from_env(usize::MAX).apply(&mut group);
    let t = tracker();
    let map = UnicodeMap::builtin();

    for size in [1024, 4096, 16384] {
        let clean = unicode_clean_ascii(size);
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::new("clean_ascii", size), &clean, |b, input| {
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
        });
    }

    for size in [1024, 4096, 16384] {
        let mixed = unicode_fullwidth_mixed(size);
        group.throughput(Throughput::Bytes(mixed.len() as u64));
        group.bench_with_input(BenchmarkId::new("fullwidth_mixed", size), &mixed, |b, input| {
            b.iter(|| {
                let s = std::str::from_utf8(black_box(input)).unwrap();
                let mut out = Vec::with_capacity(input.len());
                let mut char_buf = Vec::new();
                for ch in s.chars() {
                    char_buf.clear();
                    if map.lookup_into(ch, &mut char_buf) {
                        for &tc in &char_buf { let mut enc = [0u8; 4]; out.extend_from_slice(tc.encode_utf8(&mut enc).as_bytes()); }
                    } else {
                        let mut enc = [0u8; 4]; out.extend_from_slice(ch.encode_utf8(&mut enc).as_bytes());
                    }
                }
                out
            });
        });
    }

    let math = unicode_math_bold_mixed(4096);
    group.throughput(Throughput::Bytes(math.len() as u64));
    group.bench_with_input(BenchmarkId::new("math_bold_mixed", math.len()), &math, |b, input| {
        b.iter(|| {
            let s = std::str::from_utf8(black_box(input)).unwrap();
            let mut out = Vec::with_capacity(input.len());
            let mut char_buf = Vec::new();
            for ch in s.chars() {
                char_buf.clear();
                if map.lookup_into(ch, &mut char_buf) {
                    for &tc in &char_buf { let mut enc = [0u8; 4]; out.extend_from_slice(tc.encode_utf8(&mut enc).as_bytes()); }
                } else {
                    let mut enc = [0u8; 4]; out.extend_from_slice(ch.encode_utf8(&mut enc).as_bytes());
                }
            }
            out
        });
    });

    let real = unicode_real_world();
    group.throughput(Throughput::Bytes(real.len() as u64));
    t.before(capture("unicode_normalize/real_world_cargo", real.len()));
    group.bench_with_input(BenchmarkId::new("real_world_cargo", real.len()), &real, |b, input| {
        b.iter(|| {
            let s = std::str::from_utf8(black_box(input)).unwrap();
            let mut out = Vec::with_capacity(input.len());
            let mut char_buf = Vec::new();
            for ch in s.chars() {
                char_buf.clear();
                if map.lookup_into(ch, &mut char_buf) {
                    for &tc in &char_buf { let mut enc = [0u8; 4]; out.extend_from_slice(tc.encode_utf8(&mut enc).as_bytes()); }
                } else {
                    let mut enc = [0u8; 4]; out.extend_from_slice(ch.encode_utf8(&mut enc).as_bytes());
                }
            }
            out
        });
    });
    t.after(capture("unicode_normalize/real_world_cargo", real.len()));

    let fullwidth_chars: Vec<char> = (0xFF01..=0xFF5Eu32).map(|cp| char::from_u32(cp).unwrap()).collect();
    group.bench_function("lookup_fullwidth_94", |b| {
        b.iter(|| { for &c in black_box(&fullwidth_chars) { let _ = black_box(map.lookup_char(c)); } });
    });

    let ascii_chars: Vec<char> = (0x20..=0x7Eu32).map(|cp| char::from_u32(cp).unwrap()).collect();
    group.bench_function("lookup_ascii_miss_95", |b| {
        b.iter(|| { for &c in black_box(&ascii_chars) { let _ = black_box(map.lookup_char(c)); } });
    });

    group.finish();
}

// ── Resource flush (must be last) ───────────────────────────────────

fn bench_flush_resources(c: &mut Criterion) {
    // Flush the shared ResourceTracker to JSON.
    // This is a no-op benchmark — just triggers the flush.
    let cache = CacheInfo::detect();
    flush_resources(FlushParams {
        tracker: tracker(),
        cache: &cache,
        sizes: &[],
    });

    // Criterion requires at least one benchmark in a group.
    let mut group = c.benchmark_group("_flush");
    group.sample_size(10);
    group.bench_function("noop", |b| b.iter(|| {}));
    group.finish();
    let _ = c;
}

criterion_group!(
    benches,
    bench_strip,
    bench_strip_in_place,
    bench_contains_ansi,
    bench_stream,
    bench_classifier,
    bench_filter_detail,
    bench_check_threats,
    bench_transform_pipeline,
    bench_augment_color,
    bench_unicode_normalize,
    bench_flush_resources,
);
criterion_main!(benches);
