# Token Usage Graph Widget Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build an authenticated event-driven daily token-usage publisher, cached and rate-limited public series API, and reusable TypeScript Web Component plus React wrapper.

**Architecture:** A focused Rust aggregate module owns UTC bucket construction, persistence, querying, caching, and limiting while the existing Axum router wires hosted routes and configuration. The reporter computes absolute buckets from the local observation store and upserts each independently. A standalone ESM TypeScript package shares one typed client and SVG renderer between a custom element and a thin React adapter.

**Tech Stack:** Rust 2021, Axum 0.8, Tokio, Serde, Clap, Ureq, Chrono, SHA-256, TypeScript 5, Vite library mode, Vitest, jsdom, React 18/19 peer dependency, inline SVG.

## Global Constraints

- Store only daily aggregate records keyed by `(date, harness)`; never persist session identifiers, prompts, or raw observations in the hosted aggregate store.
- Publication events contain absolute totals and are idempotent; an older `updated_at` returns `409 Conflict`.
- Public ranges are inclusive UTC dates and cannot exceed 366 days.
- Supported metrics are exactly `input_tokens`, `output_tokens`, `total_tokens`, and `estimated_cost_usd`.
- Public and publication rate limits are independent, bounded, configurable token buckets.
- Only configured trusted-proxy mode may use forwarded client addresses.
- Public successful responses use bounded in-process caching, `ETag`, conditional `304`, and immediate mutation invalidation.
- React remains an optional peer dependency; framework-independent imports cannot load React.
- The graph has no charting runtime dependency and uses accessible inline SVG.
- Browser code never receives or stores the publication token.

---

## File Map

### Rust

- `crates/cli/src/daily.rs` — bucket wire types, UTC aggregation from observations, validation, and series query logic.
- `crates/cli/src/daily_store.rs` — versioned aggregate file, atomic upsert, stale-event handling, and generation counter.
- `crates/cli/src/api_controls.rs` — bounded token buckets, canonical response cache, ETag handling, CORS helpers, and trusted-proxy client resolution.
- `crates/cli/src/publish_api.rs` — authenticated HTTP upsert client and multi-bucket publication report.
- `crates/cli/src/lib.rs` — module exports, aggregate API state, routes, handlers, and stable errors.
- `crates/cli/src/bin/token-usage-api.rs` — hosted aggregate environment configuration and startup validation.
- `crates/cli/src/bin/token-usage-reporter.rs` — `publish-api` arguments and orchestration.
- `crates/cli/tests/daily.rs` — aggregation and series contract tests.
- `crates/cli/tests/daily_store.rs` — persistence and idempotency tests.
- `crates/cli/tests/api_controls.rs` — limiter, cache, ETag, and proxy tests.
- `crates/cli/tests/usage_api.rs` — Axum endpoint, auth, CORS, cache, and rate-limit tests.
- `crates/cli/tests/publish_api.rs` — reporter HTTP publication and partial-failure tests.

### TypeScript

- `packages/token-usage-widget/package.json` — package scripts, exports, peer dependencies, and published files.
- `packages/token-usage-widget/tsconfig.json` — strict source and declaration compilation.
- `packages/token-usage-widget/vite.config.ts` — ESM library build with React externalized.
- `packages/token-usage-widget/vitest.config.ts` — jsdom test environment.
- `packages/token-usage-widget/src/types.ts` — public request, response, metric, theme, and event types.
- `packages/token-usage-widget/src/client.ts` — validation, canonical URL generation, fetch deduplication, TTL cache, and abort handling.
- `packages/token-usage-widget/src/graph.ts` — dependency-free accessible SVG and view-state rendering.
- `packages/token-usage-widget/src/element.ts` — custom element lifecycle and attribute contract.
- `packages/token-usage-widget/src/react.tsx` — typed React wrapper and custom-event bridge.
- `packages/token-usage-widget/src/index.ts` — framework-independent exports and element registration helper.
- `packages/token-usage-widget/src/react-entry.ts` — React-only export.
- `packages/token-usage-widget/test/client.test.ts` — client contract and cache tests.
- `packages/token-usage-widget/test/element.test.ts` — element states, reactivity, accessibility, and retry tests.
- `packages/token-usage-widget/test/react.test.tsx` — prop mapping and event bridge tests.
- `packages/token-usage-widget/demo/index.html` — actual Web Component and React smoke surface.
- `packages/token-usage-widget/demo/main.tsx` — demo mounting both public surfaces.

