//! Benchmark configuration — single source of truth for all bench binaries.

use criterion::BenchmarkGroup;
use criterion::measurement::WallTime;

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
    fn parse_size_variants() {
        assert_eq!(parse_size("4096"), Some(4096));
        assert_eq!(parse_size("64M"), Some(64 * 1024 * 1024));
        assert_eq!(parse_size("1G"), Some(1024 * 1024 * 1024));
        assert_eq!(parse_size("32K"), Some(32 * 1024));
        assert_eq!(parse_size("32k"), Some(32 * 1024));
        assert_eq!(parse_size(""), None);
    }
}
