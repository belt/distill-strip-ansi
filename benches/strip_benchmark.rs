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
);
criterion_main!(benches);
