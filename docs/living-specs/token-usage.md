# Token Usage — Living Spec

> Status: active
> Last updated: 2026-08-21
> Owners: toktally

## Intent

Give every coding-agent harness one place to report token usage. Plugins and
host wrappers map whatever the host actually emits (a session hook, a global
`/usage` snapshot, or a partial session fragment) into the same observation,
and a Rust API persists those observations so later reads return the same
totals for the same harness/session identity.

Success looks like: Claude Code, Codex, Grok, oh-my-pi, jcode, Hermes,
OpenCode, Gemini CLI, Aider, Goose, Amp, Droid, Cline, and Pi can each
submit usage; a second report for the same identity updates the stored total
instead of creating a sibling; different harnesses stay distinct and remain
queryable together.

## Boundaries

### In scope

- Domain objects for harness, session, counts (plus optional extras), source,
  and session-store completeness
- Durable local store with sync-on-same-identity
- Publish/pull to a user-owned GitHub gist or a directory they commit
- Central widget service (`widgets.mintychochip.dev`) as the default publish target
- Signed ed25519 identity derived from a local keypair; UUID derived from the public key
- Multi-target `publish` controlled by `~/.toktally/publish-config.json`
- GitHub Pages opt-out via `--github-pages` (creates/updates a repo with Pages enabled)
- Copy-paste GitHub badge and website embed that read published summary/badge JSON
### Out of scope / non-goals

- Marketplace publishing or live install into a running harness
- Billing UI, invoices, payment, or a usage TUI/dashboard
- Asking the user to submit $/token (prices are looked up internally)
- Auth, multi-user tenancy, or a hosted usage database
- Remote replication of user totals onto api.mintychochip.dev
- Implementing a tokenizer or independently recounting tokens
- Completing or reverse-engineering Grok Build's session store

## Invariants

- An observation identity is `(harness, session_id)`. The same session string
  under two harnesses is two identities.
- Named harnesses are Claude Code, Codex, Grok, oh-my-pi, jcode, Hermes,
  OpenCode, Gemini CLI, Aider, Goose, Amp, Droid, Cline, and Pi.
- Input and output token counts are always present. Extra counts (cache,
  reasoning, tokens before/after compact) are optional.
- `model` is optional. Cost is derived at read time from `(model, counts, price
  table)`. Missing model or unknown price means no cost — do not invent a rate
  or a default model.
- A public summary that already has per-session plugin reports must not also add
  that harness's `__harness_global__` row (that would double-count).
- `ObservationSource` is either a per-session plugin report or a harness-global
  approximation.
- `SessionStoreCompleteness` is `complete`, `partial`, or `unknown`. Grok
  fragments default to partial/unknown rather than inventing a full store.
- Ingest of an existing identity updates that identity's totals in place.
  It never creates a second independent total for the same identity.
- Adapters only translate payloads. They do not persist. The store is the
  single write path.
- Host wrappers exec the Rust reporter; they do not re-implement ingest.
- The store records `last_synced_at` on every ingest and a per-harness scan time.
- The first ingest/list/sync for a harness walks that host's on-disk sessions and
  ingest each mapped payload. Later reports do not rescan unless `--force`.
- Scanners must not invent token counts. Unreadable or unmapped files are skipped.

## Implementation guidance

- Domain crate is pure: no HTTP, no filesystem.
- Store is file-backed JSON with atomic replace (temp file + rename).
- Same-identity sync is last-write-wins on counts, source, and completeness
  (harness reports are cumulative snapshots, not deltas).
- API is a thin I/O layer over the store. Reporter is the plugin entry point
  (`ingest` from stdin/file, `get`, `publish`/`pull`, and the API binary).
- GitHub transport is `gh gist` / `gh api`. Tests fake `gh` via `TOKTALLY_GH`.
  Public publishes omit `usage.jsonl`.
- Global `/usage` snapshots live at `{harness-home}/usage.json` and are marked
  `kind: global_usage`. `sync --interval N` re-scans in a loop (implies force).
- Prices: parse OpenRouter `/api/v1/models` (LiteLLM object as fallback). Cache
  `prices.json` next to the store. `TOKTALLY_PRICES` overrides. Fetch is
  skipped when `TOKTALLY_PRICES_FETCH=0`. No cost if model or rate is missing.
- Ambiguous host ids (`opus-5-1m`, `claude-opus-5-200k`) resolve to the longest
  matching priced id. Exact priced variants win. Do not collapse onto a sibling
  (`opus-4`). Still no cost when nothing unique matches.
- GitHub/website components are paste snippets over published `usage-badge.json`
  / `usage-summary.json`. They must not include session ids.
- Tests drive shipped types and the real store/adapters, using fixture JSON
  rather than live harness processes.
- Do not hard-code expected totals in tests without feeding those totals
  through the adapter/store. Do not re-implement merge or parse logic in tests.

## Current

