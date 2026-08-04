//! Benchmark configuration — single source of truth for all bench binaries.

use criterion::BenchmarkGroup;
use criterion::SamplingMode;
use criterion::measurement::WallTime;
use std::time::{Duration, Instant};

/// Measurement parameters shared across all bench binaries.
///
/// ## Defaults and rationale
///
/// - `sample_size: 200` — enough iterations-per-sample that a
///   single context switch or stray IRQ doesn't dominate the CI.
///   At 100 × 5s (the earlier value) sub-µs benches showed 20-40%
///   CV. 200 × 9s tightens that to the 3-8% range on a quiet
///   Linux workstation without requiring CPU pinning. That trades
///   ~4min more wall time for numbers you can actually publish.
/// - `measurement_secs: 9` — at larger sample counts criterion
///   must pack more iters per sample to avoid floor-ing the batch
///   at "1 iteration". 9s leaves headroom for the cold cache-tier
///   benches while keeping total bench time under 15 min.
/// - `warmup_secs: 3` — branch predictor, BTB, uop cache, and
///   iTLB all need more than the default 1s to warm for
///   small-input benches. At 1s warmup the first measured samples
///   on OSC-8-class inputs ran 15-20% slow.
/// - `max_size` — bounded by L3 × 2 unless overridden with
///   `BENCH_MAX_SIZE`.
///
/// Override with `BENCH_QUICK=1` for a 20s/bench quick-check run
/// (sample_size=20, measurement=1s, warmup=500ms) when iterating
/// on the algorithm. Don't publish numbers from quick runs.
pub struct BenchConfig {
    pub sample_size: usize,
    pub measurement_secs: u64,
    pub warmup_secs: u64,
    pub max_size: usize,
}

impl BenchConfig {
    /// Standard config. Reads `BENCH_MAX_SIZE` and `BENCH_QUICK` from env.
    #[must_use]
    pub fn from_env(default_max: usize) -> Self {
        let max_size = match std::env::var("BENCH_MAX_SIZE") {
            Ok(s) if s == "0" => usize::MAX,
            Ok(s) => parse_size(&s).unwrap_or(default_max),
            Err(_) => default_max,
        };

        let quick = std::env::var("BENCH_QUICK")
            .ok()
            .map(|v| v != "0" && !v.is_empty())
            .unwrap_or(false);

        if quick {
            Self {
                sample_size: 20,
                measurement_secs: 1,
                warmup_secs: 1,
                max_size,
            }
        } else {
            // 200 samples × 9s measurement gives criterion enough
            // iterations-per-sample to absorb single-context-switch
            // outliers on sub-µs benches. Empirically this tightens
            // CV from 20-40% (at 100×5s) to 3-8% on a quiet Linux
            // box — without requiring CPU pinning.
            Self {
                sample_size: 200,
                measurement_secs: 9,
                warmup_secs: 3,
                max_size,
            }
        }
    }

    /// Apply this config to a criterion benchmark group.
    pub fn apply(&self, group: &mut BenchmarkGroup<'_, WallTime>) {
        group.sample_size(self.sample_size);
        group.measurement_time(std::time::Duration::from_secs(self.measurement_secs));
        group.warm_up_time(std::time::Duration::from_secs(self.warmup_secs));
    }

