# Project Rename: `token-usage` → `toktally` Design

## 1. Overview

Rename the project, crates, binaries, environment variables, documentation, and workflows from `token-usage` to **`toktally`**.

**Project Tagline / Description**: *"Track token usage across all your AI coding agents."*

## 2. Workspace & Crate Mapping

| Component | Old Identifier | New Identifier |
| :--- | :--- | :--- |
| **Workspace Name** | `token-usage` | `toktally` |
| **Domain Crate** | `token-usage-domain` | `toktally-domain` (`toktally_domain`) |
| **Store Crate** | `token-usage-store` | `toktally-store` (`toktally_store`) |
| **Adapters Crate** | `token-usage-adapters` | `toktally-adapters` (`toktally_adapters`) |
| **Sync Crate** | `token-usage-sync` | `toktally-sync` (`toktally_sync`) |
| **CLI Crate** | `token-usage-cli` | `toktally-cli` (`toktally_cli`) |

## 3. Binaries & Commands

- Primary CLI: `toktally` (formerly `token-usage-reporter`)
  - Subcommands: `ingest`, `publish`, `pull`, `export`, `import`, `sync`, `list`, `get`
- HTTP Server: `toktally-api` (formerly `token-usage-api`)

## 4. Configuration & Environment Variables

- Default store file: `~/.toktally/store.json`
- Default prices file: `~/.toktally/prices.json`
- Default github config: `~/.toktally/github.json`
- Share path: `$PREFIX/share/toktally`
- Environment Variables (with backwards-compatible fallbacks):
  - `TOKTALLY_STORE` (fallback: `TOKEN_USAGE_STORE`)
  - `TOKTALLY_BIND` (fallback: `TOKEN_USAGE_BIND`)
  - `TOKTALLY_STATELESS` (fallback: `TOKEN_USAGE_STATELESS`)
  - `TOKTALLY_HARNESS_HOME` (fallback: `TOKEN_USAGE_HARNESS_HOME`)
  - `TOKTALLY_PRICES`, `TOKTALLY_PRICES_FETCH`, `TOKTALLY_PRICES_URL`
  - `TOKTALLY_REPORTER`
  - `TOKTALLY_GH`

## 5. Plugins, Embeds & Packaging

- **Host Wrappers (`plugins/*/scripts/report.sh`)**: Updated to execute `toktally ingest --adapter <harness>`.
- **Embeds**:
  - HTML Class: `.toktally-card` (and `.token-usage-card` for backwards compatibility).
  - Global JS: `window.toktallyCard`.
- **Install & Update Scripts**:
  - `scripts/install.sh`, `scripts/update.sh`
- **Release Assets**:
  - `toktally-${TAG}-${TARGET}.tar.gz`
  - `toktally-${TAG}-${TARGET}.zip`
  - `SHA256SUMS.txt`
