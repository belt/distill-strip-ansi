//! Instruction-count ecosystem bench for `fast-strip-ansi`.
//!
//! See `distill_iai.rs` for the rationale; this file mirrors that
//! structure so the generated iai tables line up column-for-column
//! across crates.

use distill_bench_harness::{LARGE, MEDIUM, SMALL, TINY, XLARGE, iai_cargo, iai_input, iai_osc8};
use iai_callgrind::{library_benchmark, library_benchmark_group, main};
use std::hint::black_box;

fn strip(input: &[u8]) -> Vec<u8> {
    fast_strip_ansi::strip_ansi_bytes(input).to_vec()
}

#[library_benchmark]
#[bench::tiny(iai_input(TINY))]
#[bench::small(iai_input(SMALL))]
#[bench::medium(iai_input(MEDIUM))]
#[bench::large(iai_input(LARGE))]
#[bench::xlarge(iai_input(XLARGE))]
fn bench_dirty(input: Vec<u8>) -> Vec<u8> {
    strip(black_box(&input))
}

#[library_benchmark]
#[bench::cargo(iai_cargo())]
#[bench::osc8(iai_osc8())]
fn bench_fixture(input: Vec<u8>) -> Vec<u8> {
    strip(black_box(&input))
}

library_benchmark_group!(
    name = fast_strip;
    benchmarks = bench_dirty, bench_fixture
);

main!(library_benchmark_groups = fast_strip);
