# Contributing

Thanks for helping with toktally. This repo is a Rust workspace: a shared
domain and store, a reporter/API, and thin host wrappers that exec the reporter.

Please read [docs/living-specs/token-usage.md](docs/living-specs/token-usage.md)
before changing ingest, merge, or harness identity rules.

## Development setup

You need a recent stable Rust toolchain (`rustc` / `cargo`). Then:

```bash
cargo test --workspace
cargo fmt --all
cargo clippy --workspace --all-targets
```

Do not set `HOME`, `CARGO_HOME`, or store paths to a throwaway directory that
the reporter would persist as user config.

## Project layout

| Path | Role |
|------|------|
| `crates/domain` | Pure types. No HTTP, no filesystem. |
| `crates/store` | Durable JSON store. Same `(harness, session_id)` updates in place. |
| `crates/adapters` | Translate harness JSON into domain observations. |
| `crates/cli` | `toktally-api` and `toktally`. |
| `plugins/<harness>` | Host-native hooks/manifests. Scripts only exec the reporter. |
| `scripts/` | `install.sh` and `update.sh` for prefix installs. |
| `crates/sync` | Walk existing host session stores; first-use ingest. |
| `docs/living-specs/` | Design intent and feature horizons. |

## How to add a harness

1. Add a `Harness` variant, slug, and aliases in `crates/domain`.
2. Add a representative fixture under `crates/adapters/fixtures/`.
3. Map that fixture in `crates/adapters` and assert through the real store
   (ingest, then read back the fixture's own counts).
4. Add `plugins/<slug>/scripts/report.sh` that runs
   `toktally ingest --adapter <slug>`.
   Do not parse token fields in the script.
5. Add a host-native manifest (`plugin.json`, `hooks.json`, `HOOK.yaml`,
   `gemini-extension.json`, …) that the host actually loads.
6. Update wrapper tests in `crates/cli/tests/wrappers.rs` and the living spec.

Tests must drive the shipped adapter and store. Do not re-implement parse or
merge logic inside the test, and do not hard-code totals that never passed
through the fixture.

## Pull requests

- One logical change per PR when you can. Prefer several small PRs over one
  mixed patch.
- Match existing commit style: imperative subject, one concern per commit.
- Link an issue when there is one (`Fixes #N`).
- Include tests for behavior changes.
- Host wrappers must keep calling `toktally`.

Open a draft PR early if you want design feedback on a new harness payload.

## Reporting issues

Use the issue templates under `.github/ISSUE_TEMPLATE/`. Security reports
belong in [SECURITY.md](SECURITY.md), not a public bug ticket.

## Code of conduct

This project follows [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
