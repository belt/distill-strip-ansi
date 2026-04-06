use criterion::{Criterion, criterion_group, criterion_main};
use distill_bench_harness::{StripBench, run_strip_bench};

fn bench(c: &mut Criterion) {
    run_strip_bench(
        c,
        &StripBench {
            crate_name: "strip-ansi-escapes",
            bench_id: "strip_ansi_escapes",
            strip_fn: |input| strip_ansi_escapes::strip(input),
        },
    );
}

criterion_group!(benches, bench);
criterion_main!(benches);
