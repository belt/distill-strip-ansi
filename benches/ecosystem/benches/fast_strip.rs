use criterion::{Criterion, criterion_group, criterion_main};
use distill_bench_harness::{StripBench, run_strip_bench};

fn bench(c: &mut Criterion) {
    run_strip_bench(
        c,
        &StripBench {
            crate_name: "fast-strip-ansi",
            bench_id: "fast_strip",
            strip_fn: |input| fast_strip_ansi::strip_ansi_bytes(input).to_vec(),
        },
    );
}

criterion_group!(benches, bench);
criterion_main!(benches);
