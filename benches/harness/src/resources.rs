//! Resource capture — RSS, peak RSS, CPU time per benchmark.
//!
//! Snapshots are taken OUTSIDE the timed loop (before first sample,
//! after last sample) so they don't affect measurement.

use std::collections::BTreeMap;
use std::io::Write;
use std::sync::Mutex;
use std::time::Instant;

use crate::cache::CacheInfo;

/// Per-benchmark resource snapshot.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Snapshot {
    pub rss_before: u64,
    pub rss_after: u64,
    pub rss_delta: i64,
    pub peak_rss: u64,
    pub cpu_user_us: u64,
    pub cpu_sys_us: u64,
}

/// Accumulates resource snapshots across a bench run.
pub struct ResourceTracker {
    data: Mutex<BTreeMap<(String, usize), Snapshot>>,
    start: Instant,
}

/// Named parameters for resource capture.
pub struct CapturePoint<'a> {
    pub crate_name: &'a str,
    pub size: usize,
}

impl ResourceTracker {
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: Mutex::new(BTreeMap::new()),
            start: Instant::now(),
        }
    }

    /// Capture RSS and CPU before a benchmark.
    pub fn before(&self, point: CapturePoint<'_>) {
        let rss = current_rss_bytes();
        let (user, sys) = cpu_times_us();
        let mut map = self.data.lock().unwrap();
        map.insert(
            (point.crate_name.to_string(), point.size),
            Snapshot {
                rss_before: rss,
                cpu_user_us: user,
                cpu_sys_us: sys,
                ..Default::default()
            },
        );
    }

    /// Capture RSS and CPU after a benchmark.
    pub fn after(&self, point: CapturePoint<'_>) {
        let rss = current_rss_bytes();
        let peak = peak_rss_bytes();
        let (user, sys) = cpu_times_us();
        let mut map = self.data.lock().unwrap();
        let key = (point.crate_name.to_string(), point.size);
        if let Some(snap) = map.get_mut(&key) {
            snap.rss_after = rss;
            snap.peak_rss = peak;
            snap.rss_delta = rss as i64 - snap.rss_before as i64;
            snap.cpu_user_us = user.saturating_sub(snap.cpu_user_us);
            snap.cpu_sys_us = sys.saturating_sub(snap.cpu_sys_us);
        }
    }

    /// Wall-clock seconds since tracker creation.
    #[must_use]
    pub fn elapsed_secs(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }
}

impl Default for ResourceTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Flush parameters — named to avoid positional confusion.
pub struct FlushParams<'a> {
    pub tracker: &'a ResourceTracker,
    pub cache: &'a CacheInfo,
    pub sizes: &'a [usize],
}

/// Write resource snapshots to `{target_dir}/criterion/bench-resources.json`.
pub fn flush_resources(params: FlushParams<'_>) {
    let map = params.tracker.data.lock().unwrap();
    let wall_secs = params.tracker.elapsed_secs();

    let target = std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".to_string());
    let dir = std::path::PathBuf::from(&target).join("criterion");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("bench-resources.json");

    // Group by crate name → size → snapshot.
    let mut nested: BTreeMap<&str, BTreeMap<usize, &Snapshot>> = BTreeMap::new();
    for ((name, size), snap) in map.iter() {
        nested.entry(name).or_default().insert(*size, snap);
    }

    #[derive(serde::Serialize)]
    struct Output<'a> {
        meta: Meta<'a>,
        crates: BTreeMap<&'a str, BTreeMap<usize, &'a Snapshot>>,
    }
    #[derive(serde::Serialize)]
    struct Meta<'a> {
        wall_secs: f64,
        cache_sizes: &'a CacheInfo,
        sizes_used: &'a [usize],
    }

    let out = Output {
        meta: Meta {
            wall_secs,
            cache_sizes: params.cache,
            sizes_used: params.sizes,
        },
        crates: nested,
    };

    // Read existing data BEFORE truncating, so each bench binary's
    // resources accumulate instead of overwriting previous runs.
    let existing: serde_json::Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let new_val = serde_json::to_value(&out).unwrap_or_default();
    let merged = merge_resources(existing, new_val);

    if let Ok(mut f) = std::fs::File::create(&path) {
        let _ = serde_json::to_writer_pretty(&mut f, &merged);
        let _ = writeln!(f);
    }

    let mins = (wall_secs / 60.0).floor() as u64;
    let secs = (wall_secs % 60.0) as u64;
    eprintln!("Resource snapshots → {} ({mins}m{secs}s)", path.display());
}

