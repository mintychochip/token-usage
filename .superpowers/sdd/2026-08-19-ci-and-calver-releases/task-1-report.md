# Task 1 Report: Multi-OS CI Matrix Testing

## Changes

Updated `.github/workflows/ci.yml` with two distinct jobs:

- `lint` runs on `ubuntu-latest`, installs Rust with `rustfmt` and `clippy`, uses `Swatinem/rust-cache@v2`, checks formatting with `cargo fmt --all --check`, and runs `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- `test` runs against the matrix `[ubuntu-latest, macos-latest, windows-latest]`, configures `Swatinem/rust-cache@v2` in each matrix job, and runs `cargo test --workspace --all-features`.

## Verification

- YAML parsing: passed using Python `yaml.safe_load`.
- `cargo fmt --check`: passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: passed.
- `cargo test --workspace --all-features`: passed (80 tests across 25 suites).

## Commit

Committed as:

`ci: add multi-os matrix testing and clippy verification`