    /// Apply this config to a group, sizing `sample_size` from the
    /// *measured* cost of one call to `probe` so the run stays clear
    /// of criterion's "Unable to complete N samples" floor — while
    /// keeping at least 10 samples (criterion's own hard minimum) and
    /// extending `measurement_secs` only for the rare crate/size
    /// combination where even 10 samples would still floor.
    ///
    /// ## Why measured cost, not input size or cache geometry
    ///
    /// An earlier version of this tiered a fixed schedule off input
    /// size vs. L2/L3 boundaries. That doesn't hold across a
    /// multi-crate ecosystem comparison: `distill-strip-ansi` and
    /// `strip-ansi-escapes` differ by 10-16x in throughput at the
    /// *same* byte count (see `doc/BENCHMARKS.md`), so a size-keyed
    /// schedule either under-precisions the fast crate or still
    /// floors the slow one — both observed in practice
    /// (`strip_ansi_escapes_dirty` warned as early as 32 KiB, well
    /// inside the "full precision" tier sized for `distill`).
    /// Sizing off the actual measured per-call cost fixes this
    /// structurally: a fast crate/size combination never gets
    /// reduced regardless of byte count, and only the genuinely slow
    /// ones do — derived fresh per call, no per-crate constants.
    ///
    /// ## The math — mirrors criterion's own formulas exactly
    ///
    /// Criterion's `SamplingMode::Auto` already picks Linear vs. Flat
    /// internally once it knows the mean iteration time (see
    /// `criterion::SamplingMode::choose_sampling_mode`); it does not,
    /// however, reduce `sample_size` itself — that's set once via
    /// `.sample_size(n)` and stays fixed. The "Unable to complete"
    /// warning fires when, at the `n` we gave it, whichever mode Auto
    /// picked still floors (Linear's per-sample multiplier `d==1`, or
    /// Flat's `iterations_per_sample==1`) — and criterion's own
    /// warning text already recommends a specific fix: "reduce sample
    /// count to N". `would_pick_flat`, `recommend_linear_sample_size`,
    /// and `recommend_flat_sample_size` below reimplement exactly the
    /// formulas behind that recommendation
    /// (`criterion::SamplingMode::choose_sampling_mode` and
    /// `criterion::ActualSamplingMode::{recommend_linear_sample_size,
    /// recommend_flat_sample_size}`), using our own single-call
    /// `probe` measurement as a stand-in for criterion's warmup mean.
    /// Verified against this repo's own real warning history — e.g.
    /// `distill_dirty/67108864`'s observed "reduce sample count to
    /// 110" matches `recommend_flat_sample_size` bit-for-bit given
    /// that benchmark's measured mean iteration time.
    ///
    /// The recommend_* functions always return >= 10 by construction
    /// (they clamp to criterion's own hard minimum even when that's
    /// mathematically insufficient) — so after picking `n`, we
    /// explicitly re-check whether `n` still floors and, only if so,
    /// extend `measurement_secs` to what 10 samples actually need
    /// (with a 2x margin so real variance doesn't tip it back onto
    /// the floor). Because `sampling_mode` is always left as `Auto`,
    /// criterion re-derives Linear vs. Flat itself from its own real
    /// warmup measurement — our estimate only has to be right enough
    /// to pick a reasonable `n`, not exactly match criterion's number.
    ///
    /// ## Why `n(n+1)/2`, and why we never predict the mode
    ///
    /// Two earlier versions of this got the arithmetic wrong in ways
    /// that produced the exact warnings it was meant to prevent.
    ///
    /// First: Linear sampling does **not** run `n` iterations. Sample
    /// `i` runs `i * d` iterations, so the schedule costs
    /// `d * n(n+1)/2` iterations in total — 55 at `n == 10`, 20100 at
    /// `n == 200`. The old fallback extended `measurement_time` to
    /// `10 * met * 2`, budgeting 20 iterations for a schedule that
    /// needs 55. Observed directly: at `strip_ansi_escapes_dirty/
    /// 50331648` we set a 40.0s target and criterion answered
    /// "increase target time to 55.7s", then reported "55 iterations".
    /// 55 × ~1.01s/iter is 55.7s. The estimate wasn't the problem;
    /// the iteration count was.
    ///
    /// Second: the old code predicted Linear-vs-Flat itself (a
    /// reimplementation of `choose_sampling_mode`) and then sized `n`
    /// for whichever mode it predicted. When the prediction and
    /// criterion's actual `Auto` choice disagreed, `n` was sized for
    /// the wrong schedule — badly so, because the two differ by a
    /// factor of `(n+1)/2`. That is what produced the near-miss
    /// warnings on the *fast* crates ("reduce sample count to 170"
    /// against our 200): Flat was predicted, `recommend_flat` returned
    /// a huge `n` clamped to the configured 200, and criterion then
    /// chose Linear and needed 20100 iterations to honour it.
    ///
    /// So this version does not predict the mode at all. Linear is
    /// always the more expensive schedule for a given `n`, so sizing
    /// `n` to fit Linear within the target is safe whichever mode
    /// `Auto` lands on. Only when even `MIN_SAMPLES` under Linear
    /// cannot fit do we switch to `Flat` explicitly — which is
    /// criterion's own advice in that situation ("or enable flat
    /// sampling"), and costs `n * iters_per_sample` iterations rather
    /// than `n(n+1)/2`, i.e. ~20 instead of 55 at `n == 10`.
    ///
    /// ## Probe budget
    ///
    /// `met` comes from a time-boxed probe loop rather than a fixed
    /// call count, mirroring how criterion's own warmup works: run
    /// until a wall-clock budget elapses, then divide. A fixed count
    /// can't adapt — three calls is a stable mean for a 90ns
    /// operation and a single cold sample for a 370ms one.
    /// `PROBE_MARGIN` keeps a little headroom on top, since the probe
    /// runs on a colder cache than criterion's post-warmup steady
    /// state.
    pub fn apply_tiered<F: FnMut()>(&self, group: &mut BenchmarkGroup<'_, WallTime>, probe: F) {
        group.warm_up_time(Duration::from_secs(self.warmup_secs));

        let met_secs = measure_time_boxed(probe) * PROBE_MARGIN;
        let mut target_secs = self.measurement_secs as f64;

        // Size for Linear: it is the costlier schedule, so an `n` that
        // fits Linear also fits Flat. Never exceed the configured
        // sample_size — this reduces precision when necessary, it
        // never silently raises it.
        let n = recommend_linear_sample_size(target_secs, met_secs)
            .min(self.sample_size)
            .max(MIN_SAMPLES);

        if linear_total_runs(n) * met_secs > target_secs {
            // Even the floor sample count can't complete a Linear
            // schedule inside the budget. Flat trades the quadratic
            // iteration count for a linear one; extend the target to
            // what Flat actually costs so `iters_per_sample` lands
            // above 1 rather than on criterion's floor.
            target_secs = (n as f64) * met_secs * FLOOR_MARGIN;
            group.sampling_mode(SamplingMode::Flat);
        } else {
            // Auto re-derives the mode from its own warmup mean; our
            // `n` is safe under either choice.
            group.sampling_mode(SamplingMode::Auto);
        }

        group.measurement_time(Duration::from_secs_f64(target_secs));
        group.sample_size(n);
    }
}