// ── OS-level resource queries ───────────────────────────────────

/// Merge new resource data into existing JSON, preserving data from
/// previous bench binary runs. Each binary only knows about its own
/// crate — merging accumulates all crates into one file.
fn merge_resources(
    mut existing: serde_json::Value,
    new: serde_json::Value,
) -> serde_json::Value {
    if let (Some(existing_crates), Some(new_crates)) = (
        existing
            .get_mut("crates")
            .and_then(|v| v.as_object_mut()),
        new.get("crates").and_then(|v| v.as_object()),
    ) {
        for (k, v) in new_crates {
            existing_crates.insert(k.clone(), v.clone());
        }
        // Update meta from latest run.
        if let Some(new_meta) = new.get("meta") {
            if let Some(obj) = existing.as_object_mut() {
                obj.insert("meta".to_string(), new_meta.clone());
            }
        }
        existing
    } else {
        // No existing crates — use new data as-is.
        new
    }
}

/// Current resident set size in bytes.
#[allow(unsafe_code)]
fn current_rss_bytes() -> u64 {
    #[cfg(target_os = "macos")]
    {
        use std::mem::{MaybeUninit, size_of};
        unsafe extern "C" {
            fn mach_task_self() -> u32;
        }
        let mut info = MaybeUninit::<libc::mach_task_basic_info_data_t>::uninit();
        let mut count = (size_of::<libc::mach_task_basic_info_data_t>()
            / size_of::<libc::natural_t>()) as libc::mach_msg_type_number_t;
        let kr = unsafe {
            libc::task_info(
                mach_task_self(),
                libc::MACH_TASK_BASIC_INFO,
                info.as_mut_ptr().cast(),
                &mut count,
            )
        };
        if kr == libc::KERN_SUCCESS {
            return unsafe { info.assume_init() }.resident_size;
        }
        0
    }
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/self/statm")
            .ok()
            .and_then(|s| s.split_whitespace().nth(1)?.parse::<u64>().ok())
            .map(|pages| pages * 4096)
            .unwrap_or(0)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        0
    }
}

/// Peak resident set size in bytes.
#[allow(unsafe_code)]
fn peak_rss_bytes() -> u64 {
    #[cfg(unix)]
    {
        let mut u = std::mem::MaybeUninit::<libc::rusage>::uninit();
        unsafe { libc::getrusage(libc::RUSAGE_SELF, u.as_mut_ptr()) };
        let u = unsafe { u.assume_init() };
        if cfg!(target_os = "macos") {
            u.ru_maxrss as u64
        } else {
            u.ru_maxrss as u64 * 1024
        }
    }
    #[cfg(not(unix))]
    {
        0
    }
}

/// CPU user and system time in microseconds.
#[allow(unsafe_code)]
fn cpu_times_us() -> (u64, u64) {
    #[cfg(unix)]
    {
        let mut u = std::mem::MaybeUninit::<libc::rusage>::uninit();
        unsafe { libc::getrusage(libc::RUSAGE_SELF, u.as_mut_ptr()) };
        let u = unsafe { u.assume_init() };
        let user = u.ru_utime.tv_sec as u64 * 1_000_000 + u.ru_utime.tv_usec as u64;
        let sys = u.ru_stime.tv_sec as u64 * 1_000_000 + u.ru_stime.tv_usec as u64;
        (user, sys)
    }
    #[cfg(not(unix))]
    {
        (0, 0)
    }
}

// ── Public accessors for bench binaries ─────────────────────────

/// Public wrapper for current RSS (for baseline logging).
pub fn current_rss_bytes_pub() -> u64 {
    current_rss_bytes()
}
