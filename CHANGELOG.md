# Changelog

## 0.7.0

### Breaking

`unicode-normalize` no longer enables `transform`.

The feature previously declared `unicode-normalize = ["transform"]`,
which pulled in `sgr_rewrite.rs` and `transform_stream.rs`. Both
reference `crate::downgrade` and `crate::palette` unconditionally, so
`--no-default-features --features unicode-normalize` failed to
compile — even though `unicode_map.rs` is alloc-only text
normalization with no dependency on transform, filter, or SGR/color
rewriting.

`unicode-normalize` is now `[]`. Crates that relied on it to enable
`transform` transitively — for example calling
`strip_ansi::sgr_rewrite` while declaring only
`features = ["unicode-normalize"]` — must now enable `transform`
explicitly:

```toml
strip-ansi = { package = "distill-strip-ansi", version = "0.7", features = [
    "unicode-normalize",
    "transform",
] }
```

`downgrade-color` and `augment-color` remain siblings under
`transform`, unchanged.

`cargo semver-checks` classifies this as
`feature_no_longer_enables_feature`, which is what makes 0.7.0 a
major bump rather than a patch.

### Internal (bench harness)

Not user-facing, but significant enough to record: the benchmark
harness had several correctness bugs found and fixed while preparing
this release's numbers.

- Cache detection clamped every reading up to a 12 MiB floor
  (`raw.l3.max(12_MiB)`), conflating "detection failed" with "this
  machine is smaller than the default." On an 8 MiB-L3 host this
  silently mis-sized the whole benchmark size ladder. Fixed to
  substitute the fallback only on an actual zero reading.
- Criterion sample-size sizing had two arithmetic bugs: predicting
  Linear-vs-Flat sampling mode ourselves (and sizing for the wrong one
  when our prediction disagreed with criterion's real choice), and
  budgeting `n` iterations for a Linear schedule that actually costs
  `n(n+1)/2`. Both produced the "Unable to complete N samples"
  warnings they were meant to prevent. Replaced with a time-boxed cost
  probe sized directly against Linear's real iteration count.
- The dispersion marker (`⚠`) reported `std_dev/mean`, which is
  inflated by the same outlier tail the median point estimate already
  rejects — it fired on nearly every cell. Now reports MAD/median,
  matching the robust statistic actually displayed.
- `doc/BENCHMARKS.md` generation now scopes reported sizes to the
  triggering run via a run-boundary marker, so a smaller `--max-size`
  no longer leaves stale larger-size rows mixed into the report.
- `mise run bench:callgrind` runs the iai-callgrind instruction-count
  pass concurrently, bounded by physical core count and available
  memory. Wall-clock criterion benches remain strictly serial — they
  share one L3 and one turbo budget, so concurrent runs would not be
  comparable to prior serial numbers.
- `mise run test` and `mise run test:harness` test the library and
  bench-harness packages separately and explicitly, both now with
  `--all-features` to match CI (previously ~135 feature-gated tests
  never ran locally).
