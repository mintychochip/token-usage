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

`POST /v1/observations` accepts a canonical observation.  
`POST /v1/ingest/{harness}` accepts a raw host payload.

## Harnesses

Claude Code, Codex, Grok, oh-my-pi, jcode, Hermes, OpenCode, Gemini CLI,
Aider, Goose, Amp, Droid, Cline, and Pi. Host wrappers live under
`plugins/` and exec `token-usage-reporter`; they do not parse usage themselves.

See [docs/living-specs/token-usage.md](docs/living-specs/token-usage.md) for
invariants and [CONTRIBUTING.md](CONTRIBUTING.md) to add another host.

## License

[MIT](LICENSE)
