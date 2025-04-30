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

## Local Development Tasks (mise)

[mise](https://mise.jdx.dev/) manages tool versions and task
orchestration. Install with your package manager, then:

```sh
mise install        # install pinned tool versions
mise run ci         # full local CI gate (run before push)
mise run release:dry-run  # CI + crates.io publish dry-run
```

### Task Reference

| Task | CI Parity | What it does |
| ---- | --------- | ------------ |
| `mise run ci` | full tier 2 | fmt → clippy → deny → lockfiles → test |
| `mise run ci:lockfiles` | (drift guard) | Fails if `fuzz/Cargo.lock` is stale |
| `mise run release:dry-run` | + publish | `ci` + msrv + `cargo publish --dry-run` |
| `mise run fmt` | `fmt` job | `cargo fmt --check` |
| `mise run clippy` | `clippy` job | Lint with `-D warnings` |
| `mise run deny` | `deny` job | Advisory, license, ban, source checks |
| `mise run msrv` | `msrv` job | Build + test against MSRV (1.85) |
| `mise run test` | `test` job | `cargo nextest run` |
| `mise run test-cargo` | — | `cargo test` (standard runner) |
| `mise run coverage` | `diff-coverage` | tarpaulin with llvm engine |
| `mise run coverage:diff` | `diff-coverage` | Coverage + diff-cover vs main |
| `mise run bench` | — | Criterion run; regenerates BENCHMARKS.md |
| `mise run bench:quick` | — | Fast criterion pass (don't publish) |
| `mise run bench:callgrind` | — | Deterministic Ir counts (needs valgrind) |
| `mise run pgo` | — | Profile-guided optimization build |
| `mise run tools:update` | — | Check cargo-installed tools for updates |

### Pre-push Workflow

```sh
mise run ci                 # catches what GitHub CI would catch
```

For release branches:

```sh
mise run release:dry-run    # full CI + publish validation
```

### Version Bump Checklist

1. Edit `version` in `Cargo.toml`
2. Regenerate lockfiles:

   ```sh
   cargo update --workspace --manifest-path fuzz/Cargo.toml
   ```

3. Commit both `Cargo.lock` and `fuzz/Cargo.lock`
4. `mise run release:dry-run` to validate

`mise run ci:lockfiles` will fail if step 2 is forgotten —
this mirrors what CI would catch on the PR.

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

Two test runners serve different purposes:

| Context               | Runner     | Why                       |
| --------------------- | ---------- | ------------------------- |
| `mise run test`       | nextest    | Speed, process isolation  |
| `mise run test-cargo` | cargo test | Doctests, CI parity       |
| `mise run msrv`       | cargo test | Doctests on MSRV          |
| GitHub CI matrix      | cargo test | Zero-install, 5 platforms |

nextest cannot run doctests — `cargo test` is the only runner
that exercises `///` examples. The `msrv` task and GitHub CI
cover this. If you add a doctest, `mise run test` alone won't
catch compilation failures in it; use `mise run msrv` or
`mise run test-cargo`.

```sh
mise run test                        # nextest (fast, isolated)
mise run test-cargo                  # cargo test (doctests, CI parity)
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