- [x] Living domain catalog for cross-harness token usage
- [x] Domain objects (harness, session, counts, source, completeness)
- [x] Durable store: ingest, read-back, same-identity merge, distinct harnesses
- [x] Rust API + reporter CLI
- [x] Adapters and fixtures for all named harnesses
- [x] Host-native plugin/hook wrappers that call the reporter
- [x] Hermes Agent, OpenCode, Gemini CLI, Aider, Goose, Amp, Droid, Cline, and Pi adapters
- [x] `scripts/install.sh` and `scripts/update.sh` prefix installs
- [x] First-use sync of existing harness sessions plus `last_synced_at`
- [x] GitHub publish/pull: secret gist (or a directory) holds JSONL; public gist is summary + shields only
- [x] Optional sync against a harness's own global `/usage` snapshot on a timer (`{harness}/usage.json`, `sync --interval`)
- [x] Compaction-aware extra counts (tokens before/after compact)
- [x] Persist host `model` on observations when present
- [x] Internal USD estimates from OpenRouter (LiteLLM fallback); cache next to the store
- [x] Resolve host model variants (`opus-5-1m`) to a priced base id
- [x] Summary/badge skip global rows when session reports exist for that harness
- [x] Copy-paste GitHub shields badge and website embed from published JSON

## Next
- [x] Stateless hosted adapt API (`TOKTALLY_STATELESS`); storage stays client-owned
- [x] Central widget service with signed `POST /api/v1/publish` and public summary/card/profile routes
- [x] Local ed25519 identity stored in `~/.toktally/keys/`
- [x] Multi-target `publish` with shared summary; failures are collected, not skipped
- [x] GitHub Pages opt-out via `--github-pages` using `gh` + `git`
- [x] Summary/shields export so usage can be gist’d or charted without a hosted DB

## Future

- [ ] Billing UI / invoices and a usage TUI
- [ ] Multi-machine replication beyond gist/dir pull
- [ ] Auth and multi-user tenancy
- [ ] Object-store PUT per identity (fallback if no always-on HTTP)
- [ ] Append-only JSONL audit log reduced by identity

## Decisions log

| Date | Decision | Why |
|------|----------|-----|
| 2026-08-19 | Last-write-wins merge on same identity | Host reports are cumulative snapshots; adding would double-count |
| 2026-08-19 | Shared Rust reporter; host wrappers only exec it | Hosts often cannot load Rust; ingest must stay in one place |
| 2026-08-19 | Grok session fragments are partial/unknown | Grok Build may not expose a complete session store |
| 2026-08-19 | Global approximations use reserved `__harness_global__` when the payload has no session | `source` still distinguishes plugin vs global; a real session id is allowed if the host supplies one |
| 2026-08-19 | Expand named harnesses to include Hermes, OpenCode, Gemini CLI, Aider, Goose, Amp, Droid, Cline, and Pi | Same class of coding-agent hosts with plugin/hook or usage-dump surfaces |
| 2026-08-19 | First ingest/list scans all discoverable host sessions; stamp last_synced_at | Users already have history in Grok/Pi/omp/Codex stores; hooks only see the active session |
| 2026-08-19 | Remote format: HTTP POST of existing WireObservation; plugins stay `exec toktally ingest` | FileStore is the state to front, not hook JSON; no new domain. See docs/remote-format.md |
| 2026-08-19 | Hosted API (api.mintychochip.dev) is stateless; users keep FileStore or JSONL | Do not store anyone's usage on the domain; adapt only |
| 2026-08-19 | Do not require a hosted API; expose usage via summary/shields JSON on GitHub | Sync is local; charts/gists are files the user publishes |
| 2026-08-19 | GitHub is the remote: `publish`/`pull` via gist (`gh`) or a directory | Ingest stays FileStore; no usage DB on mintychochip.dev. Secret gist includes `usage.jsonl`; public gist is summary + badge only |
| 2026-08-19 | `{harness-home}/usage.json` is the global `/usage` snapshot; `sync --interval` re-reads it | Hosts dump a global approximation separately from session logs |
| 2026-08-19 | ExtraCounts.tokens_before/after copy Grok `totalTokensBeforeCompaction` / `contextTokensUsed` | Do not invent counts; only map fields the host already sent |
| 2026-08-19 | Persist optional `model`; estimate USD from OpenRouter (not user-submitted rates) | Cost is internal; missing model/price means no cost |
| 2026-08-19 | Price lookup strips context-window suffixes and matches hyphen tokens | Hosts send `opus-5-1m`; the catalog has `opus-5`. Exact variant still wins |
| 2026-08-21 | Central widget service is the default publish target; GitHub Pages is opt-out | One command publishes everywhere; users who dislike the central service can switch to a repo they own |
| 2026-08-21 | `publish` computes one summary and attempts every enabled target, collecting errors | A failure on one backend must not roll back the others or hide later successes |
| 2026-08-21 | Machine-local ed25519 identity in `~/.toktally/keys/`; UUID from blake3(public_key) | No accounts, passwords, or user setup; public key never leaves the machine |

## Open questions

- [x] How to merge a second report for the same session? Last-write-wins.
- [x] Reserved `__harness_global__` session id for global approximations that omit a session id. Any session id is allowed when `source` is global.
