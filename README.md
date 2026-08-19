# token-usage

[![CI](https://github.com/mintychochip/token-usage/actions/workflows/ci.yml/badge.svg)](https://github.com/mintychochip/token-usage/actions/workflows/ci.yml)

Cross-harness token-usage store. Plugins map whatever a coding agent emits — a
session hook, a global `/usage` snapshot, or a partial session fragment — into
one observation. A Rust API persists it so a later read returns the same
totals for the same harness/session identity.

Repository: [github.com/mintychochip/token-usage](https://github.com/mintychochip/token-usage)

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/mintychochip/token-usage/master/scripts/install.sh | sh
```

From a checkout:

```bash
git clone https://github.com/mintychochip/token-usage.git
cd token-usage
./scripts/install.sh          # PREFIX=$HOME/.local by default
./scripts/update.sh           # git pull --ff-only, then reinstall
```

This puts `token-usage-reporter` and `token-usage-api` in `$PREFIX/bin` and copies host wrappers to `$PREFIX/share/token-usage/plugins`. Cargo is required for a source build.

## Quick start

```bash
git clone https://github.com/mintychochip/token-usage.git
cd token-usage
cargo build --workspace
TOKEN_USAGE_STORE=./store.json TOKEN_USAGE_BIND=127.0.0.1:9473 \
  cargo run -p token-usage-cli --bin token-usage-api
```

```bash
TOKEN_USAGE_STORE=./store.json cargo run -p token-usage-cli --bin token-usage-reporter -- \
  ingest --adapter hermes --file crates/adapters/fixtures/hermes-session.json
```

You do **not** need a hosted API. Plugins write a **local** store. GitHub is
the remote: a gist or a directory you commit. `gh` must be logged in.

```bash
token-usage-reporter publish --gist            # secret gist (includes sessions)
token-usage-reporter publish --gist --public   # summary + shields badge only
token-usage-reporter pull --gist               # restore sessions from that gist

token-usage-reporter publish --dir ./usage     # files you commit / GitHub Pages
token-usage-reporter pull --dir ./usage
```

A secret gist (the default) has `usage.jsonl` so another machine can `pull`.
`--public` drops session ids and cannot be pulled back. The gist id is saved
next to the store as `github.json`. Point shields.io at the raw gist URL of
`usage-badge.json`.

Lower-level `export` / `import` still write the same files without `gh`:

```bash
token-usage-reporter export --format summary --file usage-summary.json
token-usage-reporter export --format shields --file usage-badge.json
token-usage-reporter export --format jsonl --file usage.jsonl
```

A local `token-usage-api` still exists if you want HTTP on loopback. A public
`api.mintychochip.dev` is optional and should stay `TOKEN_USAGE_STATELESS=1`
if you run one at all.

On first ingest or `list` for a harness, the reporter walks that host's on-disk
sessions (Grok `signals.json`, Pi/oh-my-pi JSONL, Codex/Claude JSONL, and other
JSON trees under the harness home) and stores each mapped session. Point
`--home` / `TOKEN_USAGE_HARNESS_HOME` at the directory that contains `.grok`,
`.pi`, `.omp`, and friends.

```bash
token-usage-reporter sync --harness grok
token-usage-reporter sync --force
```

## Harnesses

Claude Code, Codex, Grok, oh-my-pi, jcode, Hermes, OpenCode, Gemini CLI,
Aider, Goose, Amp, Droid, Cline, and Pi. Host wrappers live under
`plugins/` and exec `token-usage-reporter`; they do not parse usage themselves.

See [docs/living-specs/token-usage.md](docs/living-specs/token-usage.md) for
invariants and [CONTRIBUTING.md](CONTRIBUTING.md) to add another host.

## License

[MIT](LICENSE)
