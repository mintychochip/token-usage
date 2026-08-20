# Token Usage Graph Widget Design

## 1. Overview

Add an event-driven, privacy-preserving token-usage graph to `token-usage-api` and a reusable TypeScript package for websites. The hosted API at `api.mintychochip.dev` receives authenticated absolute daily aggregate upserts, serves public filtered time series, and never stores session identifiers, prompts, or raw observations for this feature.

The website package exposes a typed client, a framework-independent Web Component, and a thin React wrapper. Consumers choose the date range, one or more metrics, and optional harness filters.

## 2. Scope

### Included

- Aggregate each local store's observations into UTC daily buckets by harness.
- Publish a changed `(date, harness)` bucket as an authenticated idempotent event.
- Backfill all existing daily buckets explicitly from the reporter CLI.
- Persist hosted aggregate buckets independently from the existing session store.
- Query an inclusive date range with multiple metrics and harness filters.
- Rate-limit writes and public reads independently.
- Cache public responses in process and advertise safe HTTP cache semantics.
- Provide a typed TypeScript client, `<token-usage-graph>` Web Component, and React wrapper.
- Render an accessible, responsive SVG graph in light and dark themes.

### Excluded

- Multiple owners or public user profiles.
- Raw hosted session ingestion for graph generation.
- Prompts, session identifiers, repository data, or GitHub profile statistics.
- Browser-side publication credentials.
- Server-side account management or token issuance.
- An external cache or database dependency in the initial deployment.

## 3. Aggregate Data Model

A stored bucket is uniquely identified by `(date, harness)`:

```json
{
  "schema_version": 1,
  "date": "2026-08-19",
  "harness": "codex",
  "input_tokens": 125000,
  "output_tokens": 18000,
  "total_tokens": 143000,
  "estimated_cost_usd": 1.42,
  "updated_at": 1787097600
}
```

- `date` is an ISO `YYYY-MM-DD` UTC date.
- Token fields are non-negative integers.
- `total_tokens` equals `input_tokens + output_tokens` and is validated server-side.
- `estimated_cost_usd` is optional and non-negative. A missing value remains unknown rather than becoming zero.
- `updated_at` is the publisher's Unix timestamp and prevents an older retry from overwriting a newer bucket.
- The storage file contains only these aggregate records and a schema version.
- Writes use a temporary file plus atomic rename under an exclusive process lock.

Absolute upserts are event-based without delta ambiguity: delivering the same event repeatedly yields the same state, and recomputing a historical bucket corrects late-arriving observations.

## 4. Publication Flow

### 4.1 Endpoint

```http
PUT /v1/usage/daily/{date}/{harness}
Authorization: Bearer <TOKEN_USAGE_PUBLISH_TOKEN>
Content-Type: application/json
```

The body omits `date` and `harness`, which come from the path:

```json
{
  "schema_version": 1,
  "input_tokens": 125000,
  "output_tokens": 18000,
  "total_tokens": 143000,
  "estimated_cost_usd": 1.42,
  "updated_at": 1787097600
}
```

Responses:

- `200 OK`: bucket inserted or replaced.
- `200 OK`: duplicate event accepted without changing state.
- `409 Conflict`: event is older than the stored `updated_at`.
- `400 Bad Request`: malformed date, unknown harness, invalid totals, negative/non-finite cost, or unsupported schema.
- `401 Unauthorized`: missing or invalid bearer token.
- `429 Too Many Requests`: write limit exceeded, with `Retry-After`.

Authentication compares the configured token in constant time. Hosted aggregate routes are enabled only when both `TOKEN_USAGE_AGGREGATE_STORE` and `TOKEN_USAGE_PUBLISH_TOKEN` are configured; missing configuration fails startup rather than exposing a partially configured write route.

### 4.2 Reporter behavior

Add a reporter command that computes daily buckets from the local durable store and publishes them:

```bash
token-usage-reporter publish-api \
  --url https://api.mintychochip.dev \
  --token-env TOKEN_USAGE_PUBLISH_TOKEN
```

Default behavior publishes every current bucket, making the command a reliable backfill and repair operation. Optional `--from` and `--to` bounds restrict publication. Each bucket is one independent upsert; a failed request is reported with its date and harness, publication continues for other buckets, and the process exits non-zero if any event failed.

