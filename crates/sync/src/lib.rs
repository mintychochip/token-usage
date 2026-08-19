//! Scan on-disk harness session stores and ingest every session we can map.
//!
//! This is not limited to the active session. First-use sync walks known
//! host directories under a provided home root (never by mutating `$HOME`).

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use token_usage_adapters::{adapt, AdaptError};
use token_usage_domain::Harness;
use token_usage_store::{FileStore, StoreError};

/// Failures while scanning or ingesting discovered sessions.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("store error: {0}")]
    Store(#[from] StoreError),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("adapt error: {0}")]
    Adapt(#[from] AdaptError),
}

/// Where to look for host session files.
#[derive(Debug, Clone)]
pub struct SyncRoots {
    pub home: PathBuf,
}

impl SyncRoots {
    /// `TOKEN_USAGE_HARNESS_HOME`, else the process `HOME`, else `.`.
    pub fn from_env() -> Self {
        let home = std::env::var_os("TOKEN_USAGE_HARNESS_HOME")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        Self { home }
    }
}

/// Result of scanning one harness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncReport {
    pub harness: Harness,
    pub ingested: u64,
    pub skipped: u64,
    pub last_synced_at: u64,
}

/// Discover payloads for `harness` under `roots` and ingest each one.
pub fn sync_harness(
    store: &FileStore,
    harness: Harness,
    roots: &SyncRoots,
    last_synced_at: u64,
) -> Result<SyncReport, SyncError> {
    let payloads = discover(harness, roots)?;
    let mut ingested = 0;
    let mut skipped = 0;
    for payload in payloads {
        match adapt(harness, &payload) {
            Ok(observation) => {
                store.ingest_at(observation, last_synced_at)?;
                ingested += 1;
            }
            Err(_) => skipped += 1,
        }
    }
    store.record_harness_sync(harness, last_synced_at)?;
    Ok(SyncReport {
        harness,
        ingested,
        skipped,
        last_synced_at,
    })
}

/// Scan every named harness that has never been synced.
pub fn sync_all_needed(
    store: &FileStore,
    roots: &SyncRoots,
    last_synced_at: u64,
) -> Result<Vec<SyncReport>, SyncError> {
    let mut reports = Vec::new();
    for harness in Harness::all() {
        if store.needs_first_sync(harness)? {
            reports.push(sync_harness(store, harness, roots, last_synced_at)?);
        }
    }
    Ok(reports)
}

/// Force-scan one or all harnesses.
pub fn sync_all(
    store: &FileStore,
    roots: &SyncRoots,
    last_synced_at: u64,
) -> Result<Vec<SyncReport>, SyncError> {
    Harness::all()
        .into_iter()
        .map(|harness| sync_harness(store, harness, roots, last_synced_at))
        .collect()
}

/// JSON payloads found on disk for `harness`. One payload per session file.
pub fn discover(harness: Harness, roots: &SyncRoots) -> Result<Vec<Value>, SyncError> {
    let home = &roots.home;
    let mut payloads = match harness {
        Harness::Grok => discover_grok(home)?,
        Harness::Pi => discover_jsonl(home.join(".pi/agent/sessions"))?,
        Harness::OhMyPi => discover_jsonl(home.join(".omp/agent/sessions"))?,
        Harness::ClaudeCode => discover_jsonl(home.join(".claude/projects"))?,
        Harness::Codex => discover_jsonl(home.join(".codex/sessions"))?,
        Harness::Hermes => discover_json_tree(home.join(".hermes"))?,
        Harness::OpenCode => {
            let mut out = discover_json_tree(home.join(".opencode"))?;
            out.extend(discover_json_tree(home.join(".local/share/opencode"))?);
            out
        }
        Harness::GeminiCli => discover_json_tree(home.join(".gemini"))?,
        Harness::Goose => {
            let mut out = discover_json_tree(home.join(".config/goose"))?;
            out.extend(discover_json_tree(home.join(".local/share/goose"))?);
            out
        }
        Harness::Amp => discover_json_tree(home.join(".amp"))?,
        Harness::Droid => discover_json_tree(home.join(".factory"))?,
        Harness::Cline => discover_json_tree(home.join(".cline"))?,
        Harness::Aider => discover_json_tree(home.join(".aider"))?,
        Harness::Jcode => discover_json_tree(home.join(".jcode"))?,
    };
    payloads.extend(discover_global_usage(harness, home)?);
    Ok(payloads)
}

/// Reserved `{harness-dir}/usage.json` — a host-wide `/usage` snapshot, not a session.
fn discover_global_usage(harness: Harness, home: &Path) -> Result<Vec<Value>, SyncError> {
    let path = harness_home(harness, home).join("usage.json");
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let mut value: Value = serde_json::from_str(&fs::read_to_string(&path)?)?;
    if !looks_like_usage(&value) {
        return Ok(Vec::new());
    }
    if let Some(obj) = value.as_object_mut() {
        obj.entry("kind".to_string())
            .or_insert_with(|| json!("global_usage"));
    }
    Ok(vec![value])
}

fn harness_home(harness: Harness, home: &Path) -> PathBuf {
    match harness {
        Harness::ClaudeCode => home.join(".claude"),
        Harness::Codex => home.join(".codex"),
        Harness::Grok => home.join(".grok"),
        Harness::OhMyPi => home.join(".omp"),
        Harness::Jcode => home.join(".jcode"),
        Harness::Hermes => home.join(".hermes"),
        Harness::OpenCode => home.join(".opencode"),
        Harness::GeminiCli => home.join(".gemini"),
        Harness::Aider => home.join(".aider"),
        Harness::Goose => home.join(".config/goose"),
        Harness::Amp => home.join(".amp"),
        Harness::Droid => home.join(".factory"),
        Harness::Cline => home.join(".cline"),
        Harness::Pi => home.join(".pi"),
    }
}

