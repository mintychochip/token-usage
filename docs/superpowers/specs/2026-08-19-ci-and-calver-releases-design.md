# CI Matrix & CalVer GitHub Releases Design

## 1. Overview

Provide comprehensive cross-platform CI verification and automated CalVer binary releases on GitHub Actions for the `token-usage` Rust workspace.

Releases produce 6 platform-native archives containing both `token-usage-reporter` and `token-usage-api` binaries, alongside license/documentation and an accompanying `SHA256SUMS.txt` checksum manifest.

## 2. CalVer Versioning Scheme

- **Tag format**: `vYYYY.M.D.<run_number>` (computed in UTC). Example: `v2026.8.19.1`.
- **Cargo version / metadata**: Injected or aligned during build as `YYYY.M.D+<run_number>`.
- **Trigger**: GitHub Actions `workflow_dispatch` (manual one-click trigger).

## 3. Platform & Target Matrix

The release workflow builds and packages 6 distinct target architectures:

| Platform | Target Triple | Runner Environment | Build Tool | Archive Format |
| :--- | :--- | :--- | :--- | :--- |
| **Linux x86_64** | `x86_64-unknown-linux-gnu` | `ubuntu-latest` | `cargo build --release` | `.tar.gz` |
| **Linux ARM64** | `aarch64-unknown-linux-gnu` | `ubuntu-latest` | `cargo-zigbuild` / `cross` | `.tar.gz` |
| **macOS Intel** | `x86_64-apple-darwin` | `macos-13` (or `macos-latest` cross) | `cargo build --release --target x86_64-apple-darwin` | `.tar.gz` |
| **macOS Apple Silicon** | `aarch64-apple-darwin` | `macos-latest` (ARM64 native) | `cargo build --release --target aarch64-apple-darwin` | `.tar.gz` |
| **Windows x86_64** | `x86_64-pc-windows-msvc` | `windows-latest` | `cargo build --release --target x86_64-pc-windows-msvc` | `.zip` |
| **Windows ARM64** | `aarch64-pc-windows-msvc` | `windows-latest` | `cargo build --release --target aarch64-pc-windows-msvc` | `.zip` |

## 4. Archive Layout & Asset Naming

Each archive is named `token-usage-<TAG>-<TARGET>.(tar.gz|zip)` and contains:
- `token-usage-reporter` (`.exe` on Windows)
- `token-usage-api` (`.exe` on Windows)
- `README.md`
- `LICENSE`

The collator job downloads all 6 built archives, computes SHA-256 digests into `SHA256SUMS.txt`, creates the GitHub release for `<TAG>`, and attaches all 6 archives plus `SHA256SUMS.txt`.

## 5. Workflows Specification

### 5.1 CI Workflow (`.github/workflows/ci.yml`)
- **Triggers**: `push` and `pull_request` on branches `main` and `master`.
- **Jobs**:
  - `lint`: Runs `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` on `ubuntu-latest`.
  - `test`: Matrix build across `ubuntu-latest`, `macos-latest`, and `windows-latest` running `cargo test --workspace`.

### 5.2 Release Workflow (`.github/workflows/release.yml`)
- **Triggers**: `workflow_dispatch`.
- **Jobs**:
  1. `calculate-version`: Generates `tag` (e.g. `v2026.8.19.1`) from UTC date and GitHub run number.
  2. `build-matrix`: Matrix of 6 targets; compiles release binaries, stages archive contents, creates compressed package, and uploads as intermediate workflow artifact.
  3. `publish-release`: Downloads all 6 staged archives, calculates `SHA256SUMS.txt`, tags the commit, creates GitHub release, and uploads all assets.
