# Store and API Security Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restrict the unauthenticated API to loopback and make JSON-store initialization and updates safe across concurrent processes.

**Architecture:** Validate addresses inside both exported serving functions before opening files or sockets. Coordinate every store operation through a stable sidecar file lock, and replace the JSON file using a unique same-directory temporary file with explicit durability syncs.

**Tech Stack:** Rust 2021, Tokio, Axum, `fs2`, `tempfile`, Cargo integration tests.

## Global Constraints

- `serve` and `serve_stateless` always reject non-loopback addresses with `io::ErrorKind::InvalidInput`; no override exists.
- Validation happens before stateful store creation and before socket binding.
- `FileStore::open` initialization and every operation use the same stable sidecar inter-process lock.
- Lock acquisition and release are RAII; all operations acquire the in-process mutex before the file lock.
- Writes use unique temporary files in the destination directory, sync file contents, atomically replace the destination, and sync the parent directory on supported targets.
- Existing JSON format remains unchanged; no repair, authentication, SQLite migration, or unrelated refactor is included.

---

### Task 1: Enforce Loopback-Only Serving

**Files:**
- Modify: `crates/cli/src/lib.rs`
- Test: `crates/cli/tests/api.rs`

**Interfaces:**
- Consumes: existing `serve(PathBuf, SocketAddr) -> Result<(), io::Error>` and `serve_stateless(SocketAddr) -> Result<(), io::Error>`.
- Produces: private `validate_bind_addr(addr: SocketAddr) -> Result<(), io::Error>` used by both exported functions.

- [ ] **Step 1: Add failing stateful and stateless rejection tests**

Add Tokio tests that call each exported function with wildcard addresses and assert immediate `InvalidInput`. For the stateful test, use a nonexistent store path and assert it remains nonexistent:

```rust
use token_usage_cli::{app, serve, serve_stateless, ApiState, WireObservation};

#[tokio::test]
async fn serve_rejects_non_loopback_before_creating_store() {
    let dir = tempdir().unwrap();
    let store = dir.path().join("must-not-exist.json");
    let err = serve(store.clone(), "0.0.0.0:0".parse().unwrap())
        .await
        .unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(!store.exists());
}

#[tokio::test]
async fn serve_stateless_rejects_ipv6_wildcard() {
    let err = serve_stateless("[::]:0".parse().unwrap())
        .await
        .unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}
```

- [ ] **Step 2: Run the targeted tests and confirm the security contract fails**

Run:

```bash
cargo test -p token-usage-cli --test api serve_rejects_non_loopback_before_creating_store
cargo test -p token-usage-cli --test api serve_stateless_rejects_ipv6_wildcard
```

Expected: tests do not complete with `InvalidInput`; the stateful call may create the store or either function may begin serving.

- [ ] **Step 3: Validate at the exported serving boundary**

Add the private validator and invoke it as the first statement of each serve function:

```rust
fn validate_bind_addr(addr: SocketAddr) -> Result<(), std::io::Error> {
    if addr.ip().is_loopback() {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("refusing unauthenticated non-loopback bind address {addr}"),
        ))
    }
}

pub async fn serve(store_path: PathBuf, addr: SocketAddr) -> Result<(), std::io::Error> {
    validate_bind_addr(addr)?;
    // existing store-open, bind, and serve flow
}

pub async fn serve_stateless(addr: SocketAddr) -> Result<(), std::io::Error> {
    validate_bind_addr(addr)?;
    // existing bind and serve flow
}
```

- [ ] **Step 4: Verify rejection and existing loopback behavior**

Run:

```bash
cargo test -p token-usage-cli --test api
```

Expected: all API integration tests pass, including wildcard rejection and the existing `127.0.0.1:0` binary round-trip.

- [ ] **Step 5: Commit the API security unit**

```bash
git add crates/cli/src/lib.rs crates/cli/tests/api.rs
git diff --cached --check
git commit -m "Restrict the API server to loopback"
```

---

### Task 2: Serialize Store Initialization and Transactions Across Processes

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/store/Cargo.toml`
- Modify: `crates/store/src/lib.rs`
- Test: `crates/store/tests/store.rs`
- Create: `crates/store/src/bin/store-test-worker.rs`

**Interfaces:**
- Consumes: `FileStore::open`, `FileStore::ingest_at`, and the existing JSON schema.
- Produces: a private stable lock path and RAII exclusive lock guard used for initialization, reads, and mutations; test-only worker commands `open` and `ingest`.

- [ ] **Step 1: Add a process worker for deterministic integration tests**

Create a small binary that accepts `open <store-path>` or `ingest <store-path> <session-id>`, opens the real `FileStore`, and ingests a distinct Codex observation for the second command. Use `UsageObservation::new`, `ObservationIdentity::new`, `SessionId::parse`, `UsageCounts::new`, `ObservationSource::PluginReport`, and `SessionStoreCompleteness::Complete`; exit nonzero on any error. This binary is production-buildable but exists only to drive real cross-process behavior without mocking the store.

- [ ] **Step 2: Add failing concurrent-process tests**

In `crates/store/tests/store.rs`, use `env!("CARGO_BIN_EXE_store-test-worker")`, `std::process::Command`, and a `Barrier`-coordinated pair of launcher threads. Add:

```rust
#[test]
fn concurrent_first_openers_create_valid_store() {
    // Launch two `open` workers against the same nonexistent path at once.
    // Assert both statuses succeed and FileStore::open(path).list() is empty.
}

