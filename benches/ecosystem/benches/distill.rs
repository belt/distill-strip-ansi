use criterion::{Criterion, criterion_group, criterion_main};
use distill_bench_harness::{StripBench, run_strip_bench};

fn bench(c: &mut Criterion) {
    run_strip_bench(
        c,
        &StripBench {
            crate_name: "distill-strip-ansi",
            bench_id: "distill",
            strip_fn: |input| strip_ansi::strip(input).into_owned(),
        },
    );
}

criterion_group!(benches, bench);
criterion_main!(benches);
