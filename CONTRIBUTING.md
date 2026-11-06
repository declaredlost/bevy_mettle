# Contributing

Thank you for your interest in contributing to `bevy_mettle`!

## Getting Started

### Prerequisites

- Rust 1.99 or later, and a nightly toolchain for formatting
- Git
- Familiarity with Bevy and its ECS
- [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) and
  [cargo-machete](https://github.com/bnjbvr/cargo-machete), for the security
  and dependency checks

```bash
rustup toolchain install nightly --component rustfmt
cargo install cargo-deny cargo-machete --locked
```

Clone the repository:

```bash
git clone https://github.com/declaredlost/bevy_mettle.git
cd bevy_mettle
```

### Checks

These commands mirror the CI checks:

```bash
# Check
cargo check --workspace --all-targets

# Fix the formatting with nightly rustfmt
cargo +nightly fmt --all

# Clippy
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Tests
cargo test --workspace --all-features

# Unused dependencies
cargo machete

# Check advisories, licenses and sources with cargo-deny
cargo deny check all
```

## Making Changes

1. Create a new branch for your changes:
   ```bash
   git checkout -b feat/your-feature-name
   ```

2. Make your changes following the [code style](#code-style)

3. Add tests for new behaviour

4. Ensure all checks pass

## Code Style

This project follows the standard Rust style guidelines:

- Format with nightly rustfmt (`cargo +nightly fmt --all`), which applies the
  import grouping in `rustfmt.toml`
- Ensure `cargo clippy` passes with no warnings; `clippy::pedantic` is denied
- Lint exceptions are `#[expect(..., reason = "...")]`, never `#[allow]`
- Add documentation comments (`///`) for every item; `missing_docs` is denied

## Submitting Changes

1. Commit with [Conventional Commits](https://www.conventionalcommits.org)
   messages, matching the repository labels:
   ```bash
   git commit -m "feat: add the thing"
   ```

2. Push your branch and open a Pull Request with:
   - A clear title describing the change
   - What changed and why
   - Related issues

### Pull Request Guidelines

- Keep PRs focused on a single feature or fix
- Update CHANGELOG.md with your changes
- Ensure CI passes (format, clippy, tests, machete, deny)
- Label the PR with its type

## License

By contributing, you agree that your contributions are dual licensed under
the MIT and Apache 2.0 licenses, as the rest of the project is, without any
additional terms or conditions.
