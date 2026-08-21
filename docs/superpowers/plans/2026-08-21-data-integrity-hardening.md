# Data-Integrity Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the four confirmed data-loss/data-integrity findings from the repo review: cross-process store races, premature harness-sync marking, silent summary undercount, and metadata wipe on re-ingest.

**Architecture:** All four fixes stay inside existing crates. The store gains an OS-level advisory file lock (`fs4` flock wrapper) around its load→mutate→persist critical sections; sync stops recording a completed scan when discovered content all failed adaptation; summary only lets `SessionStoreCompleteness::Complete` plugin evidence veto harness-global approximations; the store merge preserves `model`/`recorded_at` that the incoming payload omitted.

**Tech Stack:** Rust 2021 workspace, `fs4 = "0.13"` (flock/LockFileEx), existing dev-deps (tempfile, serde_json).

## Global Constraints

- Do NOT commit or reformat the unrelated uncommitted refactor (`crates/web/**`, `crates/cli/src/lib.rs`, `crates/cli/src/wire.rs` deletion, `crates/cli/src/bin/*`, `crates/cli/tests/api.rs`, root/Cli `Cargo.toml`). Stage only files this plan touches.
- TDD: every task writes its failing test first and shows the failure before implementing.
- Repo commit style: conventional commits (`fix(store): …`), one commit per task.
- Verification per task: `cargo test -p <crate>` green; final: `cargo test --workspace && cargo clippy --workspace --all-targets` exit 0.
- Store file format unchanged (version 2 JSON); `.lock` sidecar file appears next to `store.json` (documented in module doc).

---

### Task 1: Cross-process store lock (finding #1)

**Files:**
- Modify: `crates/store/src/lib.rs`
- Modify: `crates/store/Cargo.toml` (add `fs4.workspace = true`)
- Modify: root `Cargo.toml` `[workspace.dependencies]` (add `fs4 = "0.13"`); NOTE this file carries an unrelated staged-free hunk (web member lines) — append only, stage whole file is NOT allowed; instead put fs4 in `crates/store/Cargo.toml` directly as `fs4 = "0.13"` and leave root untouched.
- Test: `crates/store/tests/store.rs`

**Interfaces:**
- Consumes: existing `FileStore::ingest_at/bulk_ingest/record_harness_sync/get/list` signatures (unchanged).
- Produces: private `fn write_locked<T>(&self, f: impl FnOnce(&mut StoreFile) -> Result<T, StoreError>) -> Result<T, StoreError>` and `fn read_locked<T>(&self, f: impl FnOnce(&StoreFile) -> T) -> Result<T, StoreError>`; lock sidecar path = `<store>.lock`; all public signatures unchanged.

- [ ] **Step 1: Write failing tests** in `crates/store/tests/store.rs`:

```rust
#[test]
fn ingest_blocks_while_another_process_holds_the_store_lock() {
    let (_dir, store) = open_store();
    let lock_path = dir_lock_path(&store);
    let mut guard = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .unwrap();
    fs4::FileExt::lock_exclusive(&guard).unwrap();

    let handle = std::thread::spawn(move || {
        store.ingest(observation(
            Harness::ClaudeCode, "locked-1", 5, 6,
            ObservationSource::PluginReport,
            SessionStoreCompleteness::Complete,
        )).unwrap();
    });
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(
        !handle.is_finished(),
        "ingest must block while the lock file is held by another writer"
    );
    fs4::FileExt::unlock(&mut guard).unwrap();
    handle.join().unwrap();
}

#[test]
fn parallel_store_instances_on_one_path_do_not_lose_updates() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("store.json");
    let handles: Vec<_> = (0..8)
        .map(|worker| {
            let path = path.clone();
            std::thread::spawn(move || {
                let store = FileStore::open(path).unwrap();
                for i in 0..25 {
                    store.ingest(observation(
                        Harness::ClaudeCode,
                        &format!("w{worker}-s{i}"), 1, 1,
                        ObservationSource::PluginReport,
                        SessionStoreCompleteness::Complete,
                    )).unwrap();
                }
            })
        })
        .collect();
    for h in handles { h.join().unwrap(); }
    let store = FileStore::open(path).unwrap();
    assert_eq!(store.list().unwrap().len(), 200, "no update may be lost across instances");
}

/// Path of the lock sidecar: `<store>.lock`.
fn dir_lock_path(store: &FileStore) -> std::path::PathBuf { store.path().with_extension("json.lock") }
```

