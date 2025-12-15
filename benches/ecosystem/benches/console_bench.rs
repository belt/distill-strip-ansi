use criterion::{criterion_group, criterion_main, Criterion};
use distill_bench_harness::{StripBench, run_strip_bench};

fn bench(c: &mut Criterion) {
    run_strip_bench(c, &StripBench {
        crate_name: "console",
        bench_id: "console",
        strip_fn: |input| {
            let s = String::from_utf8_lossy(input);
            console::strip_ansi_codes(&s).into_owned().into_bytes()
        },
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
