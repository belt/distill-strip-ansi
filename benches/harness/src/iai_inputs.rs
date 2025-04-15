//! Fixed workload set for instruction-count benchmarks.
//!
//! Criterion benches size-adapt to the host cache hierarchy so wall-clock
//! numbers reflect memory-bandwidth tiers. Instruction counts are *the
//! opposite*: host-independent currency for capacity planning (vCPU
//! budgeting, cost-per-MB modeling). Hardware-adaptive sizing would
//! defeat that premise — one run on a Haswell workstation would report
//! different sizes than the same run in EC2, breaking cross-host
//! comparison.
//!
//! So this module hands out a *fixed* size ladder covering four decades
//! (256 B → 16 MiB) plus cargo + OSC-8 fixtures. Every iai bench binary
//! in the workspace uses the same set; the generated table is directly
//! comparable across crates.
//!
//! Sizes chosen for instruction-count significance, not cache effects:
//!
//! - `TINY` (256 B)    — per-line overhead on small stdin chunks
//! - `SMALL` (4 KiB)   — typical CI log line group, one page
//! - `MEDIUM` (64 KiB) — moderate cargo output slice
//! - `LARGE` (1 MiB)   — substantial cargo log
//! - `XLARGE` (16 MiB) — container build output / noisy CI run

use crate::dirty_input;

pub const TINY: usize = 256;
pub const SMALL: usize = 4 * 1024;
pub const MEDIUM: usize = 64 * 1024;
pub const LARGE: usize = 1024 * 1024;
pub const XLARGE: usize = 16 * 1024 * 1024;

/// The fixed size ladder used by every iai-callgrind bench.
pub const IAI_SIZES: &[usize] = &[TINY, SMALL, MEDIUM, LARGE, XLARGE];

/// Generate synthetic dirty input at one of the iai size tiers.
///
/// Thin wrapper around [`dirty_input`] — keeps the iai benches decoupled
/// from the input generator so the distribution can evolve (e.g. weight
/// OSC 8 more heavily) in one place.
#[must_use]
pub fn iai_input(size: usize) -> Vec<u8> {
    dirty_input(size)
}

/// Cargo-style fixture reused across all iai benches.
///
/// Byte-identical to the `real_world_cargo` fixture in
/// `benches/internals.rs` — kept in sync so internals and ecosystem
/// iai benches measure the same workload.
#[must_use]
pub fn iai_cargo() -> Vec<u8> {
    let mut v = Vec::new();
    for _ in 0..100 {
        v.extend_from_slice(b"\x1b[0m\x1b[1m\x1b[32m   Compiling\x1b[0m memchr v2.7.1\n");
    }
    v
}

/// OSC 8 hyperlink fixture reused across all iai benches.
#[must_use]
pub fn iai_osc8() -> Vec<u8> {
    let mut v = Vec::new();
    for _ in 0..50 {
        v.extend_from_slice(b"\x1b]8;;https://docs.rs/memchr/2.7.1\x07memchr\x1b]8;;\x07 v2.7.1\n");
    }
    v
}
