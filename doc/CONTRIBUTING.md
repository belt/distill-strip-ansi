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

## Local validation with act

`act` can be useful for local workflow validation, but it is not a perfect replacement for GitHub-hosted runners.

Best practice:

- Run the Linux matrix entries locally with `act`.
- Use a real GitHub token for actions that access the network or external actions.
- Prefer non-root container execution to better emulate GitHub runner user behavior.
- Do not treat `act` as authoritative for macOS or Windows workflows; those should be verified on GitHub-hosted runners.

Example commands:

```sh
unset GITHUB_TOKEN
act -s GITHUB_TOKEN="$(gh auth token)" --matrix "name:alpine (musl)" -P windows-latest=ubuntu:latest -P macos-latest=ubuntu:latest -j test
act -s GITHUB_TOKEN="$(gh auth token)" --matrix "name:debian (glibc)" -P windows-latest=ubuntu:latest -P macos-latest=ubuntu:latest -j test
act -s GITHUB_TOKEN="$(gh auth token)" --matrix "name:ubuntu (native)" -P windows-latest=ubuntu:latest -P macos-latest=ubuntu:latest -j test
```

If you want better parity with GitHub Actions user permissions:

```sh
act -s GITHUB_TOKEN="$(gh auth token)" --container-options '--user=1000:1000' --matrix "name:debian (glibc)" -P windows-latest=ubuntu:latest -P macos-latest=ubuntu:latest -j test
```

`act` still runs workflow containers under Docker and may show root-owned paths such as `/root/.cargo/bin`. That is an `act`-specific behavior and not a reason to assume GitHub-hosted runners use root.

For architecture, design, and module responsibilities, link to the source-of-truth rather than duplicating it here: see `doc/DESIGN.md`.

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