fn discover_grok(home: &Path) -> Result<Vec<Value>, SyncError> {
    let mut payloads = Vec::new();
    for path in walk_files(&home.join(".grok/sessions"), "signals.json")? {
        let mut value: Value = serde_json::from_str(&fs::read_to_string(&path)?)?;
        let session_id = grok_session_id(&path);
        if let Some(obj) = value.as_object_mut() {
            obj.insert("sessionId".to_string(), json!(session_id));
        }
        payloads.push(value);
    }
    Ok(payloads)
}

fn grok_session_id(signals: &Path) -> String {
    let dir = signals.parent();
    if let Some(dir) = dir {
        let summary = dir.join("summary.json");
        if let Ok(raw) = fs::read_to_string(&summary) {
            if let Ok(value) = serde_json::from_str::<Value>(&raw) {
                if let Some(id) = value
                    .pointer("/info/id")
                    .and_then(Value::as_str)
                    .or_else(|| value.get("sessionId").and_then(Value::as_str))
                {
                    return id.to_string();
                }
            }
        }
        if let Some(name) = dir.file_name().and_then(|n| n.to_str()) {
            return name.to_string();
        }
    }
    "unknown".to_string()
}

fn discover_jsonl(root: PathBuf) -> Result<Vec<Value>, SyncError> {
    let mut payloads = Vec::new();
    for path in walk_files(&root, "")? {
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        if let Some(payload) = aggregate_jsonl(&path)? {
            payloads.push(payload);
        }
    }
    Ok(payloads)
}

fn discover_json_tree(root: PathBuf) -> Result<Vec<Value>, SyncError> {
    let mut payloads = Vec::new();
    for path in walk_files(&root, "")? {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext == "jsonl" {
            if let Some(payload) = aggregate_jsonl(&path)? {
                payloads.push(payload);
            }
        } else if ext == "json" {
            if let Ok(raw) = fs::read_to_string(&path) {
                if let Ok(value) = serde_json::from_str::<Value>(&raw) {
                    if looks_like_usage(&value) {
                        payloads.push(value);
                    }
                }
            }
        }
    }
    Ok(payloads)
}

fn aggregate_jsonl(path: &Path) -> Result<Option<Value>, SyncError> {
    let raw = fs::read_to_string(path)?;
    let mut session_id: Option<String> = None;
    let mut input = 0u64;
    let mut output = 0u64;
    let mut cache_read = 0u64;
    let mut saw_usage = false;
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if session_id.is_none() {
            if value.get("type").and_then(Value::as_str) == Some("session") {
                if let Some(id) = value.get("id").and_then(Value::as_str) {
                    session_id = Some(id.to_string());
                }
            }
            if let Some(id) = value
                .get("session_id")
                .or_else(|| value.get("sessionId"))
                .and_then(Value::as_str)
            {
                session_id = Some(id.to_string());
            }
        }
        if let Some(usage) = find_usage(&value) {
            saw_usage = true;
            input += usage_u64(
                usage,
                &["input", "input_tokens", "inputTokens", "prompt_tokens"],
            );
            output += usage_u64(
                usage,
                &[
                    "output",
                    "output_tokens",
                    "outputTokens",
                    "completion_tokens",
                ],
            );
            cache_read += usage_u64(
                usage,
                &[
                    "cacheRead",
                    "cache_read",
                    "cache_read_tokens",
                    "cached_input_tokens",
                ],
            );
        }
    }
    if !saw_usage {
        return Ok(None);
    }
    let session_id = session_id.unwrap_or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string()
    });
    Ok(Some(json!({
        "sessionId": session_id,
        "stats": {
            "inputTokens": input,
            "outputTokens": output,
            "cacheReadTokens": cache_read
        }
    })))
}

fn find_usage(value: &Value) -> Option<&Value> {
    value
        .pointer("/message/usage")
        .or_else(|| value.pointer("/usage"))
        .or_else(|| value.pointer("/token_usage"))
        .or_else(|| value.pointer("/payload/usage"))
        .filter(|v| v.is_object())
}

fn looks_like_usage(value: &Value) -> bool {
    find_usage(value).is_some()
        || value.get("input_tokens").is_some()
        || value.get("inputTokens").is_some()
        || value.get("contextTokensUsed").is_some()
}

fn usage_u64(usage: &Value, keys: &[&str]) -> u64 {
    keys.iter()
        .find_map(|key| usage.get(*key).and_then(Value::as_u64))
        .unwrap_or(0)
}

fn walk_files(root: &Path, filename: &str) -> Result<Vec<PathBuf>, SyncError> {
    let mut out = Vec::new();
    if !root.exists() {
        return Ok(out);
    }
    walk_files_inner(root, filename, &mut out, 0)?;
    Ok(out)
}

fn walk_files_inner(
    dir: &Path,
    filename: &str,
    out: &mut Vec<PathBuf>,
    depth: usize,
) -> Result<(), SyncError> {
    if depth > 12 {
        return Ok(());
    }
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if ft.is_dir() {
            walk_files_inner(&path, filename, out, depth + 1)?;
        } else if ft.is_file()
            && (filename.is_empty() || path.file_name().and_then(|n| n.to_str()) == Some(filename))
        {
            out.push(path);
        }
    }
    Ok(())
}
