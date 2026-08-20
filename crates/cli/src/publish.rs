//! Publish local usage to a directory or GitHub gist. Ingest stays local.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use token_usage_store::FileStore;

use crate::pricing::load_price_table;
use crate::summary::{shields_badge, summarize_priced};
use crate::wire::WireObservation;

/// Files written for GitHub (gist or a repo directory).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishBundle {
    pub summary_json: String,
    pub shields_json: String,
    pub sessions_jsonl: String,
}

/// Remembers the last gist so later `publish`/`pull` need no id.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GithubConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gist_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gist_owner: Option<String>,
}

/// Gist identity parsed from `gh gist create` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GistRef {
    pub id: String,
    pub owner: Option<String>,
}

/// Build the files a gist or usage repo should contain.
pub fn bundle_from_store(
    store: &FileStore,
    store_path: &Path,
    generated_at: u64,
) -> Result<PublishBundle, String> {
    let listed = store.list().map_err(|e| e.to_string())?;
    let prices = load_price_table(store_path);
    let summary = summarize_priced(&listed, generated_at, prices.as_ref());
    let mut sessions_jsonl = String::new();
    for obs in &listed {
        sessions_jsonl.push_str(
            &serde_json::to_string(&WireObservation::from_observation(obs))
                .map_err(|e| e.to_string())?,
        );
        sessions_jsonl.push('\n');
    }
    Ok(PublishBundle {
        summary_json: format!(
            "{}\n",
            serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?
        ),
        shields_json: format!(
            "{}\n",
            serde_json::to_string_pretty(&shields_badge(&summary)).map_err(|e| e.to_string())?
        ),
        sessions_jsonl,
    })
}

/// Write a bundle into `dir`. Session JSONL is omitted for a public publish.
pub fn write_bundle(
    dir: &Path,
    bundle: &PublishBundle,
    include_sessions: bool,
) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    fs::write(dir.join("usage-summary.json"), &bundle.summary_json).map_err(|e| e.to_string())?;
    fs::write(dir.join("usage-badge.json"), &bundle.shields_json).map_err(|e| e.to_string())?;
    if include_sessions {
        fs::write(dir.join("usage.jsonl"), &bundle.sessions_jsonl).map_err(|e| e.to_string())?;
    } else {
        let _ = fs::remove_file(dir.join("usage.jsonl"));
    }
    Ok(())
}

/// Import `usage.jsonl` from `dir` into the local store (last-write-wins).
pub fn pull_dir(store: &FileStore, dir: &Path) -> Result<u64, String> {
    let path = dir.join("usage.jsonl");
    if !path.is_file() {
        return Err(
            "no usage.jsonl in that directory (summary-only cannot restore sessions)".into(),
        );
    }
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    import_jsonl(store, &raw)
}

fn import_jsonl(store: &FileStore, raw: &str) -> Result<u64, String> {
    let mut n = 0u64;
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let wire: WireObservation = serde_json::from_str(line).map_err(|e| e.to_string())?;
        store
            .ingest(wire.into_observation().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        n += 1;
    }
    Ok(n)
}

/// Push a bundle with `gh gist`. Returns id and owner when the create URL has them.
pub fn push_gist(
    bundle: &PublishBundle,
    id: Option<&str>,
    public: bool,
) -> Result<GistRef, String> {
    // Private staging: a unique mode-0700 temp dir so other local users cannot
    // read `usage.jsonl` (session ids) or swap files between write and `gh`.
    let work = tempfile::Builder::new()
        .prefix("token-usage-publish-")
        .tempdir()
        .map_err(|e| e.to_string())?;
    write_bundle(work.path(), bundle, !public)?;
    let gh = gh_bin();
    if let Some(id) = id {
        let mut cmd = Command::new(&gh);
        cmd.arg("gist").arg("edit").arg(id);
        cmd.arg(work.path().join("usage-summary.json"));
        cmd.arg(work.path().join("usage-badge.json"));
        if !public {
            cmd.arg(work.path().join("usage.jsonl"));
        }
        run_gh(&mut cmd)?;
        return Ok(GistRef {
            id: id.to_string(),
            owner: None,
        });
    }
    let mut cmd = Command::new(&gh);
    cmd.arg("gist").arg("create").arg("-d").arg("token-usage");
    if public {
        cmd.arg("--public");
    }
    cmd.arg(work.path().join("usage-summary.json"));
    cmd.arg(work.path().join("usage-badge.json"));
    if !public {
        cmd.arg(work.path().join("usage.jsonl"));
    }
    let out = run_gh(&mut cmd)?;
    parse_gist_ref(&out).ok_or_else(|| format!("could not parse gist id from: {out}"))
}

/// Fetch a gist via `gh api` and import `usage.jsonl` when present.
pub fn pull_gist(store: &FileStore, id: &str) -> Result<u64, String> {
    let mut cmd = Command::new(gh_bin());
    cmd.arg("api").arg(format!("gists/{id}"));
    let out = run_gh(&mut cmd)?;
    let value: serde_json::Value = serde_json::from_str(&out).map_err(|e| e.to_string())?;
    let files = value
        .get("files")
        .and_then(|f| f.as_object())
        .ok_or_else(|| "gist has no files object".to_string())?;
    let jsonl = files
        .get("usage.jsonl")
        .and_then(|f| f.get("content"))
        .and_then(|c| c.as_str())
        .ok_or_else(|| {
            "gist has no usage.jsonl (summary-only cannot restore sessions)".to_string()
        })?;
    import_jsonl(store, jsonl)
}

pub fn config_path_for_store(store_path: &Path) -> PathBuf {
    store_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("github.json")
}

pub fn load_github_config(store_path: &Path) -> GithubConfig {
    let path = config_path_for_store(store_path);
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn save_github_config(store_path: &Path, config: &GithubConfig) -> Result<(), String> {
    let path = config_path_for_store(store_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(
        path,
        serde_json::to_string_pretty(config).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

fn gh_bin() -> PathBuf {
    std::env::var_os("TOKEN_USAGE_GH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("gh"))
}

fn run_gh(cmd: &mut Command) -> Result<String, String> {
    let out = cmd.output().map_err(|e| format!("failed to run gh: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "gh failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn parse_gist_ref(output: &str) -> Option<GistRef> {
    let url = output.lines().rev().find(|l| !l.trim().is_empty())?.trim();
    if let Some(rest) = url.split("gist.github.com/").nth(1) {
        let mut parts = rest.trim_matches('/').split('/');
        let first = parts.next()?.trim();
        if first.is_empty() {
            return None;
        }
        if let Some(id) = parts.next() {
            let id = id.trim();
            if !id.is_empty() {
                return Some(GistRef {
                    id: id.to_string(),
                    owner: Some(first.to_string()),
                });
            }
        }
        return Some(GistRef {
            id: first.to_string(),
            owner: None,
        });
    }
    url.rsplit('/').next().map(|id| GistRef {
        id: id.trim().to_string(),
        owner: None,
    })
}

/// `gh api user` login, used when github.json has an id but no owner.
pub fn gh_login() -> Option<String> {
    let mut cmd = Command::new(gh_bin());
    cmd.arg("api").arg("user");
    let out = run_gh(&mut cmd).ok()?;
    serde_json::from_str::<serde_json::Value>(&out)
        .ok()?
        .get("login")
        .and_then(|v| v.as_str())
        .map(str::to_string)
}
