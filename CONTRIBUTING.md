# Contributing to distill-strip-ansi

Thank you for your interest in contributing. This project welcomes
contributions from everyone who respects our
[Code of Conduct](CODE_OF_CONDUCT.md).

## Quick Start

1. Fork the repo and clone your fork
2. Run the test suite to confirm a clean baseline:

   ```sh
   cargo test --all-features
   ```

3. Create a feature branch from `main`
4. Make your changes
5. Run the full validation suite:

   ```sh
   cargo fmt --check
   cargo clippy --all-targets --all-features
   cargo test --all-features
   ```

6. Open a pull request against `main`

## Detailed Guide

See [doc/CONTRIBUTING.md](doc/CONTRIBUTING.md) for the full
contributor guide covering:

- Threat database contributions
- Local CI validation with `act`
- Testing strategy (unit, integration, property tests)
- Code style and `unsafe` policy
- Documentation conventions

## Reporting Bugs

Use the [Bug Report](https://github.com/belt/distill-strip-ansi/issues/new?template=bug_report.yml)
issue template. Include:

- The version you are running (`strip-ansi --version`)
- Steps to reproduce
- Expected vs actual behavior
- Input that triggers the bug (if possible)

## Requesting Features

Use the [Feature Request](https://github.com/belt/distill-strip-ansi/issues/new?template=feature_request.yml)
issue template. Describe the use case and why existing
functionality does not cover it.

## Security Vulnerabilities

Do **not** open a public issue for security vulnerabilities.
See [SECURITY.md](SECURITY.md) for responsible disclosure
instructions.

## License

By contributing, you agree that your contributions will be
licensed under the same terms as the project:
[MIT OR Apache-2.0](LICENSE-MIT).
