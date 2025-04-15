//! Instruction-count ecosystem bench for `console`.
//!
//! `console::strip_ansi_codes` takes `&str` — the UTF-8 conversion
//! stays inside the benched closure here, matching how real callers
//! invoke it (callers almost never have pre-validated UTF-8 on hand).
//! The criterion sibling bench does the same, so both measurements
//! describe apples-to-apples cost.

use distill_bench_harness::{LARGE, MEDIUM, SMALL, TINY, XLARGE, iai_cargo, iai_input, iai_osc8};
use iai_callgrind::{library_benchmark, library_benchmark_group, main};
use std::hint::black_box;

fn strip(input: &[u8]) -> Vec<u8> {
    let s = String::from_utf8_lossy(input);
    console::strip_ansi_codes(&s).into_owned().into_bytes()
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
    name = console_strip;
    benchmarks = bench_dirty, bench_fixture
);

main!(library_benchmark_groups = console_strip);
