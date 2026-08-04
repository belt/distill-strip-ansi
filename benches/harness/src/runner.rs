//! Shared strip-benchmark runner — one function drives all per-crate
//! bench binaries with identical sizes, config, and resource capture.

use criterion::{BenchmarkId, Criterion, Throughput};
use std::collections::HashSet;
use std::hint::black_box;
use std::time::Instant;

use crate::{
    BenchConfig, CacheInfo, CapturePoint, FlushParams, InputSource, ResourceTracker, clean_input,
    flush_resources, fmt_bytes, load_fixture, select_input,
};

/// Parameters for a crate strip benchmark.
pub struct StripBench {
    /// Crate name for resource tracking (e.g. "distill-strip-ansi").
    pub crate_name: &'static str,
    /// Short ID for criterion benchmark names (e.g. "distill").
    pub bench_id: &'static str,
    /// The strip function: takes `&[u8]`, returns owned bytes.
    pub strip_fn: fn(&[u8]) -> Vec<u8>,
}

/// Run a complete strip benchmark suite for one crate.
///
/// Exercises dirty (all sizes — fixture-bucketed), clean (cache
/// boundaries), and real-world fixtures — with RSS/CPU capture.
pub fn run_strip_bench(c: &mut Criterion, bench: &StripBench) {
    let cache = CacheInfo::detect();
    let default_max = (cache.l3 as usize) * 2;
    let config = BenchConfig::from_env(default_max);
    let sizes = cache.build_sizes(config.max_size);
    let tracker = ResourceTracker::new();
    // Wall-clock reference for this bench binary's run. Printed via plain
    // eprintln! (unconditional, one line per event) so it survives being
    // redirected/piped — unlike criterion's own progress lines, which use
    // print_overwritable and collapse to a single carriage-return-updated
    // line on a real TTY. This is what actually proves elapsed real time
    // per benchmark ID, as opposed to criterion's pre-measurement estimate
    // (itself derived from a possibly noisy warmup, see config.rs).
    let bench_start = Instant::now();

    eprintln!(
        "[{}] Cache: L1d={}  L2={}  L3={}  RAM={}",
        bench.crate_name,
        fmt_bytes(cache.l1d),
        fmt_bytes(cache.l2),
        fmt_bytes(cache.l3),
        fmt_bytes(cache.ram),
    );
    eprintln!(
        "[{}] Sizes ({}): {:?}",
        bench.crate_name,
        sizes.len(),
        sizes
            .iter()
            .map(|&s| fmt_bytes(s as u64))
            .collect::<Vec<_>>(),
    );

    let mut group = c.benchmark_group("ecosystem");

    // ── Dirty: all sizes (fixture-bucketed) ─────────────────────
    let mut seen_ids: HashSet<(String, usize)> = HashSet::new();
    for &target_size in &sizes {
        let (input, meta) = select_input(target_size);
        let actual_size = input.len();

        let label = match &meta.source {
            InputSource::Fixture(name) => {
                let stem = name.strip_suffix(".raw.txt").unwrap_or(name);
                format!("{}_{}", bench.bench_id, stem)
            }
            InputSource::Generated => {
                format!("{}_dirty", bench.bench_id)
            }
        };

        // Deduplicate: if the same fixture was already benched at
        // another target size, fall back to generated input.
        let id_key = (label.clone(), actual_size);
        let (input, meta, label) = if seen_ids.insert(id_key) {
            (input, meta, label)
        } else {
            let fallback = crate::dirty_input(target_size);
            let fb_meta = crate::inputs::analyze_pub(&fallback);
            let fb_label = format!("{}_dirty", bench.bench_id);
            // Also deduplicate the generated fallback.
            let fb_key = (fb_label.clone(), fallback.len());
            if seen_ids.insert(fb_key) {
                // New element inserted, proceed with fallback
            } else {
                eprintln!("  {}: skipped (duplicate)", fmt_bytes(target_size as u64),);
                continue;
            }
            (fallback, fb_meta, fb_label)
        };
        let actual_size = input.len();

        eprintln!(
            "  {}: {}",
            fmt_bytes(target_size as u64),
            meta.verbose_display(),
        );

        config.apply_tiered(&mut group, || {
            black_box((bench.strip_fn)(black_box(&input)));
        });
        group.throughput(Throughput::Bytes(actual_size as u64));

        let point = CapturePoint {
            crate_name: bench.crate_name,
            size: actual_size,
        };
        tracker.before(point);

        let bench_id_str = format!("{label}/{actual_size}");
        eprintln!(
            "[{}] t+{:>8.3}s  START {}",
            bench.crate_name,
            bench_start.elapsed().as_secs_f64(),
            bench_id_str,
        );
        let iter_start = Instant::now();

        group.bench_with_input(BenchmarkId::new(&label, actual_size), &input, |b, inp| {
            b.iter(|| (bench.strip_fn)(black_box(inp)));
        });

        eprintln!(
            "[{}] t+{:>8.3}s  DONE  {} ({:.3}s wall)",
            bench.crate_name,
            bench_start.elapsed().as_secs_f64(),
            bench_id_str,
            iter_start.elapsed().as_secs_f64(),
        );

        let point = CapturePoint {
            crate_name: bench.crate_name,
            size: actual_size,
        };
        tracker.after(point);
    }

    // ── Clean: cache boundary sizes ─────────────────────────────
    let clean_sizes = build_clean_sizes(&cache, config.max_size);
    for &size in &clean_sizes {
        let input = clean_input(size);
        config.apply_tiered(&mut group, || {
            black_box((bench.strip_fn)(black_box(&input)));
        });
        group.throughput(Throughput::Bytes(size as u64));

        let bench_id_str = format!("{}_clean/{}", bench.bench_id, size);
        eprintln!(
            "[{}] t+{:>8.3}s  START {}",
            bench.crate_name,
            bench_start.elapsed().as_secs_f64(),
            bench_id_str,
        );
        let iter_start = Instant::now();

        group.bench_with_input(
            BenchmarkId::new(format!("{}_clean", bench.bench_id), size),
            &input,
            |b, inp| {
                b.iter(|| (bench.strip_fn)(black_box(inp)));
            },
        );

        eprintln!(
            "[{}] t+{:>8.3}s  DONE  {} ({:.3}s wall)",
            bench.crate_name,
            bench_start.elapsed().as_secs_f64(),
            bench_id_str,
            iter_start.elapsed().as_secs_f64(),
        );
    }

    // ── Real-world fixtures ─────────────────────────────────────
    let fixture_ctx = FixtureBenchCtx {
        tracker: &tracker,
        config: &config,
        bench,
        bench_start,
    };
    if let Some(cargo) = load_fixture("cargo-test.raw.txt") {
        bench_fixture(&mut group, &fixture_ctx, &cargo, "cargo");
    }
    if let Some(osc8) = load_fixture("brew-upgrade.raw.txt") {
        bench_fixture(&mut group, &fixture_ctx, &osc8, "osc8");
    }

    group.finish();

    flush_resources(FlushParams {
        tracker: &tracker,
        cache: &cache,
        sizes: &sizes,
    });
}

