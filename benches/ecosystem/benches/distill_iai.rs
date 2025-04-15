//! Instruction-count ecosystem bench for `distill-strip-ansi`.
//!
//! Host-independent cost measurement via Callgrind — the numbers
//! reported are deterministic counts of executed instructions and
//! cache references, comparable across machines and usable as inputs
//! to capacity planning (vCPU/cost budgeting, request-rate projections).
//!
//! Wall-clock throughput lives in the sibling `distill.rs` bench;
//! this file is the "how many cycles does one MiB cost" side.

use distill_bench_harness::{LARGE, MEDIUM, SMALL, TINY, XLARGE, iai_cargo, iai_input, iai_osc8};
use iai_callgrind::{library_benchmark, library_benchmark_group, main};
use std::hint::black_box;

fn strip(input: &[u8]) -> Vec<u8> {
    strip_ansi::strip(input).into_owned()
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
    name = distill;
    benchmarks = bench_dirty, bench_fixture
);

main!(library_benchmark_groups = distill);
