//! Benchmark configuration — single source of truth for all bench binaries.

use criterion::BenchmarkGroup;
use criterion::measurement::WallTime;

/// Measurement parameters shared across all bench binaries.
///
/// 10 samples × 3s measurement = Criterion packs thousands of
/// iterations per sample for fast benchmarks, a few iterations
/// for slow ones. Statistically sound either way.
pub struct BenchConfig {
    pub sample_size: usize,
    pub measurement_secs: u64,
    pub warmup_secs: u64,
    pub max_size: usize,
}

impl BenchConfig {
    /// Standard config. Reads `BENCH_MAX_SIZE` from env.
    #[must_use]
    pub fn from_env(default_max: usize) -> Self {
        let max_size = match std::env::var("BENCH_MAX_SIZE") {
            Ok(s) if s == "0" => usize::MAX,
            Ok(s) => parse_size(&s).unwrap_or(default_max),
            Err(_) => default_max,
        };
        Self {
            sample_size: 10,
            measurement_secs: 3,
            warmup_secs: 1,
            max_size,
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