### Documentation

- `README.md` — hosted aggregate environment, publish command, API query, Web Component, and React examples.

---

### Task 1: Daily Aggregate Domain

**Files:**
- Create: `crates/cli/src/daily.rs`
- Modify: `crates/cli/src/lib.rs`
- Modify: `crates/cli/Cargo.toml`
- Modify: `Cargo.toml`
- Test: `crates/cli/tests/daily.rs`

**Interfaces:**
- Consumes: `UsageObservation::{identity,counts,last_synced_at,model}`, `Harness`, and existing `PriceTable` pricing helpers.
- Produces: `DailyBucket`, `DailyUpsert`, `Metric`, `SeriesQuery`, `UsageSeries`, `aggregate_daily(&[UsageObservation], Option<&PriceTable>) -> Result<Vec<DailyBucket>, DailyError>`, and `build_series(&[DailyBucket], &SeriesQuery) -> Result<UsageSeries, DailyError>`.

- [ ] **Step 1: Add UTC date support and write aggregation tests**

Add workspace `chrono = { version = "0.4", default-features = false, features = ["std"] }`, consume it from `crates/cli`, and create tests using observations stamped around midnight:

```rust
#[test]
fn aggregates_absolute_totals_by_utc_date_and_harness() {
    let rows = vec![
        obs(Harness::Codex, "a", 1_000, 100, 1_787_011_199),
        obs(Harness::Codex, "b", 2_000, 200, 1_787_011_200),
        obs(Harness::Pi, "c", 3_000, 300, 1_787_011_200),
    ];
    let buckets = aggregate_daily(&rows, None).unwrap();
    assert_eq!(buckets, vec![
        bucket("2026-08-18", Harness::Codex, 1_000, 100),
        bucket("2026-08-19", Harness::Codex, 2_000, 200),
        bucket("2026-08-19", Harness::Pi, 3_000, 300),
    ]);
}
```

Also assert that missing timestamps return `DailyError::MissingTimestamp` rather than assigning today's date.

- [ ] **Step 2: Run the focused test and verify the red state**

Run: `cargo test -p token-usage-cli --test daily aggregates_absolute_totals_by_utc_date_and_harness`

Expected: compile failure because `daily` exports do not exist.

- [ ] **Step 3: Implement the aggregate types and UTC grouping**

