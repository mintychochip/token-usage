# toktally

[![build](https://github.com/mintychochip/toktally/actions/workflows/ci.yml/badge.svg)](https://github.com/mintychochip/toktally/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/mintychochip/toktally?logo=github&label=release)](https://github.com/mintychochip/toktally/releases)
[![Platforms](https://img.shields.io/badge/platforms-linux%20%7C%20macos%20%7C%20windows-lightgrey)](https://github.com/mintychochip/toktally/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Cross-harness toktally store. Plugins map whatever a coding agent emits — a
session hook, a global `/usage` snapshot, or a partial session fragment — into
one observation. A Rust API persists it so a later read returns the same
totals for the same harness/session identity.

Repository: [github.com/mintychochip/toktally](https://github.com/mintychochip/toktally)

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/mintychochip/toktally/master/scripts/install.sh | sh
```

From a checkout:

```bash
git clone https://github.com/mintychochip/toktally.git
cd toktally
./scripts/install.sh          # PREFIX=$HOME/.local by default
./scripts/update.sh           # git pull --ff-only, then reinstall
```

This puts `toktally` and `toktally-api` in `$PREFIX/bin` and copies host wrappers to `$PREFIX/share/toktally/plugins`. Cargo is required for a source build.

## Quick start

```bash
git clone https://github.com/mintychochip/toktally.git
cd toktally
cargo build --workspace
TOKTALLY_STORE=./store.json TOKTALLY_BIND=127.0.0.1:9473 \
  cargo run -p toktally-cli --bin toktally-api
```

```bash
TOKTALLY_STORE=./store.json cargo run -p toktally-cli --bin toktally -- \
  ingest --adapter hermes --file crates/adapters/fixtures/hermes-session.json
```

You do **not** need a hosted API. Plugins write a **local** store. GitHub is
the remote: a gist or a directory you commit. `gh` must be logged in.

```bash
toktally publish --gist            # secret gist (includes sessions)
toktally publish --gist --public   # summary + shields badge only
toktally pull --gist               # restore sessions from that gist

toktally publish --dir ./usage --url https://you.github.io/usage
toktally pull --dir ./usage
```

`publish` prints paste snippets (also `snippets.md` in a directory publish):

```markdown
[![token usage](https://img.shields.io/endpoint?url=https%3A%2F%2Fyou.github.io%2Fusage%2Fusage-badge.json)]
```

```html
<div class="toktally-card" data-summary-url="https://you.github.io/usage/usage-summary.json"></div>
<script>
/* inlined from embed/usage-card.js — gist raw cannot serve JS */
</script>
```

`publish` prints the full inlined snippet (gist raw is `text/plain` and cannot host `usage-card.js`).
The card shows totals and estimated cost when present. It never includes session ids.
A directory publish also copies `usage-card.js` next to the JSON if you prefer a separate file.

A secret gist (the default) has `usage.jsonl` so another machine can `pull`.
`--public` drops session ids and cannot be pulled back. The gist id is saved
next to the store as `github.json`. Point shields.io at the raw gist URL of
`usage-badge.json`.

Lower-level `export` / `import` still write the same files without `gh`:

```bash
toktally export --format summary --file usage-summary.json
toktally export --format shields --file usage-badge.json
toktally export --format jsonl --file usage.jsonl
```

`export --format summary` includes `estimated_cost_usd` when the host named a
model we can price. Rates come from OpenRouter (`GET /api/v1/models`), cached
next to the store as `prices.json`. You do not submit $/token. Override with
`TOKTALLY_PRICES=/path/to/prices.json`. Set `TOKTALLY_PRICES_FETCH=0` to
skip the network. Host ids like `opus-5-1m` price as `opus-5` when that is
what the catalog has. No model or unknown model means no cost.

A local `toktally-api` still exists if you want HTTP on loopback. A public
`api.mintychochip.dev` is optional and should stay `TOKTALLY_STATELESS=1`
if you run one at all.

On first ingest or `list` for a harness, the reporter walks that host's on-disk
sessions (Grok `signals.json`, Pi/oh-my-pi JSONL, Codex/Claude JSONL, and other
JSON trees under the harness home) and stores each mapped session. Point
`--home` / `TOKTALLY_HARNESS_HOME` at the directory that contains `.grok`,
`.pi`, `.omp`, and friends.

```bash
toktally sync --harness grok
toktally sync --force
toktally sync --interval 3600   # re-read sessions and `{harness}/usage.json`
```

## Harnesses

Claude Code, Codex, Grok, oh-my-pi, jcode, Hermes, OpenCode, Gemini CLI,
Aider, Goose, Amp, Droid, Cline, and Pi. Host wrappers live under
`plugins/` and exec `toktally`; they do not parse usage themselves.

See [docs/living-specs/token-usage.md](docs/living-specs/token-usage.md) for
invariants and [CONTRIBUTING.md](CONTRIBUTING.md) to add another host.

## License

[MIT](LICENSE)
