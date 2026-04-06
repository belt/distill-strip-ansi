//! Hardware cache detection — L1d, L2, L3, RAM.

/// Detected cache hierarchy sizes in bytes.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct CacheInfo {
    pub l1d: u64,
    pub l2: u64,
    pub l3: u64,
    pub ram: u64,
}

impl CacheInfo {
    /// Detect cache sizes from the OS.
    ///
    /// Falls back to conservative defaults if detection fails.
    #[must_use]
    pub fn detect() -> Self {
        let raw = detect_raw();
        Self {
            l1d: raw.l1d.max(32_768),
            l2: raw.l2.max(262_144),
            l3: raw.l3.max(12_582_912),
            ram: raw.ram.max(1_073_741_824),
        }
    }

    /// Build hardware-adaptive benchmark sizes from cache boundaries.
    ///
    /// Returns sizes spanning L1 → L2 → L3 → DRAM, with
    /// power-of-two steps between boundaries.
    #[must_use]
    pub fn build_sizes(&self, max_size: usize) -> Vec<usize> {
        let mut sizes = Vec::with_capacity(20);

        // Sub-L1: 256, 512
        for &s in &[256, 512] {
            if s <= max_size {
                sizes.push(s);
            }
        }

        // L1 range: 1K up to L1d
        let mut s = 1024;
        while s <= self.l1d as usize && s <= max_size {
            sizes.push(s);
            s *= 2;
        }

        // L2 range: L1d*2 up to L2
        s = (self.l1d as usize) * 2;
        while s <= self.l2 as usize && s <= max_size {
            sizes.push(s);
            s *= 2;
        }

        // L3 range: L2*2 up to L3
        s = (self.l2 as usize) * 2;
        while s <= self.l3 as usize && s <= max_size {
            sizes.push(s);
            s *= 2;
        }

        // Beyond L3: power-of-two steps up to max_size.
        let mut s = (self.l3 as usize) * 2;
        while s <= max_size {
            sizes.push(s);
            s = match s.checked_mul(2) {
                Some(next) => next,
                None => break,
            };
        }

        // Include max_size itself if it wasn't hit by a power-of-two step.
        // This ensures --max-size 32M actually benchmarks at 32M.
        if max_size > 0 && max_size != usize::MAX {
            let last = sizes.last().copied().unwrap_or(0);
            if max_size > last {
                sizes.push(max_size);
            }
        }

        sizes.sort();
        sizes.dedup();
        sizes
    }
}

fn detect_raw() -> CacheInfo {
    #[cfg(target_os = "macos")]
    return detect_macos();

    #[cfg(target_os = "linux")]
    return detect_linux();

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    CacheInfo::default()
}

#[cfg(target_os = "macos")]
fn detect_macos() -> CacheInfo {
    fn sysctl_u64(name: &str) -> u64 {
        std::process::Command::new("sysctl")
            .args(["-n", name])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0)
    }
    CacheInfo {
        l1d: sysctl_u64("hw.l1dcachesize"),
        l2: sysctl_u64("hw.l2cachesize"),
        l3: sysctl_u64("hw.l3cachesize"),
        ram: sysctl_u64("hw.memsize"),
    }
}

#[cfg(target_os = "linux")]
fn detect_linux() -> CacheInfo {
    fn read_cache(index: u32) -> u64 {
        let path = format!("/sys/devices/system/cpu/cpu0/cache/index{index}/size");
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| {
                let s = s.trim().to_uppercase();
                if let Some(k) = s.strip_suffix('K') {
                    k.parse::<u64>().ok().map(|n| n * 1024)
                } else if let Some(m) = s.strip_suffix('M') {
                    m.parse::<u64>().ok().map(|n| n * 1024 * 1024)
                } else {
                    None
                }
            })
            .unwrap_or(0)
    }
    fn memtotal() -> u64 {
        std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("MemTotal:"))
                    .and_then(|l| {
                        l.split_whitespace()
                            .nth(1)
                            .and_then(|n| n.parse::<u64>().ok())
                            .map(|kb| kb * 1024)
                    })
            })
            .unwrap_or(0)
    }
    CacheInfo {
        l1d: read_cache(0),
        l2: read_cache(2),
        l3: read_cache(3),
        ram: memtotal(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cache() -> CacheInfo {
        CacheInfo {
            l1d: 32_768,
            l2: 262_144,
            l3: 12_582_912,
            ram: 34_359_738_368,
        }
    }

    #[test]
    fn build_sizes_includes_max_size() {
        let cache = test_cache();
        let sizes = cache.build_sizes(32 * 1024 * 1024); // 32M
        assert!(
            sizes.contains(&(32 * 1024 * 1024)),
            "32M should be in sizes: {sizes:?}"
        );
    }

    #[test]
    fn build_sizes_beyond_l3_has_steps() {
        let cache = test_cache();
        let sizes = cache.build_sizes(128 * 1024 * 1024); // 128M
        // Should have L3*2=24M, L3*4=48M, L3*8=96M, plus 128M
        assert!(
            sizes.contains(&(24 * 1024 * 1024)),
            "24M missing: {sizes:?}"
        );
        assert!(
            sizes.contains(&(48 * 1024 * 1024)),
            "48M missing: {sizes:?}"
        );
        assert!(
            sizes.contains(&(96 * 1024 * 1024)),
            "96M missing: {sizes:?}"
        );
        assert!(
            sizes.contains(&(128 * 1024 * 1024)),
            "128M missing: {sizes:?}"
        );
    }

    #[test]
    fn build_sizes_default_max_no_cap() {
        let cache = test_cache();
        // Default max = L3*2 = 24M
        let sizes = cache.build_sizes(cache.l3 as usize * 2);
        let last = *sizes.last().unwrap();
        assert_eq!(last, 24 * 1024 * 1024);
    }

    #[test]
    fn build_sizes_unlimited_no_cap_entry() {
        let cache = test_cache();
        let sizes = cache.build_sizes(usize::MAX);
        // Should NOT contain usize::MAX as a size entry
        assert!(
            !sizes.contains(&usize::MAX),
            "usize::MAX should not be a size"
        );
    }
}
