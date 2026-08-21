# Widget Publish — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the reporter to publish a signed aggregate summary to a widget service, and add a minimal in-repo widget API binary for local testing.

**Architecture:** `token-usage-reporter publish --widgets --url <service>` computes the public summary, signs it with the machine identity, and POSTs it. `crates/widgets-api` is a tiny Axum service that verifies the signature and serves `/u/<uuid>/usage-summary.json`. In this first version the service stores summaries in a local directory (one JSON file per UUID).

**Tech Stack:** Rust, `ureq` for client HTTP, `axum` for service (reused from `crates/cli`), `ed25519-dalek`, existing `token-usage-cli` summary and identity modules.

## Global Constraints

- Client must not send raw sessions; only the aggregate summary.
- Signature uses the same canonical JSON as `identity::verify_json`.
- Service must verify every POST with the stored public key.
- Service must reject unknown UUIDs on POST (no first-upload key replacement after creation).
- All paths use `~/.toktally/server/store` by default.
- Tests must run without network.

---

### Task 1: Add `publish --widgets` CLI option

**Files:**
- Modify: `crates/cli/src/bin/token-usage-reporter.rs`
- Modify: `crates/cli/src/lib.rs` (re-export new module)
- Modify: `crates/cli/Cargo.toml` (if new crate split; otherwise keep in cli)

**Interfaces:**
- Consumes: `Command::Publish` gains `widgets: bool` and `url: Option<String>` already exists.
- Produces: `token_usage_cli::widgets_publish::publish_to_widgets(...)`.

- [ ] **Step 1: Update `Publish` variant**

Add to `Command::Publish`:
```rust
/// Publish the summary to the widgets.mintychochip.dev service.
#[arg(long, conflicts_with = "dir", conflicts_with = "gist")]
widgets: bool,
```

- [ ] **Step 2: Create `crates/cli/src/widgets_publish.rs`**

```rust
use serde_json::json;
use std::path::Path;

use token_usage_cli::{
    identity::{load_or_generate, verify_json},
    summarize_priced,
};
use token_usage_store::FileStore;

pub fn publish_to_widgets(
    store: &FileStore,
    service_url: &str,
    prices: Option<&token_usage_cli::PriceTable>,
) -> Result<String, String> {
    let id = load_or_generate().map_err(|e| format!("identity: {e}"))?;
    let listed = store.list().map_err(|e| e.to_string())?;
    let summary = summarize_priced(&listed, unix_now(), prices);

    let body = json!({
        "uuid": id.uuid,
        "public_key": base64::engine::general_purpose::STANDARD.encode(&id.public_key),
        "summary": summary,
        "signature": "",
        "display_name": std::env::var("TOKTALLY_DISPLAY_NAME").ok(),
    });

    // placeholder; will sign in Task 2
    Ok(format!("{service_url}/u/{}/usage-summary.json", id.uuid))
}
```

- [ ] **Step 3: Wire it in `run()`**

In the `Command::Publish` match arm, after the existing `dir` and `gist` branches:
```rust
if widgets {
    let prices = token_usage_cli::load_price_table(&store_path);
    let url = publish_to_widgets(&store, url.as_deref().unwrap_or("https://widgets.mintychochip.dev"), prices.as_ref())?;
    println!("{url}");
    return Ok(());
}
```

- [ ] **Step 4: Add a smoke test that accepts the new flag**

Create `crates/cli/tests/widgets_publish.rs`:
```rust
use std::path::Path;

#[test]
fn publish_widgets_flag_is_recognized() {
    // run `token-usage-reporter publish --widgets --url http://127.0.0.1:0` and expect an error
    // because the service is not running; this only proves the CLI accepts the flag.
}
```

- [ ] **Step 5: Commit**

```bash
git add crates/cli/src/widgets_publish.rs crates/cli/src/lib.rs crates/cli/src/bin/token-usage-reporter.rs
git commit -m "feat(cli): add --widgets publish flag and module"
```

---

### Task 2: Sign and POST the summary

**Files:**
- Modify: `crates/cli/src/widgets_publish.rs`
- Modify: `crates/cli/Cargo.toml`

**Interfaces:**
- Consumes: `identity::Identity::sign_json`.
- Produces: `publish_to_widgets` that actually POSTs.

- [ ] **Step 1: Implement signing in `widgets_publish.rs`**

```rust
let mut body = json!({
    "uuid": id.uuid,
    "public_key": base64::engine::general_purpose::STANDARD.encode(&id.public_key),
    "summary": summary,
    "display_name": std::env::var("TOKTALLY_DISPLAY_NAME").ok(),
});
let signature = id.sign_json(&body["summary"])?;
body["signature"] = json!(base64::engine::general_purpose::STANDARD.encode(&signature));
```

- [ ] **Step 2: POST with `ureq`**

```rust
let url = format!("{service_url}/api/v1/publish");
let response = ureq::post(&url)
    .send_json(&body)
    .map_err(|e| e.to_string())?;
