# CI Matrix & CalVer GitHub Releases Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish a robust multi-platform CI verification workflow and automated CalVer GitHub Releases producing 6 target binaries (`token-usage-reporter` and `token-usage-api`) for Linux, macOS, and Windows on x86_64 and ARM64.

**Architecture:** `.github/workflows/ci.yml` provides PR/push test coverage across Linux, macOS, and Windows. `.github/workflows/release.yml` implements a 3-stage pipeline: compute CalVer tag (`vYYYY.M.D.run`), compile and package 6 target matrix archives, and collate checksums into `SHA256SUMS.txt` attached to a single GitHub Release.

**Tech Stack:** GitHub Actions, Rust (Cargo), `cargo-zigbuild`, `cross`, `softprops/action-gh-release`.

## Global Constraints

- CalVer format must be `vYYYY.M.D.<run_number>` computed in UTC.
- Exactly 6 target binary packages: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc`.
- Each package contains both `token-usage-reporter` and `token-usage-api` binaries, `README.md`, and `LICENSE`.
- Formats: `.tar.gz` for Linux and macOS targets, `.zip` for Windows targets.
- Single SHA-256 manifest `SHA256SUMS.txt` created and uploaded with every release.

---

### Task 1: Enhance `.github/workflows/ci.yml` for Multi-OS Matrix Testing

**Files:**
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: Workspace root `Cargo.toml` and existing test suites.
- Produces: GitHub Actions CI workflow executing format/clippy and multi-OS test matrix (`ubuntu-latest`, `macos-latest`, `windows-latest`).

- [ ] **Step 1: Update `.github/workflows/ci.yml`**

Write the updated CI workflow with separate linting and multi-platform testing jobs:

```yaml
name: CI

on:
  push:
    branches: [main, master]
  pull_request:
    branches: [main, master]

env:
  CARGO_TERM_COLOR: always