/// Criterion's own hard minimum sample count.
const MIN_SAMPLES: usize = 10;

/// Safety margin applied to the probe-measured mean before sizing
/// `sample_size`. See "Probe budget" on `apply_tiered` above.
const PROBE_MARGIN: f64 = 1.2;

/// Headroom applied when extending `measurement_time` for the Flat
/// regime. Above 1.0 so `iters_per_sample = ceil(target/met/n)`
/// evaluates to 2 rather than 1 — landing exactly on 1 is the
/// condition criterion warns about.
const FLOOR_MARGIN: f64 = 1.5;

/// Wall-clock budget for the cost probe.
const PROBE_BUDGET: Duration = Duration::from_millis(300);

/// Hard cap on probe iterations, so a nanosecond-scale operation
/// doesn't spin millions of times to fill the budget. 10k calls is
/// already a stable mean at that scale.
const PROBE_MAX_CALLS: u32 = 10_000;

/// Time repeated calls to `probe` for up to `PROBE_BUDGET` and return
/// the mean per-call cost in seconds, isolated from criterion's own
/// warmup.
///
/// Time-boxed rather than fixed-count so the sample count adapts to
/// the operation's cost: a sub-µs strip accumulates thousands of
/// calls, a 370ms multi-MiB scan gets one. Always runs at least once.
fn measure_time_boxed<F: FnMut()>(mut probe: F) -> f64 {
    let start = Instant::now();
    let mut calls: u32 = 0;
    loop {
        probe();
        calls += 1;
        if start.elapsed() >= PROBE_BUDGET || calls >= PROBE_MAX_CALLS {
            break;
        }
    }
    (start.elapsed().as_secs_f64() / f64::from(calls)).max(1e-9)
}