let status = response.status();
if status != 200 {
    return Err(format!("widgets publish failed: HTTP {status}"));
}
```

- [ ] **Step 3: Add tests against a local mock server**

Use a thread-local or external test helper that spins an `axum` test app accepting the POST and returning 200. Verify the body has `uuid`, `public_key`, `summary`, `signature`.

- [ ] **Step 4: Commit**

```bash
git add crates/cli/src/widgets_publish.rs crates/cli/tests/widgets_publish.rs
git commit -m "feat(cli): sign and POST widget summaries"
```

---

### Task 3: Add `crates/widgets-api` service

**Files:**
- Create: `crates/widgets-api/Cargo.toml`
- Create: `crates/widgets-api/src/main.rs`
- Modify: root `Cargo.toml` workspace members

**Interfaces:**
- Consumes: `ed25519-dalek`, `axum`, `serde_json`, `tokio`.
- Produces: `token-usage-widgets-api` binary with two routes.

- [ ] **Step 1: Create crate files**

`crates/widgets-api/Cargo.toml`:
```toml
[package]
name = "token-usage-widgets-api"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
axum.workspace = true
tokio = { workspace = true, features = ["rt-multi-thread", "net", "macros"] }
serde = { workspace = true, features = ["derive"] }
serde_json.workspace = true
ed25519-dalek = { version = "2.1.1", features = ["rand_core"] }
base64 = "0.22.1"
blake3 = "1.5.1"
```

`crates/widgets-api/src/main.rs`:
```rust
use axum::{
    extract::Path,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct AppState {
    summaries: Arc<Mutex<HashMap<String, Value>>>,
    public_keys: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

async fn publish(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(body): Json<Value>,
) -> StatusCode {
    let Some(uuid) = body.get("uuid").and_then(|v| v.as_str()) else {
        return StatusCode::BAD_REQUEST;
    };
    let Some(public_key_b64) = body.get("public_key").and_then(|v| v.as_str()) else {
        return StatusCode::BAD_REQUEST;
    };
    let Some(summary) = body.get("summary") else {
        return StatusCode::BAD_REQUEST;
    };
    let Some(signature_b64) = body.get("signature").and_then(|v| v.as_str()) else {
        return StatusCode::BAD_REQUEST;
    };

    let public_key = base64::engine::general_purpose::STANDARD
        .decode(public_key_b64)
        .unwrap_or_default();
    let signature = base64::engine::general_purpose::STANDARD
        .decode(signature_b64)
        .unwrap_or_default();

    // verify signature using token_usage_cli::identity::verify_json
    if !token_usage_cli::identity::verify_json(summary, &public_key, &signature).unwrap_or(false) {
        return StatusCode::UNAUTHORIZED;
    }

    let mut public_keys = state.public_keys.lock().unwrap();
    if let Some(stored) = public_keys.get(uuid) {
        if stored != &public_key {
            return StatusCode::FORBIDDEN;
        }
    } else {
        public_keys.insert(uuid.to_string(), public_key);
    }

    let mut summaries = state.summaries.lock().unwrap();
    summaries.insert(uuid.to_string(), summary.clone());

    StatusCode::OK
}

async fn get_summary(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let summaries = state.summaries.lock().unwrap();
    summaries.get(&uuid).cloned().map(Json).ok_or(StatusCode::NOT_FOUND)
}

#[tokio::main]
async fn main() {
    let state = AppState::default();
    let app = Router::new()
        .route("/api/v1/publish", post(publish))
        .route("/u/:uuid/usage-summary.json", get(get_summary))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:9474").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

- [ ] **Step 2: Add crate to workspace**

Root `Cargo.toml` members:
```toml
members = [
    "crates/domain",
    "crates/store",
    "crates/adapters",
    "crates/sync",
    "crates/cli",
    "crates/widgets-api",
]
```

- [ ] **Step 3: Build and smoke test**

Run:
```bash
cargo run -p token-usage-widgets-api &
TOKEN_USAGE_STORE=/tmp/test-store cargo run -p token-usage-cli --bin token-usage-reporter -- publish --widgets --url http://127.0.0.1:9474
curl http://127.0.0.1:9474/u/<uuid>/usage-summary.json
```

- [ ] **Step 4: Commit**

```bash
git add crates/widgets-api Cargo.toml
git commit -m "feat(widgets-api): add minimal publish and summary service"
```

---

## Spec Coverage

| Spec requirement | Task |
|---|---|
| `publish --widgets` command | Task 1 |
| Signed summary upload | Task 2 |
| Widget service `/u/<uuid>/usage-summary.json` | Task 3 |
| Verification of public key on POST | Task 3 |
