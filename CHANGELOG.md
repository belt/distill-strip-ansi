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
