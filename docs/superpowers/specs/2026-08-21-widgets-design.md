# Token Usage Widgets — Design Spec

> Status: draft
> Date: 2026-08-21
> Owner: token-usage

## Goal

Let any user show a public token-usage summary card on websites or in READMEs without a central usage database by default, while still offering a managed `widgets.mintychochip.dev` option for users who want the easiest deployment.

## Non-goals

- Store per-session data or prompts on `widgets.mintychochip.dev`
- Require a user account or password
- Query GitHub on the user’s behalf
- Make the central service the only option

## Design

### Identity

Each machine gets an ed25519 keypair stored in:

```text
~/.toktally/keys/identity.pub
~/.toktally/keys/identity.sec
```

- Generated automatically on first publish.
- The private key stays on the machine.
- A stable UUID is derived from the public key.
- The widget URL is `https://widgets.mintychochip.dev/u/<uuid>/usage-summary.json`.

### Authentication

Uploads are signed with the private key. The server verifies the signature against the stored public key. The signature uses a service-specific namespace to prevent cross-protocol replay.

```text
POST /api/v1/publish
{
  "uuid": "...",
  "public_key": "...",
  "summary": { ... },
  "signature": "...",
  "display_name": "..."
}
```

First upload creates the profile. Later uploads must be signed with the matching private key.

### Publish modes

1. **Central service (default)**

   ```bash
   token-usage-reporter publish --widgets
   ```

   Posts the aggregate summary to `widgets.mintychochip.dev`. The service stores and serves it.

2. **GitHub Pages (opt-out)**

   ```bash
   token-usage-reporter publish --github-pages
   ```

   Uses the existing `gh` CLI to push to `<user>/token-usage-pages`, served from `https://<user>.github.io/token-usage-pages/usage-summary.json`.

### Synchronization between modes

The local store is the single source of truth. The reporter computes one summary and writes it to every configured target in the same publish run.

```text
~/.token-usage/store.json
        │
        ▼
   token-usage-reporter publish
        │
        ├── widgets.mintychochip.dev
        └── github.io/token-usage-pages
```

- Target configuration lives in `~/.toktally/publish-config.json`.
- `token-usage-reporter publish` without flags publishes to all configured targets.
- `token-usage-reporter publish --widgets --github-pages` publishes to both once.
- A failure on one target does not roll back the others.
- Switching modes does not migrate history; the next publish writes the current local totals to the new target.

### Data stored centrally

Only the public aggregate:

```json
{
  "schema": 1,
  "uuid": "...",
  "display_name": "...",
  "generated_at": "...",
  "totals": {
    "input_tokens": 1234567,
    "output_tokens": 456789,
    "cache_read_tokens": 987654
  },
  "estimated_cost_usd": 12.34
}
```

No session IDs, no harness metadata, no prompts, no per-session breakdowns.

### Widget component

```html
<token-usage-card
  uuid="..."
  src="(optional override)">
</token-usage-card>
```

- With `uuid`, the component fetches from `widgets.mintychochip.dev/u/<uuid>/usage-summary.json`.
- With `src`, it fetches from the user-provided URL (GitHub Pages, object storage, etc.).
- The component shows `generated_at` as freshness and explicit error states.

### Multi-machine

Copy `~/.toktally/keys/` to another machine to reuse the same UUID. Otherwise each machine publishes under its own UUID.

### Security

- Ed25519 signatures with a service-specific namespace.
- TLS for all requests.
- Server verifies signatures, never fetches arbitrary URLs.
- Only aggregate data stored.
- Rate limiting per UUID.
- No session or credential data in the public summary.

### Scope impact

This changes the current living spec. The following items move from out-of-scope/future to in-scope for the widgets service:

- Optional central widget hosting on `widgets.mintychochip.dev`
- Auth by public key (no multi-user tenancy or database of sessions)
- Object-store/public-URL alternative for privacy-conscious users

`api.mintychochip.dev` stays stateless as documented.

## Open questions

- Should the server support profile deletion and re-keying?
- Should GitHub Pages publish automatically create the repo or require the user to create it first?
- Should the widget support theming (light/dark) out of the box?