#[test]
fn concurrent_process_ingests_preserve_both_observations() {
    // Initialize once, launch two `ingest` workers with distinct session IDs,
    // assert both statuses succeed, and assert both identities are present.
}
```

Repeat the ingest race enough times in one test to make the current lost-update window observable while keeping the test deterministic through simultaneous process release. Assert the final file parses through `FileStore`, and assert `path.with_extension("json.tmp")` does not exist.

- [ ] **Step 3: Run the process tests and confirm the current implementation fails**

Run:

```bash
cargo test -p token-usage-store --test store concurrent_ -- --nocapture
```

Expected: at least one current implementation failure: a worker rename error, failed first-open, or a missing observation after concurrent ingests.

- [ ] **Step 4: Add established locking and temporary-file dependencies**

Add workspace dependency:

```toml
fs2 = "0.4"
```

Update `crates/store/Cargo.toml`:

```toml
[dependencies]
fs2.workspace = true
tempfile.workspace = true
```

Keep `tempfile` in dev-dependencies only if tests still import it directly through that section; Cargo accepts the same package in both sections, but prefer one normal dependency when integration tests inherit it transitively only through explicit test dependencies.

- [ ] **Step 5: Introduce the stable sidecar lock and RAII acquisition**

Extend `FileStore` with `lock_path: PathBuf`. Derive it without changing the store extension, for example by appending `.lock` to the full filename so `store.json` maps to `store.json.lock`.

Add a helper that opens the sidecar without truncation and takes an exclusive OS lock:

```rust
use fs2::FileExt;
use std::fs::{File, OpenOptions};

fn lock_exclusive(path: &Path) -> Result<File, StoreError> {
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(path)?;
    file.lock_exclusive()?;
    Ok(file)
}
```

The returned `File` is the RAII guard. Do not explicitly delete the sidecar and do not truncate it.

- [ ] **Step 6: Lock initialization and every store operation consistently**

In `FileStore::open`, create the parent directory, derive the sidecar path, acquire its exclusive lock, then check and initialize the store while the guard remains in scope. Store `lock_path` in `FileStore`.

For every public operation, preserve one order:

```rust
let _thread_guard = self.lock.lock().expect("store lock");
let _process_guard = lock_exclusive(&self.lock_path)?;
```

Mutations keep both guards through `load`, modification, and `write_atomic`. Reads keep both through `load` and result construction. Never acquire these locks in reverse order.

- [ ] **Step 7: Replace the fixed temporary pathname with durable unique replacement**

Use `tempfile::NamedTempFile::new_in(parent)` or `tempfile::Builder` in the destination directory. Serialize before or after creation, write through `as_file_mut`, append the newline, flush, and call `sync_all`. Persist over the destination using the crate API appropriate for replacing an existing file on supported platforms; map `PersistError` to its underlying `io::Error`.

After replacement, sync the parent directory on Unix:

```rust
#[cfg(unix)]
fn sync_parent(path: &Path) -> Result<(), std::io::Error> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    File::open(parent)?.sync_all()
}

#[cfg(not(unix))]
fn sync_parent(_path: &Path) -> Result<(), std::io::Error> {
    Ok(())
}
```

Call `sync_parent(path)?` only after successful replacement. The non-Unix branch is an explicit platform gate, not an ignored runtime failure.

- [ ] **Step 8: Verify process safety and all store behavior**

Run:

```bash
cargo test -p token-usage-store
```

Expected: all store unit/integration tests pass; concurrent first-open and ingest workers exit successfully, both observations survive, and JSON remains readable.

- [ ] **Step 9: Commit the process-safe store unit**

```bash
git add Cargo.toml Cargo.lock crates/store/Cargo.toml crates/store/src/lib.rs crates/store/src/bin/store-test-worker.rs crates/store/tests/store.rs
git diff --cached --check
git commit -m "Make file store updates process-safe"
```

---

### Task 3: Workspace Verification and Security Review

**Files:**
- Modify only if verification exposes a defect directly caused by Tasks 1 or 2.

**Interfaces:**
- Consumes: loopback-only serving and process-safe store implementation.
- Produces: evidence that the complete workspace preserves its observable contracts and a repository security-review report.

- [ ] **Step 1: Run formatting and workspace tests**

Run:

```bash
cargo fmt --all -- --check
cargo test --workspace
```

Expected: both commands exit successfully. If formatting fails, run `cargo fmt --all`, inspect the resulting diff, and rerun both commands.

- [ ] **Step 2: Run workspace compilation and dependency checks**

Run:

```bash
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: both commands exit successfully with no warnings promoted to errors.

- [ ] **Step 3: Smoke-test the shipped API boundary**

Run the API binary once with `TOKEN_USAGE_BIND=0.0.0.0:0` and a temporary store path. Expected: exit status `1`, stderr contains the non-loopback refusal, and the store file does not exist. Then run the existing API integration test to exercise a real loopback listener:

```bash
cargo test -p token-usage-cli --test api api_binary_roundtrip_returns_submitted_identity_and_counts
```

Expected: PASS.

- [ ] **Step 4: Review security-sensitive repository boundaries**

Inspect filesystem path construction, subprocess execution, HTTP inputs, publishing/import flows, secret handling, and dependencies. Report only reproducible findings with exact file/line evidence and severity. Confirm candidate findings against actual call sites and tests; distinguish verified defects from hardening suggestions.

- [ ] **Step 5: Review final commit boundaries**

Run:

```bash
git status --short
git log -3 --oneline
git show --stat --oneline HEAD~1..HEAD
```

Expected: no accidental uncommitted files; API and store fixes remain separate atomic commits with matching tests. Any security-review-only code fix becomes its own tested commit rather than being folded into either completed unit.
