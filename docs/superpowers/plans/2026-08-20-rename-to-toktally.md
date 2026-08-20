# Rename Project from `token-usage` to `toktally` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Completely rename the workspace, packages, binaries, environment variables, documentation, and release workflows from `token-usage` to `toktally`.

**Architecture:** Systematic rename across Cargo workspace definitions, internal module imports, binary entry points (`toktally` and `toktally-api`), environment variables with backwards-compatible fallbacks, plugin wrappers, shell scripts, embed JavaScript, and GitHub Actions workflows.

**Tech Stack:** Rust (Cargo), Shell, JavaScript, GitHub Actions.

## Global Constraints

- All 5 crates renamed: `toktally-domain`, `toktally-store`, `toktally-adapters`, `toktally-sync`, `toktally-cli`.
- Primary binaries: `toktally` and `toktally-api`.
- Default config/store directory: `~/.toktally/`.
- Environment variables: `TOKTALLY_*` (supporting legacy `TOKEN_USAGE_*` as fallback).
- Project tagline: *"Track token usage across all your AI coding agents."*

---

### Task 1: Rename Cargo Workspace, Package Manifests, and Crate Imports

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/domain/Cargo.toml`
- Modify: `crates/store/Cargo.toml`
- Modify: `crates/adapters/Cargo.toml`
- Modify: `crates/sync/Cargo.toml`
- Modify: `crates/cli/Cargo.toml`
- Rename: `crates/cli/src/bin/token-usage-reporter.rs` → `crates/cli/src/bin/toktally.rs`
- Rename: `crates/cli/src/bin/token-usage-api.rs` → `crates/cli/src/bin/toktally-api.rs`
- Modify: `crates/*/src/lib.rs` and `crates/*/tests/*.rs`

- [ ] **Step 1: Update Cargo.toml definitions**
- [ ] **Step 2: Rename bin source files**
- [ ] **Step 3: Update Rust crate import statements across tests and source files**
- [ ] **Step 4: Verify workspace builds and passes cargo check**
- [ ] **Step 5: Commit changes**

---

### Task 2: Update Environment Variables, Config Paths, and Embeds

**Files:**
- Modify: `crates/cli/src/bin/toktally.rs`
- Modify: `crates/cli/src/bin/toktally-api.rs`
- Modify: `crates/cli/src/components.rs`
- Modify: `crates/cli/src/pricing.rs`
- Modify: `crates/cli/src/publish.rs`
- Modify: `crates/sync/src/lib.rs`
- Modify: `embed/usage-card.js`

- [ ] **Step 1: Update environment variable lookups to TOKTALLY_* with fallback to TOKEN_USAGE_***
- [ ] **Step 2: Update default paths to ~/.toktally/**
- [ ] **Step 3: Update embed script with toktallyCard global and .toktally-card selector**
- [ ] **Step 4: Verify test suite passes**
- [ ] **Step 5: Commit changes**

---

### Task 3: Update Plugin Wrappers, Installer, and Updater Scripts

**Files:**
- Modify: `plugins/*/scripts/report.sh`
- Modify: `scripts/install.sh`
- Modify: `scripts/update.sh`

- [ ] **Step 1: Update plugins report.sh scripts to invoke toktally**
- [ ] **Step 2: Update install.sh and update.sh for toktally**
- [ ] **Step 3: Verify installer tests pass on Unix**
- [ ] **Step 4: Commit changes**

---

### Task 4: Update Workflows, README, Documentation, and Living Specs

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `.github/workflows/release.yml`
- Modify: `README.md`
- Modify: `CONTRIBUTING.md`
- Modify: `SECURITY.md`
- Modify: `docs/living-specs/token-usage.md`

- [ ] **Step 1: Update release.yml artifact names and binary paths to toktally**
- [ ] **Step 2: Update README.md with punchy description, new repo links, and toktally commands**
- [ ] **Step 3: Update contributing, security, and specs**
- [ ] **Step 4: Commit changes**

---

### Task 5: Final Validation and Smoke Test

- [ ] **Step 1: Run full pre-flight checks: fmt, clippy, and test**
- [ ] **Step 2: Verify binary compilation and --help output for toktally**
