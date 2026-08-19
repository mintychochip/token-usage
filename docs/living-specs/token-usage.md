# Token Usage — Living Spec

> Status: active
> Last updated: 2026-08-19
> Owners: token-usage

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
- Thin Rust HTTP API and a reporter CLI that plugins exec
- Adapters for representative harness payloads
- Host-native wrappers (hooks/manifests/scripts) that invoke the reporter
- First-use scan of existing harness session stores (all discoverable sessions, not only the active one)
- Last-synced timestamps per observation and per harness

### Out of scope / non-goals

- Marketplace publishing or live install into a running harness
- Billing, pricing, dashboards, TUI/web UI
- Auth, multi-user tenancy, remote replication
- Implementing a tokenizer or independently recounting tokens
- Completing or reverse-engineering Grok Build's session store

## Invariants

- An observation identity is `(harness, session_id)`. The same session string
  under two harnesses is two identities.
- Named harnesses are Claude Code, Codex, Grok, oh-my-pi, jcode, Hermes,
  OpenCode, Gemini CLI, Aider, Goose, Amp, Droid, Cline, and Pi.
- Input and output token counts are always present. Extra counts (cache,
  reasoning) are optional.
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
  (`ingest` from stdin/file, `get`, and the API binary).
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

## Next

- [ ] Optional sync against a harness's own global `/usage` snapshot on a timer
- [ ] Compaction-aware extra counts (tokens before/after compact)
- [ ] Reporter POST of WireObservation to a remote `token-usage-api` (plugins stay exec-only)

## Future

- [ ] Billing/pricing tables and a usage TUI
- [ ] Multi-machine replication
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
| 2026-08-19 | Remote format: HTTP POST of existing WireObservation; plugins stay `exec token-usage-reporter ingest` | FileStore is the state to front, not hook JSON; no new domain. See docs/remote-format.md |

## Open questions

- [x] How to merge a second report for the same session? Last-write-wins.
- [x] Reserved `__harness_global__` session id for global approximations that omit a session id. Any session id is allowed when `source` is global.
