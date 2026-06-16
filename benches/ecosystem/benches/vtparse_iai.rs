//! Instruction-count ecosystem bench for `vtparse`.
//!
//! Mirrors the criterion bench (`vtparse.rs`) so the iai tables line
//! up column-for-column with the other crates. See `distill_iai.rs`
//! for the rationale.

use distill_bench_harness::{LARGE, MEDIUM, SMALL, TINY, XLARGE, iai_cargo, iai_input, iai_osc8};
use iai_callgrind::{library_benchmark, library_benchmark_group, main};
use std::hint::black_box;
use vtparse::{CsiParam, VTActor, VTParser};

struct StripActor {
    out: Vec<u8>,
}

impl VTActor for StripActor {
    fn print(&mut self, b: char) {
        let mut buf = [0u8; 4];
        self.out.extend_from_slice(b.encode_utf8(&mut buf).as_bytes());
    }
    fn execute_c0_or_c1(&mut self, control: u8) {
        self.out.push(control);
    }
    fn dcs_hook(&mut self, _: u8, _: &[i64], _: &[u8], _: bool) {}
    fn dcs_put(&mut self, _: u8) {}
    fn dcs_unhook(&mut self) {}
    fn esc_dispatch(&mut self, _: &[i64], _: &[u8], _: bool, _: u8) {}
    fn csi_dispatch(&mut self, _: &[CsiParam], _: bool, _: u8) {}
    fn osc_dispatch(&mut self, _: &[&[u8]]) {}
    fn apc_dispatch(&mut self, _: Vec<u8>) {}
}

fn strip(input: &[u8]) -> Vec<u8> {
    let mut actor = StripActor {
        out: Vec::with_capacity(input.len()),
    };
    let mut parser = VTParser::new();
    parser.parse(input, &mut actor);
    actor.out
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
    name = vtparse_strip;
    benchmarks = bench_dirty, bench_fixture
);

main!(library_benchmark_groups = vtparse_strip);