jobs:
  lint:
    name: Lint & Formatting
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - uses: Swatinem/rust-cache@v2

      - name: Check formatting
        run: cargo fmt --all --check

      - name: Clippy
        run: cargo clippy --workspace --all-targets --all-features -- -D warnings

  test:
    name: Test (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - uses: Swatinem/rust-cache@v2
        with:
          key: ${{ matrix.os }}

      - name: Run tests
        run: cargo test --workspace --all-features
```

- [ ] **Step 2: Verify workflow file format**

Check YAML structure locally.

- [ ] **Step 3: Commit changes**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add multi-os matrix testing and clippy verification"
```

---

### Task 2: Create CalVer Release Workflow `.github/workflows/release.yml`

**Files:**
- Create: `.github/workflows/release.yml`

**Interfaces:**
- Consumes: Trigger via `workflow_dispatch`.
- Produces: 6 target archives and `SHA256SUMS.txt` uploaded to a GitHub Release tagged `vYYYY.M.D.<run>`.

- [ ] **Step 1: Create `.github/workflows/release.yml`**

Implement the 3-stage release workflow:

```yaml
name: Release

on:
  workflow_dispatch:

permissions:
  contents: write

env:
  CARGO_TERM_COLOR: always

jobs:
  compute-version:
    name: Compute CalVer Version
    runs-on: ubuntu-latest
    outputs:
      tag: ${{ steps.calver.outputs.tag }}
      version: ${{ steps.calver.outputs.version }}
    steps:
      - name: Calculate CalVer
        id: calver
        run: |
          YEAR=$(date -u +'%Y')
          MONTH=$(date -u +'%-m')
          DAY=$(date -u +'%-d')
          RUN_NUM=${{ github.run_number }}
          TAG="v${YEAR}.${MONTH}.${DAY}.${RUN_NUM}"
          VERSION="${YEAR}.${MONTH}.${DAY}+${RUN_NUM}"
          echo "tag=${TAG}" >> "$GITHUB_OUTPUT"
          echo "version=${VERSION}" >> "$GITHUB_OUTPUT"
          echo "Computed release tag: ${TAG}"

  build-matrix:
    name: Build (${{ matrix.target }})
    needs: compute-version
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
            use_cross: false
            archive_format: tar.gz
          - target: aarch64-unknown-linux-gnu
            os: ubuntu-latest
            use_cross: true
            archive_format: tar.gz
          - target: x86_64-apple-darwin
            os: macos-13
            use_cross: false
            archive_format: tar.gz
          - target: aarch64-apple-darwin
            os: macos-latest
            use_cross: false
            archive_format: tar.gz
          - target: x86_64-pc-windows-msvc
            os: windows-latest
            use_cross: false
            archive_format: zip
          - target: aarch64-pc-windows-msvc
            os: windows-latest
            use_cross: false
            archive_format: zip

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Install cross-compilation tools
        if: matrix.use_cross
        run: |
          cargo install cross --git https://github.com/cross-rs/cross

      - uses: Swatinem/rust-cache@v2
        with:
          key: ${{ matrix.target }}

      - name: Build release binaries
        run: |
          if [ "${{ matrix.use_cross }}" = "true" ]; then
            cross build --release --target ${{ matrix.target }} -p token-usage-cli --bins
          else
            cargo build --release --target ${{ matrix.target }} -p token-usage-cli --bins
          fi
        shell: bash

      - name: Package artifacts (Unix)
        if: matrix.archive_format == 'tar.gz'
        shell: bash
        run: |
          TAG="${{ needs: compute-version.outputs.tag }}"
          PKG_NAME="token-usage-${TAG}-${{ matrix.target }}"
          mkdir -p "${PKG_NAME}"
          cp "target/${{ matrix.target }}/release/token-usage-reporter" "${PKG_NAME}/"
          cp "target/${{ matrix.target }}/release/token-usage-api" "${PKG_NAME}/"
          cp README.md LICENSE "${PKG_NAME}/"
          tar -czf "${PKG_NAME}.tar.gz" "${PKG_NAME}"
          echo "ARCHIVE_PATH=${PKG_NAME}.tar.gz" >> "$GITHUB_ENV"

      - name: Package artifacts (Windows)
        if: matrix.archive_format == 'zip'
        shell: bash
        run: |
          TAG="${{ needs: compute-version.outputs.tag }}"
          PKG_NAME="token-usage-${TAG}-${{ matrix.target }}"
          mkdir -p "${PKG_NAME}"
          cp "target/${{ matrix.target }}/release/token-usage-reporter.exe" "${PKG_NAME}/"
          cp "target/${{ matrix.target }}/release/token-usage-api.exe" "${PKG_NAME}/"
          cp README.md LICENSE "${PKG_NAME}/"
          7z a "${PKG_NAME}.zip" "${PKG_NAME}"
          echo "ARCHIVE_PATH=${PKG_NAME}.zip" >> "$GITHUB_ENV"

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.target }}
          path: ${{ env.ARCHIVE_PATH }}
          if-no-files-found: error

  publish:
    name: Publish GitHub Release
    needs: [compute-version, build-matrix]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Download all built archives
        uses: actions/download-artifact@v4
        with:
          path: release-assets
          merge-multiple: true

      - name: Generate SHA256SUMS.txt
        working-directory: release-assets
        run: |
          sha256sum * > SHA256SUMS.txt
          cat SHA256SUMS.txt

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: ${{ needs: compute-version.outputs.tag }}
          name: ${{ needs: compute-version.outputs.tag }}
          draft: false
          prerelease: false
          files: |
            release-assets/*
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

- [ ] **Step 2: Commit workflow**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add automated CalVer release workflow for 6 target platforms"
```

---

### Task 3: Workflow Validation and Smoke Test Script

**Files:**
- Test: Local verification of archive creation script and YAML syntax.

- [ ] **Step 1: Verify packaging logic locally**

Run a local release build and test packaging script to ensure directory layout and binary paths resolve cleanly.

```bash
cargo build --release -p token-usage-cli --bins
mkdir -p target/package-smoke-test
cp target/release/token-usage-reporter target/package-smoke-test/
cp target/release/token-usage-api target/package-smoke-test/
cp README.md LICENSE target/package-smoke-test/
tar -czf target/package-smoke-test.tar.gz -C target package-smoke-test
tar -tf target/package-smoke-test.tar.gz
rm -rf target/package-smoke-test target/package-smoke-test.tar.gz
```

- [ ] **Step 2: Verify git tree and run pre-flight checks**

Run: `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: PASS