Requires adding `pub fn path(&self) -> &Path` accessor to `FileStore` (one line, tested implicitly here).

- [ ] **Step 2: Run to verify RED**

Run: `cargo test -p toktally-store`
Expected: FAIL — `ingest_blocks_while_another_process_holds_the_store_lock` ("must block") because no lock exists yet; second test may pass by luck on fast machines (retry/bump iterations if so; the blocking test is the deterministic red).

- [ ] **Step 3: Implement** in `crates/store/src/lib.rs`:

```rust
use fs4::FileExt;

impl FileStore {
    /// Path of the advisory lock sidecar guarding cross-process access.
    fn lock_path(&self) -> PathBuf {
        let mut s = self.path.as_os_str().to_os_string();
        s.push(".lock");
        PathBuf::from(s)
    }

    /// Run `f` with an exclusive interprocess lock over the store file.
    fn write_locked<T>(
        &self,
        f: impl FnOnce(&mut StoreFile) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let lock = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(self.lock_path())?;
        lock.lock_exclusive()?;
        let outcome = {
            let mut file = self.load()?;
            let result = f(&mut file);
            if result.is_ok() {
                write_atomic(&self.path, &file)?;
            }
            result
        };
        drop(lock); // release flock before returning
        outcome
    }

    /// Run `f` with a shared interprocess lock over the store file.
    fn read_locked<T>(&self, f: impl FnOnce(&StoreFile) -> T) -> Result<T, StoreError> {
        let lock = fs::OpenOptions::new()
            .read(true)
            .open(self.lock_path())?;
        lock.lock_shared()?;
        let outcome = Ok(f(&self.load()?));
        drop(lock);
        outcome
    }
}
```

Rewrite mutators to route through `write_locked` (bodies become closures: find-by-identity replace/push logic unchanged); rewrite `get`/`list`/`harness_last_synced`/`list_harness_syncs` through `read_locked`. Remove the now-unused `Mutex<()>` field. Add `pub fn path(&self) -> &Path { &self.path }`. Update module doc: persistence guarded by a `<path>.lock` advisory file lock; sidecar is harmless if left behind.

- [ ] **Step 4: Verify GREEN**

Run: `cargo test -p toktally-store`
Expected: PASS, all prior store tests intact (mutex removal must not break same-thread semantics).

- [ ] **Step 5: Commit**

```bash
git add crates/store/src/lib.rs crates/store/Cargo.toml crates/store/tests/store.rs Cargo.lock
git commit -m "fix(store): serialize cross-process access with a lock-file flock"
```

---

### Task 2: Don't mark harness synced when all discovered payloads fail (finding #2)

**Files:**
- Modify: `crates/sync/src/lib.rs:66-75` (`sync_harness`)
- Test: `crates/sync/tests/sync.rs`

**Interfaces:**
- Consumes: `Harness::Grok`, `SyncRoots { home }`, fixture layout `.grok/sessions/<dir>/signals.json` (session id auto-injected from directory name).
- Produces: `sync_harness` contract change — `record_harness_sync` runs only when `ingested > 0 || skipped == 0`. Poison fixture shape: signals.json containing `{"boom": true}` passes discovery but fails `adapt_grok` (no usage/context fields).

- [ ] **Step 1: Write failing test** in `crates/sync/tests/sync.rs`:

```rust
#[test]
fn sync_harness_leaves_needs_first_sync_true_when_every_payload_fails_adaptation() {
    let home = tempdir().unwrap();
    let sess = home.path().join(".grok/sessions/proj/sess-poison");
    std::fs::create_dir_all(&sess).unwrap();
    std::fs::write(sess.join("signals.json"), r#"{"boom": true}"#).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let store = FileStore::open(dir.path().join("store.json")).unwrap();
    let roots = SyncRoots { home: home.path().to_path_buf() };

    let report = sync_harness(&store, Harness::Grok, &roots, 42).unwrap();
    assert_eq!(report.ingested, 0);
    assert_eq!(report.skipped, 1);
    assert!(
        store.needs_first_sync(Harness::Grok).unwrap(),
        "failed scans must not permanently blackhole the harness"
    );
}
```

(Adapt existing imports/helpers in that file to match its current style.)

- [ ] **Step 2: Verify RED**

Run: `cargo test -p toktally-sync`
Expected: FAIL on `needs_first_sync == true` (currently recorded unconditionally at line 75).

