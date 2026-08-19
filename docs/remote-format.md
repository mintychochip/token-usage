# Remote usage format — plugins stay stateless

> Status: research (format chosen; backend not implemented)
> Last updated: 2026-08-19

Host plugins must not grow a local usage database, last-synced file, or
merge log. They already do one thing:

```sh
exec "$REPORTER" ingest --adapter grok
```

(`plugins/grok/scripts/report.sh` and the other `plugins/*/scripts/report.sh`
wrappers). The hook JSON on stdin is **not** the state. The state today is
`FileStore` (`crates/store`): one row per `(harness, session_id)`, last-write-wins
on ingest, plus per-harness `last_synced_at`. A remote format replaces or fronts
**that** store. Domain objects stay `UsageObservation` / `WireObservation`.
No new fields.

A report is the existing wire object (`crates/cli/src/wire.rs`):

| Field | Required | Notes |
|-------|----------|--------|
| `harness` | yes | kebab-case slug (`grok`, `claude-code`, …) |
| `session_id` | yes | host session id, or `__harness_global__` |
| `input_tokens` | yes | |
| `output_tokens` | yes | |
| `source` | yes | `plugin_report` or `harness_global_approximation` |
| `completeness` | yes | `complete`, `partial`, or `unknown` |
| `extras` | no | `cache_read`, `cache_write`, `reasoning` when the host sent them |
| `last_synced_at` | no | unix seconds; the remote **should stamp this** so the plugin never clocks |

Same-identity merge is last-write-wins on `(harness, session_id)` (cumulative
snapshots, not deltas). Different harnesses stay distinct.

---

## Comparison

### 1. Existing HTTP observation API (remote process)

**Shape.** The reporter POSTs JSON to a reachable `token-usage-api`:

- Canonical snapshot: `POST /v1/observations` with the table above
- Raw host payload: `POST /v1/ingest/{harness}` (server adapts, then merges)
- Read-back: `GET /v1/sessions/{harness}/{session_id}`, `GET /v1/sessions`
- Scan status: `GET /v1/sync`

The server keeps `FileStore` (or an equivalent keyed map). Merge is the
same `ingest` last-write-wins already tested in `crates/store`.

**Plugin stateless?** Yes. Wrappers still `exec token-usage-reporter ingest`.
The reporter becomes a POST client. No local `~/.token-usage/store.json`, no
plugin-owned last-synced file.

**Same-identity merge remotely?** Yes, in the API process: replace the row for
that identity. A later GET returns the submitted counts.

**Fit.** Best match for “instead of this”: FileStore moves off the laptop;
the wire format does not change.

### 2. Object store or gist: one JSON document per identity

**Shape.** Put the same `WireObservation` at a key derived from identity:

```text
s3://bucket/token-usage/{harness}/{session_id}.json
```

or a GitHub gist / git blob with the same path convention. Overwrite the
object on each report. A single gist *map* of all sessions would need
read-modify-write and is **not** stateless at the writer.

**Plugin stateless?** Yes if the reporter only PUTs the snapshot for that
identity and exits. No if a wrapper reads the gist, merges, and writes back.

**Same-identity merge remotely?** Last PUT wins (object overwrite). Readers
list prefixes or GET by key. No append, no reduce.

**Fit.** Fine when you already have S3/R2 and do not want a long-lived API.
Worse operationally than option 1: no `GET /v1/sessions` together unless you
list the bucket. A gist-as-one-file is racy and should be rejected.

### 3. Append-only JSONL log, reduced by identity

**Shape.** Each report is one JSON line (`WireObservation`). Transport can be
a file, S3 append, NATS, or a log drain. A reader groups by
`(harness, session_id)` and keeps the line with the greatest `last_synced_at`
(or the last line if the clock is assigned at append).

**Plugin stateless?** Yes. Append and exit. The plugin never reads.

**Same-identity merge remotely?** Not at write time. Merge is a **reduce**
on read. Two reports for `grok/sess-alpha` are two lines; the view is one
total. This is correct only if reports are snapshots (they are).

**Fit.** Best audit trail. Worst interactive GET unless a reducer materializes
option 1 or 2. Do not make this the plugin’s store.

### 4. Other formats considered (not primary)

| Format | Plugin stateless? | Why not primary |
|--------|-------------------|-----------------|
| SQLite / Postgres row keyed by identity | Yes, if reporter INSERTs/UPSERTs | Same as FileStore with more ops; still a server |
| NATS/MQTT subject `token-usage.{harness}.{session}` last-value | Yes (publish) | Need a KV/stream consumer to query together |
| OpenTelemetry `token.usage` metrics | Yes | Drops session identity, source, completeness |
| Git commit of JSON files per identity | Almost (push) | High latency; merge conflicts; not hook-friendly |
| CRDT / multi-writer | N/A | Snapshots already last-write-wins; extra complexity |

---

## Recommendation

**Primary target: client-owned files, not a hosted usage API.**

You do not need `api.mintychochip.dev` for sync. The reporter writes a local
`FileStore`. To expose usage, export JSON you can gist, commit, or chart:

- `export --format summary` — per-harness totals, no session ids
- `export --format shields` — shields.io endpoint JSON
- `export --format jsonl` — full sessions for backup/import

A public HTTP API is optional. If you run one, keep it `TOKEN_USAGE_STATELESS=1`
(adapt only, no store). The previous “primary remote” of posting totals to a
hosted FileStore is **not** the product: it would keep usage on someone else’s
disk.

The hosted process sets `TOKEN_USAGE_STATELESS=1`. It accepts:

- `POST /v1/adapt/{harness}` — map a hook payload to `WireObservation`
- `POST /v1/ingest/{harness}` / `POST /v1/observations` — same JSON back,
  **no write**

`GET /v1/sessions` returns `501` (`stateless api: no store`). Nothing is
kept on the API host.

Totals live on the machine that ran the reporter:

- default: local `FileStore` (`TOKEN_USAGE_STORE`, usually `~/.token-usage/store.json`)
- portable: one `WireObservation` per line (`token-usage-reporter export` /
  `import`)

Plugins stay:

```sh
exec "$REPORTER" ingest --adapter <harness>
```

**Why this one:** the plugin still does not manage state; the API does not
hold anyone's usage; the user owns the JSON/JSONL. Last-write-wins stays in
the local store. Object-store PUT is a fallback if they want files they
manage elsewhere. JSONL export is that managed format.

**What each report must send** (do not invent a domain): `harness`,
`session_id`, `input_tokens`, `output_tokens`, `source`, `completeness`,
and `extras` when present. `last_synced_at` is assigned by the remote ingest.

---

## What does *not* change

- Host hook JSON
- Adapter mapping
- Plugin wrappers (they already do not manage FileStore)
- Last-write-wins identity rules

Implementation of a hosted API, object-store PUT, auth, and multi-machine
replication stays **out of scope** until promoted out of Future in the living
spec. This document only chooses the format.
