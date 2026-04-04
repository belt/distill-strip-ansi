# Contributing

## Getting Started

Fork the repo and clone your fork. See
[GitHub's fork-and-pull guide](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/creating-a-pull-request-from-a-fork)
for the workflow.

```sh
cargo test --all-features
```

All tests must pass before submitting a PR.

## Threat Database

The tool ships with built-in threat patterns for known
ANSI echoback CVEs. These are compiled into the binary
and cannot be modified at runtime.

To add new threat patterns without recompiling:

1. Create a TOML file following `doc/threat-db.toml`
2. Use `--threat-db <path>` with `--check-threats`
3. External entries are additive only — built-in
   patterns cannot be overridden or removed

To propose a new built-in threat pattern:

1. Open an issue with the CVE or advisory reference
2. Include the sequence bytes and detection criteria
3. Add the entry to `builtin_entries()` in
   `src/threat_db.rs`
4. Add unit tests in the same file
5. Update `doc/threat-db.toml` with a commented example

See `doc/SECURITY.md` for the threat model and
`doc/CVE-MITIGATION.md` for the CVE matrix.

## Project Structure

```text
src/
  parser.rs       1-byte ECMA-48 state machine
  classifier.rs   12-byte ClassifyingParser (SeqDetail)
  filter.rs       FilterConfig + FilterStream
  preset.rs       Terminal presets (dumb..full)
  detect.rs       Auto-detection (terminal-detect)
  threat_db.rs    External threat database (toml-config)
  toml_config.rs  TOML config loading (toml-config)
  cli.rs          CLI argument definitions
  main.rs         CLI entry point
  strip.rs        Core strip functions
  stream.rs       StripStream
  writer.rs       StripWriter (std)
  io.rs           Output buffering
  lib.rs          Public API surface

doc/
  DESIGN.md          Architecture and data models
  SECURITY.md        Threat model and defenses
  PRESETS.md         Preset reference and gradient
  ANSI-REFERENCE.md  ECMA-48 sequence taxonomy
  ECOSYSTEM.md       Crate comparison
  threat-db.toml     Reference threat database

tests/
  integration_tests.rs          CLI end-to-end
  property_classifier_tests.rs  Proptest: classifier
  property_filter_tests.rs      Proptest: filter
  property_preset_tests.rs      Proptest: presets
  preset_unit_tests.rs          Preset unit tests
  snapshot_tests.rs             Insta snapshots
  ...
```

## Testing

```sh
cargo test --all-features            # all tests
cargo test --test integration_tests  # CLI only
cargo test --lib                     # unit tests only
```

Property tests use [proptest](https://crates.io/crates/proptest).
Failing cases are saved to `.proptest-regressions` files and
replayed on subsequent runs.

## Code Style

- `#![forbid(unsafe_code)]` — no exceptions
- Edition 2024, MSRV 1.85
- `cargo clippy --all-targets --all-features`
- `cargo fmt --check`

## Documentation

- `doc/` is for contributors and advanced users
- `README.md` is for first-time users and CI integrators
- Rust API examples belong in `doc/` not `README.md`
- Keep `README.md` focused on what/why/how, not code