- [ ] **Step 3: Implement** — replace `store.record_harness_sync(harness, last_synced_at)?;` (line 75) with:

```rust
    // Record progress only when something was ingested or nothing failed;
    // a fully-failed scan must remain retryable.
    if ingested > 0 || skipped == 0 {
        store.record_harness_sync(harness, last_synced_at)?;
    }
```

- [ ] **Step 4: Verify GREEN**

Run: `cargo test -p toktally-sync && cargo test -p toktally-cli`
Expected: PASS — including existing happy-path sync tests (empty discovery still records; web `list_scans_remaining…` regression unaffected since fixtures adapt cleanly).

- [ ] **Step 5: Commit**

```bash
git add crates/sync/src/lib.rs crates/sync/tests/sync.rs
git commit -m "fix(sync): keep failed-discovery harnesses eligible for rescan"
```

---

### Task 3: Only Complete plugin evidence vetoes harness-global approximations (finding #3)

**Files:**
- Modify: `crates/cli/src/summary.rs:225-242` (`observations_for_summary`)
- Test: `crates/cli/tests/summary.rs` if present, else `crates/cli/tests/reporter.rs`

**Interfaces:**
- Consumes: `UsageObservation::{source, completeness}`, `ObservationSource::{PluginReport, HarnessGlobalApproximation}`, `SessionStoreCompleteness::Complete`.
- Produces: rule — a harness-global approximation is suppressed only when at least one `PluginReport` row **with `completeness == Complete`** exists for that harness. Partial/Unknown plugin rows no longer veto.

- [ ] **Step 1: Write failing tests** (place per existing summary-test location):

```rust
use toktally_cli::summarize;
use toktally_domain::{
    ExtraCounts, Harness, ObservationIdentity, ObservationSource, SessionId,
    SessionStoreCompleteness, UsageCounts, UsageObservation,
};

fn obs(harness: Harness, session: &str, input: u64, output: u64,
       source: ObservationSource, completeness: SessionStoreCompleteness) -> UsageObservation {
    UsageObservation::new(
        ObservationIdentity::new(harness, SessionId::parse(session).unwrap()),
        UsageCounts::new(input, output),
        source,
        completeness,
    )
}

#[test]
fn global_approximation_survives_partial_plugin_evidence() {
    let rows = vec![
        obs(Harness::Grok, "__grok_global__", 1000, 500,
            ObservationSource::HarnessGlobalApproximation, SessionStoreCompleteness::Partial),
        obs(Harness::Grok, "sess-a", 10, 5,
            ObservationSource::PluginReport, SessionStoreCompleteness::Partial),
    ];
    let summary = summarize(&rows);
    assert_eq!(summary.input_tokens, 1010, "partial session evidence must not discard global totals");
}

#[test]
fn global_approximation_still_yields_to_complete_plugin_evidence() {
    let rows = vec![
        obs(Harness::Grok, "__grok_global__", 1000, 500,
            ObservationSource::HarnessGlobalApproximation, SessionStoreCompleteness::Partial),
        obs(Harness::Grok, "sess-b", 10, 5,
            ObservationSource::PluginReport, SessionStoreCompleteness::Complete),
    ];
    let summary = summarize(&rows);
    assert_eq!(summary.input_tokens, 10, "complete session store supersedes the approximation");
}
```

Confirm `summarize` signature/field names against `crates/cli/src/summary.rs` before writing (adjust test, not production API).

- [ ] **Step 2: Verify RED**

Run: `cargo test -p toktally-cli summary` (or the chosen test target)
Expected: first test FAILS with 1010 != 10 (current filter vetoes on bare `has_plugin`).

- [ ] **Step 3: Implement** — replace `observations_for_summary` body:

```rust
fn observations_for_summary(observations: &[UsageObservation]) -> Vec<&UsageObservation> {
    // Only a *complete* per-session view supersedes the host-wide approximation;
    // partial session evidence must not silently shrink reported totals.
    let mut has_complete = Vec::new();
    for obs in observations {
        if obs.source() == ObservationSource::PluginReport
            && obs.completeness() == SessionStoreCompleteness::Complete
        {
            let harness = obs.identity().harness();
            if !has_complete.contains(&harness) {
                has_complete.push(harness);
            }
        }
    }
    observations
        .iter()
        .filter(|obs| {
            obs.source() != ObservationSource::HarnessGlobalApproximation
                || !has_complete.contains(&obs.identity().harness())
        })
        .collect()
}
```