Define serializable types with checked arithmetic:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DailyBucket {
    pub schema_version: u32,
    pub date: NaiveDate,
    pub harness: Harness,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub estimated_cost_usd: Option<f64>,
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DailyUpsert {
    pub schema_version: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub estimated_cost_usd: Option<f64>,
    pub updated_at: u64,
}
```

Use `DateTime::from_timestamp(last_synced_at, 0).date_naive()`, group in a `BTreeMap<(NaiveDate, Harness), ...>`, use `checked_add`, set `updated_at` to the maximum contributing timestamp, and sum only known cost estimates. Reuse existing pricing functions rather than creating a second pricing convention.

- [ ] **Step 4: Add query tests for ranges, metrics, harnesses, zero fill, and cost nullability**

```rust
#[test]
fn series_combines_selected_harnesses_and_fills_dates() {
    let query = SeriesQuery::parse("from=2026-08-18&to=2026-08-20&metrics=input_tokens,estimated_cost_usd&harnesses=codex,pi", fixed_today()).unwrap();
    let series = build_series(&fixture_buckets(), &query).unwrap();
    assert_eq!(series.points.len(), 3);
    assert_eq!(series.points[1]["input_tokens"], 4_000);
    assert_eq!(series.points[2]["input_tokens"], 0);
    assert!(series.points[2]["estimated_cost_usd"].is_null());
}
```

Add table cases for default 30 days, inclusive 366-day success, 367-day failure, reversed dates, partial dates, duplicates, empty lists, unknown parameters, metrics, and harnesses.

- [ ] **Step 5: Implement strict query parsing and series construction**

Make `SeriesQuery::parse` accept raw query pairs and an injected `today`, reject unknown or duplicate keys, canonicalize metrics/harnesses in enum order, and expose `canonical_key()`. Build each point from `from..=to`; include only selected metric properties. Represent omitted harness filter as `None` in both query and response.

- [ ] **Step 6: Run daily contract tests**

Run: `cargo test -p token-usage-cli --test daily`

Expected: all aggregation, validation, and series tests pass.

- [ ] **Step 7: Commit the domain slice**

```bash
git add Cargo.toml Cargo.lock crates/cli/Cargo.toml crates/cli/src/daily.rs crates/cli/src/lib.rs crates/cli/tests/daily.rs
git commit -m "Add daily token usage aggregates"
```

---

### Task 2: Durable Aggregate Store

**Files:**
- Create: `crates/cli/src/daily_store.rs`
- Modify: `crates/cli/src/lib.rs`
- Test: `crates/cli/tests/daily_store.rs`

**Interfaces:**
- Consumes: `DailyBucket` and `DailyUpsert` from Task 1.
- Produces: `DailyStore::open(path)`, `DailyStore::upsert(date, harness, upsert) -> Result<UpsertOutcome, DailyStoreError>`, `DailyStore::list() -> Result<Vec<DailyBucket>, DailyStoreError>`, `DailyStore::generation() -> u64`; `UpsertOutcome::{Inserted, Updated, Duplicate, Stale}`.

- [ ] **Step 1: Write persistence and event-order tests**

```rust
#[test]
fn absolute_upserts_are_idempotent_and_reject_older_events() {
    let store = DailyStore::open(path()).unwrap();
    assert_eq!(store.upsert(date(), Harness::Codex, upsert(100, 10)), Inserted);
    assert_eq!(store.upsert(date(), Harness::Codex, upsert(100, 10)), Duplicate);
    assert_eq!(store.upsert(date(), Harness::Codex, upsert(90, 9)), Stale);
    assert_eq!(store.upsert(date(), Harness::Codex, upsert(110, 11)), Updated);
    assert_eq!(DailyStore::open(path()).unwrap().list().unwrap()[0].input_tokens, 110);
}
```

Add tests for total mismatch, non-finite/negative cost, corrupt existing JSON, version mismatch, sorted list output, and atomic file replacement without a surviving `.tmp` file.

- [ ] **Step 2: Run store tests and verify failure**

Run: `cargo test -p token-usage-cli --test daily_store`

Expected: compile failure because `DailyStore` is missing.

- [ ] **Step 3: Implement versioned storage and validated atomic upsert**

Use one `Mutex<StoreState>` containing sorted buckets and generation. Parse existing storage strictly; do not reset corrupt data. Validate schema `1`, date/path harness, checked total equality, finite cost, and monotonic timestamp before mutation. Serialize to a sibling temporary file, `sync_all`, rename, and update memory only after persistence succeeds. Increment generation only for `Inserted` or `Updated`.

- [ ] **Step 4: Run store tests**

Run: `cargo test -p token-usage-cli --test daily_store`

Expected: all durable-store tests pass.

- [ ] **Step 5: Commit the store slice**

```bash
git add crates/cli/src/daily_store.rs crates/cli/src/lib.rs crates/cli/tests/daily_store.rs
git commit -m "Persist idempotent daily usage events"
```

---

### Task 3: Rate Limiter and Response Cache

**Files:**
- Create: `crates/cli/src/api_controls.rs`
- Modify: `crates/cli/src/lib.rs`
- Modify: `crates/cli/Cargo.toml`
- Modify: `Cargo.toml`
- Test: `crates/cli/tests/api_controls.rs`

**Interfaces:**
- Consumes: canonical query strings and daily-store generation.
- Produces: `RateLimiter<K>::new(LimitConfig)`, `RateLimiter::check(key, now) -> LimitDecision`, `ResponseCache::new(capacity, ttl)`, `ResponseCache::{get,insert}`, `CachedSeries { body, etag, generation, expires_at }`, `resolve_client_ip(peer, forwarded, trusted_hops)`, and `constant_time_eq`.

- [ ] **Step 1: Write deterministic token-bucket tests with injected time**

```rust
#[test]
fn limiter_allows_burst_then_returns_retry_after() {
    let limiter = RateLimiter::new(LimitConfig { per_minute: 2, burst: 2, max_entries: 4, idle_ttl: Duration::from_secs(120) });
    assert!(limiter.check("client", instant(0)).allowed);
    assert!(limiter.check("client", instant(0)).allowed);
    let denied = limiter.check("client", instant(0));
    assert!(!denied.allowed);
    assert_eq!(denied.retry_after, Duration::from_secs(30));
}
```

Test refill, independent keys, bounded least-recently-used eviction, idle eviction, read/write independence, and zero/overflow-invalid configuration.

- [ ] **Step 2: Write cache, ETag, auth, and proxy tests**

Use a capacity-2 cache to prove canonical-key hits, TTL expiry, generation mismatch, and LRU eviction. Assert SHA-256 ETags are quoted and stable. Assert constant-time equality accepts only equal bytes. Test socket-only resolution and trusted-hop selection across `X-Forwarded-For` chains.

- [ ] **Step 3: Run control tests and verify failure**

Run: `cargo test -p token-usage-cli --test api_controls`

Expected: compile failure because control types are missing.

- [ ] **Step 4: Implement bounded controls**

Add workspace `sha2 = "0.10"`. Implement token refill with saturating elapsed-time math, bounded maps, and deterministic eviction. Cache serialized `Bytes` or `Vec<u8>` plus quoted SHA-256 ETag. Do not cache errors. Implement proxy parsing with `IpAddr` and reject malformed chains instead of trusting caller text.

- [ ] **Step 5: Run control tests**

Run: `cargo test -p token-usage-cli --test api_controls`

Expected: limiter, cache, authentication, and proxy tests pass.

- [ ] **Step 6: Commit the controls slice**

```bash
git add Cargo.toml Cargo.lock crates/cli/Cargo.toml crates/cli/src/api_controls.rs crates/cli/src/lib.rs crates/cli/tests/api_controls.rs
git commit -m "Add bounded API rate limits and caching"
```

---

### Task 4: Hosted Usage API Routes

**Files:**
- Modify: `crates/cli/src/lib.rs`
- Modify: `crates/cli/src/bin/token-usage-api.rs`
- Create: `crates/cli/tests/usage_api.rs`
- Modify: `crates/cli/tests/api.rs`

**Interfaces:**
- Consumes: `DailyStore`, `DailyUpsert`, `SeriesQuery`, `build_series`, `RateLimiter`, and `ResponseCache`.
- Produces: `AggregateConfig::from_env()`, `ApiState::with_aggregates(...)`, `PUT /v1/usage/daily/{date}/{harness}`, and `GET /v1/usage/series`.

- [ ] **Step 1: Write authenticated upsert route tests**

Construct `ApiState::with_aggregates` with temporary storage and tiny deterministic limits. Assert missing/bad bearer tokens return `401`, valid inserts/duplicates/updates return `200`, stale events return `409`, malformed payloads return `400`, oversized bodies are rejected, publication responses use `Cache-Control: no-store`, and publication never emits `Access-Control-Allow-Origin`.

- [ ] **Step 2: Write public query, CORS, cache, and limiter tests**

```rust
#[tokio::test]
async fn series_supports_etag_cors_and_mutation_invalidation() {
    let first = get_series(&router, None, Some("https://mintychochip.dev")).await;
    assert_eq!(first.status(), StatusCode::OK);
    let etag = first.headers()[ETAG].clone();
    assert_eq!(first.headers()[ACCESS_CONTROL_ALLOW_ORIGIN], "https://mintychochip.dev");
    assert_eq!(get_series(&router, Some(etag), None).await.status(), StatusCode::NOT_MODIFIED);
    put_new_bucket(&router).await;
    let changed = get_series(&router, Some(etag), None).await;
    assert_eq!(changed.status(), StatusCode::OK);
    assert_ne!(changed.headers()[ETAG], etag);
}
```

Add explicit-origin rejection, preflight behavior, `Vary: Origin`, `Cache-Control`, canonical query cache hits, read `429` with `Retry-After`, write `429`, and health exemption.

- [ ] **Step 3: Run endpoint tests and verify failure**

Run: `cargo test -p token-usage-cli --test usage_api`

Expected: failures because aggregate routes and state are absent.

- [ ] **Step 4: Extend API state and route handlers**

Keep existing session/stateless behavior intact. Add an optional `Arc<AggregateApiState>` containing store, token, limits, cache, origins, and trusted hops. Route handlers return `501` only when tests construct legacy state without aggregate configuration; the shipped hosted binary fails startup if only one required aggregate environment variable is set. Enforce auth and limits before JSON/storage work. Serialize successful series once, compute ETag, cache by canonical key and generation, and handle `If-None-Match`.

- [ ] **Step 5: Parse hosted environment configuration**

Support these exact variables with validated positive bounds:

```text
TOKEN_USAGE_AGGREGATE_STORE
TOKEN_USAGE_PUBLISH_TOKEN
TOKEN_USAGE_ALLOWED_ORIGINS=https://mintychochip.dev
TOKEN_USAGE_READS_PER_MINUTE=120
TOKEN_USAGE_READ_BURST=30
TOKEN_USAGE_WRITES_PER_MINUTE=60
TOKEN_USAGE_WRITE_BURST=20
TOKEN_USAGE_RATE_LIMIT_MAX_ENTRIES=10000
TOKEN_USAGE_CACHE_TTL_SECONDS=60
TOKEN_USAGE_CACHE_CAPACITY=512
TOKEN_USAGE_TRUSTED_PROXY_HOPS=0
```

When both aggregate store and token are absent, preserve local-only startup. When exactly one is present or any numeric/origin value is invalid, print a precise startup error and exit `2`.

- [ ] **Step 6: Run existing and new API tests**

Run: `cargo test -p token-usage-cli --test api --test usage_api`

Expected: all existing API behavior and new aggregate contracts pass.

- [ ] **Step 7: Commit the hosted API slice**

```bash
git add crates/cli/src/lib.rs crates/cli/src/bin/token-usage-api.rs crates/cli/tests/api.rs crates/cli/tests/usage_api.rs
git commit -m "Serve cached token usage series"
```

---

### Task 5: Reporter Event Publisher

**Files:**
- Create: `crates/cli/src/publish_api.rs`
- Modify: `crates/cli/src/lib.rs`
- Modify: `crates/cli/src/bin/token-usage-reporter.rs`
- Create: `crates/cli/tests/publish_api.rs`
- Modify: `crates/cli/tests/reporter.rs`

**Interfaces:**
- Consumes: `aggregate_daily`, `DailyBucket`, `FileStore::list`, `load_price_table`, and Ureq.
- Produces: `PublishApiOptions { base_url, token, from, to }`, `publish_daily_buckets(&[DailyBucket], &PublishApiOptions) -> PublishReport`, and CLI command `publish-api --url URL --token-env NAME [--from DATE --to DATE]`.

- [ ] **Step 1: Write HTTP publication tests against a local fixture server**

Capture request paths, bearer headers, and bodies. Assert sorted one-request-per-bucket delivery, no token in diagnostics, continued delivery after one `500`, non-zero report failure count, `409` surfaced as stale, and successful `200` insert/duplicate/update handling.

- [ ] **Step 2: Write reporter CLI argument tests**

Assert `--token-env` defaults to `TOKEN_USAGE_PUBLISH_TOKEN`, the named environment variable must exist and be non-empty, `--url` rejects credentials/query/fragment, `--from` and `--to` must appear together, and the command filters inclusively.

- [ ] **Step 3: Run publisher tests and verify failure**

Run: `cargo test -p token-usage-cli --test publish_api --test reporter publish_api`

Expected: compile or assertion failure because the command and publisher are absent.

- [ ] **Step 4: Implement bucket publication and CLI orchestration**

Build each URL from a normalized HTTPS/HTTP base plus percent-safe known date/harness segments. Set bearer and JSON headers, enforce finite request timeout, parse status without echoing response secrets, collect `PublishFailure { date, harness, status, message }`, and continue. In the CLI, aggregate the current local store with pricing, filter dates, publish, print a concise count, and return an error after all attempts when failures exist.

- [ ] **Step 5: Run reporter and publisher tests**

Run: `cargo test -p token-usage-cli --test publish_api --test reporter`

Expected: all new and existing reporter contracts pass.

- [ ] **Step 6: Commit the publisher slice**

```bash
git add crates/cli/src/publish_api.rs crates/cli/src/lib.rs crates/cli/src/bin/token-usage-reporter.rs crates/cli/tests/publish_api.rs crates/cli/tests/reporter.rs
git commit -m "Publish daily usage events to hosted API"
```

---

### Task 6: Typed TypeScript Client

**Files:**
- Create: `packages/token-usage-widget/package.json`
- Create: `packages/token-usage-widget/tsconfig.json`
- Create: `packages/token-usage-widget/vite.config.ts`
- Create: `packages/token-usage-widget/vitest.config.ts`
- Create: `packages/token-usage-widget/src/types.ts`
- Create: `packages/token-usage-widget/src/client.ts`
- Create: `packages/token-usage-widget/src/index.ts`
- Create: `packages/token-usage-widget/test/client.test.ts`

**Interfaces:**
- Produces: `UsageMetric`, `HarnessId`, `UsageSeriesRequest`, `UsageSeries`, `UsagePoint`, `fetchTokenUsageSeries(request, options?)`, and `clearTokenUsageCache()`.
- Request accepts exactly one of `{ range: "7d" | "30d" | "90d" | "1y" }` or `{ from: string; to: string }`.

- [ ] **Step 1: Create package metadata and strict build configuration**

Define ESM exports `.` and `./react`; include generated `.d.ts`; mark React external and optional peer dependency. Scripts:

```json
{
  "scripts": {
    "build": "tsc --noEmit && vite build",
    "test": "vitest run"
  }
}
```

Use strict TypeScript, DOM libraries, `noUncheckedIndexedAccess`, and jsdom tests.

- [ ] **Step 2: Write URL and runtime validation tests**

Test preset conversion against injected UTC `today`, explicit dates, sorted/deduplicated metric and harness values, endpoint slash normalization, mutually exclusive range/date rejection, HTTP error handling, malformed schema rejection, and preservation of `AbortSignal`.

- [ ] **Step 3: Write cache and request-deduplication tests**

Use a mocked fetch and clock. Prove concurrent canonical requests call fetch once, successful responses cache for server `max-age`, expired entries refetch, `cache: "no-store"` bypasses storage, failures are not cached, and one caller abort does not poison a distinct later request.

- [ ] **Step 4: Run client tests and verify failure**

Run: `npm test -- --run test/client.test.ts` from `packages/token-usage-widget`.

Expected: module-not-found failures before implementation.

- [ ] **Step 5: Implement public types and client**

Use discriminated request unions, `URL`/`URLSearchParams`, strict own-property checks for response points, finite/non-negative number validation, and a bounded 64-entry LRU. Read `Cache-Control: max-age`; cap client retention at 60 seconds. Deduplicate only live requests with identical canonical URLs and cache mode.

- [ ] **Step 6: Run client tests and build**

Run: `npm test -- --run test/client.test.ts && npm run build` from `packages/token-usage-widget`.

Expected: tests pass and ESM/declaration output builds without bundling React into the base entry.

- [ ] **Step 7: Commit the client slice**

```bash
git add packages/token-usage-widget
git commit -m "Add typed token usage series client"
```

---

### Task 7: Accessible Web Component Graph

**Files:**
- Create: `packages/token-usage-widget/src/graph.ts`
- Create: `packages/token-usage-widget/src/element.ts`
- Modify: `packages/token-usage-widget/src/index.ts`
- Create: `packages/token-usage-widget/test/element.test.ts`

**Interfaces:**
- Consumes: `fetchTokenUsageSeries`, `UsageSeriesRequest`, and `UsageSeries`.
- Produces: `TokenUsageGraphElement`, `defineTokenUsageGraph(tagName = "token-usage-graph")`, `TokenUsageLoadEvent`, and `TokenUsageErrorEvent`.

- [ ] **Step 1: Write lifecycle and attribute tests**

Assert default attributes produce the default API query, all observed attribute changes abort and replace the request, invalid combinations render an error without fetch, disconnected elements abort, `AbortError` is silent, and stale responses cannot overwrite a newer query.

- [ ] **Step 2: Write graph state and accessibility tests**

Assert loading skeleton, empty state, API error plus retry button, successful SVG with one series per metric, token/USD dual axes, legend totals, `<title>` and `<desc>`, semantic text summary, focusable date markers, keyboard left/right movement, custom load/error events, reduced-motion CSS, and theme custom properties.

- [ ] **Step 3: Run element tests and verify failure**

Run: `npm test -- --run test/element.test.ts`.

Expected: missing element/renderer modules.

- [ ] **Step 4: Implement pure SVG geometry and formatting**

Create pure helpers for linear scales, x positions including one-point series, abbreviated axis ticks, exact tooltip values, and SVG path strings. Use separate token and USD scales only when both classes are selected. Avoid per-render global listeners and avoid rebuilding data structures inside point loops.

- [ ] **Step 5: Implement element lifecycle and states**

Attach one shadow root, parse attributes into the typed request, sequence requests with an incrementing revision, abort obsolete fetches, and render loading/success/empty/error states. Retry repeats the current request with `cache: "no-store"`. Dispatch typed bubbling composed custom events without exposing internal elements.

- [ ] **Step 6: Run element and client tests plus build**

Run: `npm test -- --run test/client.test.ts test/element.test.ts && npm run build`.

Expected: all component tests pass and package builds.

- [ ] **Step 7: Commit the Web Component slice**

```bash
git add packages/token-usage-widget/src packages/token-usage-widget/test packages/token-usage-widget/package.json
git commit -m "Add accessible token usage graph element"
```

---

### Task 8: React Wrapper and Browser Demo

**Files:**
- Create: `packages/token-usage-widget/src/react.tsx`
- Create: `packages/token-usage-widget/src/react-entry.ts`
- Create: `packages/token-usage-widget/test/react.test.tsx`
- Create: `packages/token-usage-widget/demo/index.html`
- Create: `packages/token-usage-widget/demo/main.tsx`
- Modify: `packages/token-usage-widget/package.json`
- Modify: `packages/token-usage-widget/vite.config.ts`

**Interfaces:**
- Consumes: registered `TokenUsageGraphElement` and its custom events.
- Produces: React `TokenUsageGraph` and `TokenUsageGraphProps` from `@mintychochip/token-usage-widget/react`.

- [ ] **Step 1: Write React prop and event tests**

Render with Testing Library and assert arrays map to comma-separated attributes, range/date props remain mutually exclusive at the TypeScript boundary, theme/cache map correctly, prop changes update attributes, `onLoad` receives `UsageSeries`, `onError` receives `Error`, and listener cleanup prevents duplicate callbacks after rerender/unmount.

- [ ] **Step 2: Run React test and verify failure**

Run: `npm test -- --run test/react.test.tsx`.

Expected: missing React entry and wrapper.

- [ ] **Step 3: Implement the thin wrapper**

Use a ref plus `useEffect` only for custom event subscription. Render the custom element directly; do not fetch or draw in React. Define JSX intrinsic element typing locally without mutating consumer globals unexpectedly. Keep `react` and `react-dom` external in the React build.

- [ ] **Step 4: Build a demo surface using both integrations**

Mount one native element and one React wrapper against an endpoint supplied by `?endpoint=` with default `http://127.0.0.1:9473`. Include controls for `7d/30d/90d/1y`, metric toggles, harness input, theme, and an intentional invalid endpoint button for error/retry verification.

- [ ] **Step 5: Run package tests and inspect bundle imports**

Run: `npm test && npm run build`.

Expected: all tests pass. Inspect package build metadata/output to confirm the base entry has no `react` import and the React entry keeps React external.

- [ ] **Step 6: Commit the React and demo slice**

```bash
git add packages/token-usage-widget
git commit -m "Add React token usage graph wrapper"
```

---

### Task 9: End-to-End Verification and Documentation

**Files:**
- Modify: `README.md`
- Modify: `scripts/install.sh`
- Modify: `scripts/update.sh`
- Modify: `packages/token-usage-widget/demo/index.html` only if runtime verification exposes a defect
- Modify: implementation files only for defects proven during this task

**Interfaces:**
- Consumes: complete Rust API/reporter and TypeScript package.
- Produces: deploy/run instructions and verified end-to-end behavior.

- [ ] **Step 1: Add deploy and consumption documentation**

Document every aggregate environment variable with defaults and proxy warning; show `publish-api`; show range/metric/harness query; show Web Component script/import usage; show React import; document CSS variables, cache behavior, `429` handling, and single-instance limiter/cache constraint. Update install/update scripts to copy the built widget distribution only when present, without requiring Node for Rust-only installs.

- [ ] **Step 2: Run Rust formatting, lint, and all workspace tests**

Run:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: all commands exit `0`.

- [ ] **Step 3: Run TypeScript tests and production build**

Run from `packages/token-usage-widget`:

```bash
npm test
npm run build
```

Expected: all Vitest contracts pass and the production package builds.

- [ ] **Step 4: Launch the actual API with temporary hosted state**

Start `token-usage-api` with:

```text
TOKEN_USAGE_BIND=127.0.0.1:9473
TOKEN_USAGE_STATELESS=1
TOKEN_USAGE_AGGREGATE_STORE=<temp>/daily.json
TOKEN_USAGE_PUBLISH_TOKEN=<fixture-secret>
TOKEN_USAGE_ALLOWED_ORIGINS=http://127.0.0.1:4173
TOKEN_USAGE_READS_PER_MINUTE=6
TOKEN_USAGE_READ_BURST=2
```

Wait for the emitted listening line and health response.

- [ ] **Step 5: Exercise publication, correction, filtering, caching, and limits**

Use the actual reporter with a fixture local store to publish all buckets. Repeat once to prove duplicate delivery, modify one local observation and publish again to prove correction, then query:

```text
/v1/usage/series?from=2026-08-01&to=2026-08-19&metrics=input_tokens,output_tokens
/v1/usage/series?from=2026-08-01&to=2026-08-19&metrics=total_tokens,estimated_cost_usd&harnesses=codex,pi
```

Record successful JSON, `Cache-Control`, `ETag`, matching `304`, changed ETag after correction, allowed CORS origin, rejected origin, and `429` plus `Retry-After` after exhausting the configured burst.

- [ ] **Step 6: Browser-verify both component surfaces**

Serve the built demo, open it in Chromium, and verify at desktop and narrow mobile widths:

- native Web Component and React wrapper both load real API data;
- range, metrics, and harness changes issue correct queries;
- token-only, cost-only, and dual-axis graphs remain legible;
- pointer tooltip and keyboard point traversal show exact values;
- light, dark, and auto themes render correctly;
- invalid endpoint shows error and retry recovers after restoration;
- no console errors or accessibility-name omissions occur.

- [ ] **Step 7: Commit documentation and verified fixes**

```bash
git add README.md scripts/install.sh scripts/update.sh packages/token-usage-widget crates
git commit -m "Document and verify hosted usage widget"
```

- [ ] **Step 8: Request final code review**

Invoke `superpowers:requesting-code-review`, resolve all correctness/security findings, then rerun the commands and smoke scenarios affected by any fix.