/// Total iterations a Linear schedule runs at `n` samples with the
/// per-sample multiplier `d == 1`: `1 + 2 + ... + n`.
fn linear_total_runs(n: usize) -> f64 {
    let n = n as f64;
    n * (n + 1.0) / 2.0
}

/// Replica of `criterion::ActualSamplingMode::recommend_linear_sample_size`.
fn recommend_linear_sample_size(target_secs: f64, met_secs: f64) -> usize {
    let c = target_secs / met_secs;
    let n = (-1.0 + (4.0 * c).sqrt()) / 2.0;
    let n = ((n.max(0.0) as usize) / 10) * 10;
    n.max(MIN_SAMPLES)
}

/// Parse human-readable size strings: "64M", "1G", "32K", "4096".
fn parse_size(s: &str) -> Option<usize> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (num, mult) = match s.as_bytes().last()? {
        b'K' | b'k' => (&s[..s.len() - 1], 1024),
        b'M' | b'm' => (&s[..s.len() - 1], 1024 * 1024),
        b'G' | b'g' => (&s[..s.len() - 1], 1024 * 1024 * 1024),
        _ => (s, 1),
    };
    num.trim().parse::<usize>().ok().map(|n| n * mult)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_total_runs_is_quadratic_not_linear() {
        // The regression that caused the flooring warnings: a Linear
        // schedule at n=10 costs 55 iterations, not 10 or 20.
        assert_eq!(linear_total_runs(10), 55.0);
        assert_eq!(linear_total_runs(200), 20100.0);
    }

    #[test]
    fn recommended_n_fits_the_linear_schedule() {
        // For a spread of per-call costs, the n we pick must actually
        // fit a Linear schedule inside the target — this is the
        // invariant the old mode-predicting version violated.
        let target = 9.0;
        for met in [1e-9, 1e-7, 1e-5, 1e-3, 1e-2, 0.1, 0.37, 1.0] {
            let n = recommend_linear_sample_size(target, met)
                .min(200)
                .max(MIN_SAMPLES);
            let cost = linear_total_runs(n) * met;
            // Either it fits, or n was clamped to the floor — in
            // which case apply_tiered switches to Flat and extends
            // the target instead.
            assert!(
                cost <= target || n == MIN_SAMPLES,
                "met={met}: n={n} needs {cost}s > {target}s target",
            );
        }
    }

    #[test]
    fn slow_operations_land_on_the_flat_floor() {
        // A 1s-per-call operation cannot do 55 iterations in 9s, so
        // it must be the clamped-to-floor case that selects Flat.
        let n = recommend_linear_sample_size(9.0, 1.0)
            .min(200)
            .max(MIN_SAMPLES);
        assert_eq!(n, MIN_SAMPLES);
        assert!(linear_total_runs(n) * 1.0 > 9.0);
        // Flat's extended target must give iters_per_sample >= 2.
        let target = (n as f64) * 1.0 * FLOOR_MARGIN;
        let iters_per_sample = (target / 1.0 / n as f64).ceil();
        assert!(iters_per_sample >= 2.0, "got {iters_per_sample}");
    }

    #[test]
    fn probe_runs_at_least_once_and_reports_positive_cost() {
        let mut calls = 0_u32;
        let met = measure_time_boxed(|| calls += 1);
        assert!(calls >= 1, "probe never ran");
        assert!(met > 0.0, "non-positive cost estimate: {met}");
        assert!(calls <= PROBE_MAX_CALLS, "exceeded call cap: {calls}");
    }

    #[test]
    fn parse_size_variants() {
        assert_eq!(parse_size("4096"), Some(4096));
        assert_eq!(parse_size("64M"), Some(64 * 1024 * 1024));
        assert_eq!(parse_size("1G"), Some(1024 * 1024 * 1024));
        assert_eq!(parse_size("32K"), Some(32 * 1024));
        assert_eq!(parse_size("32k"), Some(32 * 1024));
        assert_eq!(parse_size(""), None);
    }
}