- [ ] **Step 4: Verify GREEN**

Run: `cargo test -p toktally-cli`
Expected: PASS incl. existing reporter/chart tests (fixtures emitting Complete rows keep old suppression).

- [ ] **Step 5: Commit**

```bash
git add crates/cli/src/summary.rs <test file>
git commit -m "fix(summary): require complete session evidence to drop global totals"
```

---

### Task 4: Preserve omitted metadata on same-identity re-ingest (finding #4)

**Files:**
- Modify: `crates/store/src/lib.rs` (merge sites: former lines 75 and 92-99, post-Task-1 inside `write_locked` closures)
- Test: `crates/store/tests/store.rs`

**Interfaces:**
- Consumes: `UsageObservation::{with_model, model, recorded_at}` accessors.
- Produces: private `fn merge_observation(existing: &UsageObservation, incoming: UsageObservation) -> UsageObservation` — replaces counts/source/completeness wholesale, fills `model`/`recorded_at` from `existing` when incoming omits them (`None`), leaves `last_synced_at` to the caller's stamping.

- [ ] **Step 1: Write failing test** in `crates/store/tests/store.rs`:

```rust
#[test]
fn reingest_without_metadata_preserves_stored_model_and_recorded_at() {
    let (_dir, store) = open_store();
    let rich = observation(
        Harness::Pi, "meta-sess", 100, 20,
        ObservationSource::PluginReport, SessionStoreCompleteness::Complete,
    )
    .with_recorded_at(1_700_000_000)
    .with_model("pi/gpt-9");
    store.ingest(rich).unwrap();

    let bare = observation(
        Harness::Pi, "meta-sess", 150, 30,
        ObservationSource::PluginReport, SessionStoreCompleteness::Complete,
    );
    store.ingest(bare).unwrap();

    let loaded = store.get(&identity(Harness::Pi, "meta-sess")).unwrap().unwrap();
    assert_eq!(loaded.counts().input_tokens(), 150, "totals still take the newest value");
    assert_eq!(loaded.model(), Some("pi/gpt-9"));
    assert_eq!(loaded.recorded_at(), Some(1_700_000_000));
}
```

(`observation` helper returns builder-ready `UsageObservation`; extend it only if chaining requires ownership tweaks.)

- [ ] **Step 2: Verify RED**

Run: `cargo test -p toktally-store reingest_without_metadata`
Expected: FAIL — model/recorded_at are `None` after the second ingest (full replacement).

- [ ] **Step 3: Implement** in `crates/store/src/lib.rs`:

```rust
/// Merge `incoming` over `existing`: counts and classification take the new
/// value; optional metadata the new payload omitted is carried forward.
fn merge_observation(existing: &UsageObservation, incoming: UsageObservation) -> UsageObservation {
    let mut merged = incoming;
    if merged.model().is_none() {
        if let Some(model) = existing.model() {
            merged = merged.with_model(model);
        }
    }
    if merged.recorded_at().is_none() {
        if let Some(at) = existing.recorded_at() {
            merged = merged.with_recorded_at(at);
        }
    }
    merged
}
```

Use at both same-identity merge sites (`*existing = merge_observation(existing, observation);`).

- [ ] **Step 4: Verify GREEN**

Run: `cargo test -p toktally-store`
Expected: PASS — totals-update test (`second_report_for_same_identity_updates_totals…`) unaffected (its payloads carry no metadata).

- [ ] **Step 5: Commit**

```bash
git add crates/store/src/lib.rs crates/store/tests/store.rs
git commit -m "fix(store): carry forward metadata omitted by same-identity re-ingest"
```

---

### Task 5: Full verification + docs

- [ ] `cargo test --workspace` → exit 0, all suites pass
- [ ] `cargo clippy --workspace --all-targets` → exit 0, zero warnings
- [ ] Smoke: `TOKEN_USAGE_HARNESS_HOME=<tmp-home> ./target/debug/toktally list` runs clean against a store built twice concurrently (two parallel invocations of `toktally ingest --adapter claude-code` fed the same fixture) — no lost rows vs serial baseline
- [ ] Update `crates/store/src/lib.rs` module doc if not already done in Task 1 (mention `.lock` sidecar + metadata preservation)
- [ ] Final commit if docs changed: `docs(store): document lock sidecar and merge policy`