The token is read from an environment variable and is never accepted as a command-line value, printed, persisted, or placed in browser code. This implementation provides the event protocol and explicit backfill/repair command; it does not introduce a background daemon.

## 5. Public Series API

```http
GET /v1/usage/series?from=2026-07-20&to=2026-08-19&metrics=input_tokens,output_tokens&harnesses=codex,pi
```

### 5.1 Query contract

- `from` and `to` are inclusive UTC dates.
- Both dates must be present together. When both are omitted, the API returns the latest 30 UTC days including today.
- `from` must not follow `to`; the maximum range is 366 days.
- `metrics` is a comma-separated unique list from `input_tokens`, `output_tokens`, `total_tokens`, and `estimated_cost_usd`.
- Omitted `metrics` defaults to `input_tokens,output_tokens`.
- `harnesses` is an optional comma-separated unique list of supported harness IDs.
- Omitted `harnesses` combines all harnesses.
- Multiple harnesses are summed into one point per date.
- Missing dates are represented by zero token values. Cost is `null` when no selected bucket for that date has a known estimate; otherwise known costs are summed.
- Unknown parameters, duplicate values, empty values, invalid dates, unknown metrics, unknown harnesses, and oversized ranges return `400`.

### 5.2 Response

```json
{
  "schema_version": 1,
  "from": "2026-07-20",
  "to": "2026-08-19",
  "metrics": ["input_tokens", "output_tokens"],
  "harnesses": ["codex", "pi"],
  "points": [
    {
      "date": "2026-08-19",
      "input_tokens": 125000,
      "output_tokens": 18000
    }
  ]
}
```

When no harness filter is requested, `harnesses` is `null`. Point properties include only requested metrics plus `date`.

## 6. Rate Limiting

Rate limits protect application capacity, not identity or billing.

- Public series reads: token bucket per resolved client IP, default `120` requests per 60 seconds with burst `30`.
- Authenticated publication: token bucket per publish token, default `60` requests per 60 seconds with burst `20`.
- Limits are configurable with environment variables and enforced before storage or aggregation work.
- Only a configured trusted-proxy mode reads `X-Forwarded-For`; otherwise the socket peer address is authoritative. Trusted-proxy mode uses the first untrusted address according to the deployment's configured proxy hop count, preventing caller-controlled IP spoofing.
- A bounded idle-entry eviction pass prevents the in-memory limiter map from growing without bound.
- `429` responses include `Retry-After` and the same CORS policy as successful public reads.
- Health checks are not rate-limited.

The initial limiter is in-process. This is correct for the intended single API instance. A multi-replica deployment would require a shared limiter and is explicitly outside this design.

## 7. Caching

### 7.1 Server cache

- Canonical validated query parameters form the cache key, so equivalent query ordering shares an entry.
- Successful public series responses are cached in a bounded in-memory LRU for 60 seconds by default.
- Cache entries contain the serialized response body, `ETag`, and generation number.
- Every successful bucket mutation increments the generation and invalidates all prior series entries immediately.
- Invalid requests, authentication failures, write responses, and `429` responses are never cached.
- Cache capacity and TTL are configurable; capacity is bounded to prevent untrusted query combinations from causing unbounded memory growth.

### 7.2 HTTP cache contract

Public responses include:

```http
Cache-Control: public, max-age=60, stale-while-revalidate=300
ETag: "<content-hash>"
Vary: Origin
```

A matching `If-None-Match` returns `304 Not Modified` without a response body. After a successful upsert, newly computed content receives a new ETag. Publication responses use `Cache-Control: no-store`.

### 7.3 Component cache

The TypeScript client deduplicates concurrent requests with the same canonical URL and keeps a short in-memory response cache aligned with the server's `max-age`. Component instances can opt out with `cache="no-store"`; credentials are never cached because public reads require none.

## 8. CORS and Security

- `GET /v1/usage/series` permits `GET` and conditional request headers from configured origins.
- `TOKEN_USAGE_ALLOWED_ORIGINS` contains an explicit comma-separated allowlist, including `https://mintychochip.dev`; `*` is allowed only when deliberately configured.
- Publication permits no browser CORS access.
- Error bodies do not reveal configured tokens, filesystem paths, or internal parse details.
- Request bodies have a small fixed maximum size.
- All date ranges and lists are bounded before allocation.
- The bearer token must be supplied through deployment secrets.

## 9. TypeScript Package