/// Shared, per-run context for `bench_fixture` calls — everything that's
/// constant across every real-world fixture benched in one
/// `run_strip_bench` call. Bundled into a struct (rather than passed as
/// separate parameters) to keep `bench_fixture` under clippy's
/// `too_many_arguments` threshold and because these values genuinely
/// travel together as one "benchmark session" concept.
struct FixtureBenchCtx<'a> {
    tracker: &'a ResourceTracker,
    config: &'a BenchConfig,
    bench: &'a StripBench,
    bench_start: Instant,
}

fn bench_fixture(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    ctx: &FixtureBenchCtx<'_>,
    input: &[u8],
    label: &str,
) {
    let size = input.len();
    ctx.config.apply_tiered(group, || {
        black_box((ctx.bench.strip_fn)(black_box(input)));
    });
    group.throughput(Throughput::Bytes(size as u64));

    let point = CapturePoint {
        crate_name: ctx.bench.crate_name,
        size,
    };
    ctx.tracker.before(point);

    let bench_id_str = format!("{}_{}/{}", ctx.bench.bench_id, label, size);
    eprintln!(
        "[{}] t+{:>8.3}s  START {}",
        ctx.bench.crate_name,
        ctx.bench_start.elapsed().as_secs_f64(),
        bench_id_str,
    );
    let iter_start = Instant::now();

    group.bench_with_input(
        BenchmarkId::new(format!("{}_{}", ctx.bench.bench_id, label), size),
        input,
        |b, inp| {
            b.iter(|| (ctx.bench.strip_fn)(black_box(inp)));
        },
    );

    eprintln!(
        "[{}] t+{:>8.3}s  DONE  {} ({:.3}s wall)",
        ctx.bench.crate_name,
        ctx.bench_start.elapsed().as_secs_f64(),
        bench_id_str,
        iter_start.elapsed().as_secs_f64(),
    );

    let point = CapturePoint {
        crate_name: ctx.bench.crate_name,
        size,
    };
    ctx.tracker.after(point);
}

fn build_clean_sizes(cache: &CacheInfo, max_size: usize) -> Vec<usize> {
    let mut cs = vec![256, 4096];
    for &boundary in &[cache.l1d, cache.l2, cache.l3] {
        if boundary > 0 {
            cs.push(boundary as usize);
        }
    }
    if cache.l3 > 0 {
        cs.push(cache.l3 as usize * 2);
    }
    cs.retain(|&s| s <= max_size);
    cs.sort();
    cs.dedup();
    cs
}