Create a standalone package under `packages/token-usage-widget` with ESM output, declaration files, and explicit exports for the client, Web Component, and React wrapper. React is an optional peer dependency; importing the framework-independent client or element does not load React.

### 9.1 Typed client

```ts
const series = await fetchTokenUsageSeries({
  endpoint: "https://api.mintychochip.dev",
  range: "30d",
  metrics: ["input_tokens", "output_tokens"],
  harnesses: ["codex", "pi"],
  signal
});
```

The client supports either a range preset (`7d`, `30d`, `90d`, `1y`) or explicit `from`/`to`, validates mutually exclusive inputs, canonicalizes list ordering, validates the response schema, and preserves abort signals.

### 9.2 Web Component

```html
<token-usage-graph
  endpoint="https://api.mintychochip.dev"
  range="30d"
  metrics="input_tokens,output_tokens"
  harnesses="codex,pi"
  theme="auto"
></token-usage-graph>
```

Observed attributes:

- `endpoint`
- `range`, or `from` and `to`
- `metrics`
- `harnesses`
- `theme`: `light`, `dark`, or `auto`
- `cache`: `default` or `no-store`

Attribute changes abort an obsolete request and fetch the new query. The element dispatches `token-usage-load` and `token-usage-error` custom events.

### 9.3 React wrapper

```tsx
<TokenUsageGraph
  endpoint="https://api.mintychochip.dev"
  range="90d"
  metrics={["total_tokens", "estimated_cost_usd"]}
  harnesses={["codex", "pi"]}
  theme="auto"
  onLoad={setSeries}
  onError={reportError}
/>
```

The wrapper maps typed props to the Web Component and bridges its custom events. It does not duplicate fetching or graph rendering.

## 10. Graph Presentation

The component renders dependency-free inline SVG:

- responsive width with a stable minimum height;
- one line/area series per selected metric;
- separate left token axis and right USD axis when token and cost metrics are combined;
- abbreviated axis labels with exact values in tooltips;
- legend and aggregate totals;
- pointer hover and keyboard-focus inspection for each date;
- loading skeleton, empty state, and explicit error state with a retry control;
- reduced-motion compliance;
- semantic text summary and accessible SVG title/description;
- neutral light/dark defaults compatible with `mintychochip.dev`.

CSS custom properties control background, border, text, grid, series colors, radius, font, and height. Component styles are isolated in a shadow root. No charting runtime is added for this fixed graph surface.

## 11. Failure Handling

- Publication failures identify the failed bucket without exposing credentials and cause a non-zero CLI exit after remaining events are attempted.
- An older event returns `409`; the CLI reports it rather than silently replacing newer data.
- Corrupt aggregate storage prevents hosted aggregate routes from starting; it is never silently reset.
- Public malformed queries return stable JSON `400` errors.
- Component request failures retain no stale loading state, expose an accessible message, dispatch an error event, and allow retry.
- Request cancellation caused by attribute/prop changes is not shown as an error.

## 12. Verification

### Rust contract tests

- UTC daily aggregation and harness separation.
- Optional cost aggregation semantics.
- Authenticated insert, replacement, duplicate delivery, and stale-event rejection.
- Constant-time authentication path and missing configuration behavior.
- Atomic persistence and recovery from a pre-existing aggregate file.
- Date filling, metric selection, harness filtering, default query, and 366-day boundary.
- Every invalid query class returns `400` without excessive allocation.
- Read and write rate limits, `Retry-After`, client-IP derivation, and idle eviction.
- Cache hit, canonical key behavior, bounded eviction, mutation invalidation, ETag, and `304`.
- Public CORS allowlist and no publication CORS.

### TypeScript contract tests

- Query canonicalization and range conversion.
- Runtime response validation.
- Concurrent request deduplication, expiry, opt-out, and abort behavior.
- Attribute-to-query and React-prop-to-attribute mapping.
- Loading, empty, error, retry, and successful graph states.
- Keyboard interaction and accessible labeling.

### End-to-end smoke test

1. Launch the actual API with a temporary aggregate store and publish token.
2. Publish fixture buckets, including a duplicate and corrected event.
3. Query multiple metrics, ranges, and harness combinations; verify cache headers, conditional `304`, and rate limiting.
4. Render the Web Component and React wrapper against that API in a browser.
5. Exercise range/metric changes, tooltip focus, error/retry, light/dark themes, and narrow/desktop widths.
