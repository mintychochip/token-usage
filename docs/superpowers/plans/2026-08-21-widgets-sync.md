# widgets-sync

- created: 2026-08-21
- branch: `feature/widgets`
- worktree: `.worktrees/feature-widgets`
- depends on: identity, widgets service, `gh` CLI

## Goal

`token-usage-reporter publish` should compute the summary once and publish it to every configured backend in one run. Central widgets service remains the default; GitHub Pages is the opt-out path. Existing `--dir` and `--gist` keep working as explicit single-target overrides.

## Configuration source of truth

Use a new user-level file so `publish` with no flags does the right thing:

`~/.toktally/publish-config.json`:

```json
{
  "widgets": {
    "enabled": true,
    "url": "https://widgets.mintychochip.dev"
  },
  "github_pages": {
    "enabled": false,
    "repo": "mintychochip/token-usage-pages"
  }
}
```

- Default file is written on first publish.
- `token-usage-reporter publish` with no flags publishes every `enabled: true` target.
- `token-usage-reporter publish --widgets` and `--github-pages` force that target (may be combined).
- `--no-widgets` / `--no-github-pages` should also exist to skip a target for one run.

## Phase 1: Persisted publish config

### Files
- `crates/cli/src/publish_config.rs` (new)
- `crates/cli/src/lib.rs` (add `pub mod publish_config`)
- `crates/cli/src/bin/token-usage-reporter.rs` (load config and dispatch)

### Implementation
`crates/cli/src/publish_config.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetsTarget {
    pub enabled: bool,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubPagesTarget {
    pub enabled: bool,
    pub repo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishConfig {
    pub widgets: WidgetsTarget,
    pub github_pages: GithubPagesTarget,
}

impl Default for PublishConfig {
    fn default() -> Self {
        Self {
            widgets: WidgetsTarget {
                enabled: true,
                url: "https://widgets.mintychochip.dev".into(),
            },
            github_pages: GithubPagesTarget {
                enabled: false,
                repo: String::new(),
            },
        }
    }
}

pub fn default_config_path() -> PathBuf {
    token_usage_cli::identity::key_dir() // ~/.toktally
        .parent()
        .unwrap()
        .join("publish-config.json")
}

pub fn load_or_create(path: &Path) -> Result<PublishConfig, String> {
    if !path.exists() {
        let cfg = PublishConfig::default();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(path, serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        return Ok(cfg);
    }
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

pub fn save(path: &Path, cfg: &PublishConfig) -> Result<(), String> {
    std::fs::write(path, serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
```

### Acceptance
- `load_or_create` returns the default and writes `~/.toktally/publish-config.json` when it does not exist.
- `save` round-trips through serde.

### Tests
- `crates/cli/tests/publish_config.rs` (new):
  - `publish_config_defaults_and_round_trip`

---

## Phase 2: Multi-target `publish` dispatch

### Files
- `crates/cli/src/bin/token-usage-reporter.rs` (rewrite `Command::Publish` arm)
- `crates/cli/src/widgets_publish.rs` (return summary URL and raw JSON instead of just printing)

### New `Command::Publish` shape in `crates/cli/src/bin/token-usage-reporter.rs`

```rust
#[derive(Subcommand)]
enum Command {
    // ...
    Publish {
        #[arg(long, action = clap::ArgAction::SetTrue)]
        widgets: bool,
        #[arg(long, action = clap::ArgAction::SetTrue)]
        no_widgets: bool,
        #[arg(long, action = clap::ArgAction::SetTrue)]
        github_pages: bool,
        #[arg(long, action = clap::ArgAction::SetTrue)]
        no_github_pages: bool,
        #[arg(long, group = "explicit_target")]
        dir: Option<PathBuf>,
        #[arg(long, group = "explicit_target")]
        gist: Option<String>,
        #[arg(long)]
        public: bool,
        #[arg(long)]
        url: Option<String>,
    },
}
```

`dir` and `gist` remain mutually exclusive and also conflict with `widgets`/`github_pages` so they keep their old single-target behavior.

### Dispatch logic

Compute one summary and one bundle first, then attempt each target and collect errors so a failure on one target does not stop the others:

```rust
use token_usage_cli::publish_config::{PublishConfig, default_config_path, load_or_create};

let cfg = load_or_create(&default_config_path())?;

let explicit_target = dir.is_some() || gist.is_some();
let do_widgets = (widgets || (!explicit_target && cfg.widgets.enabled)) && !no_widgets;
let do_gh_pages = (github_pages || (!explicit_target && cfg.github_pages.enabled)) && !no_github_pages;
let do_dir = dir.is_some();
let do_gist = !do_dir && !do_gh_pages && !do_widgets && !explicit_target;
// ^ if no flags and no config, preserve old gist behavior so nothing breaks.

if !do_widgets && !do_gh_pages && !do_dir && !do_gist {
    return Err("no publish target selected or enabled".into());
}

let generated_at = unix_now();
let prices = token_usage_cli::load_price_table(&store_path);
let listed = store.list().map_err(|e| e.to_string())?;
let summary = token_usage_cli::summarize_priced(&listed, generated_at, prices.as_ref());
let bundle = token_usage_cli::bundle_from_summary(&summary, &listed)?;
```

let mut errors: Vec<String> = Vec::new();

if do_widgets {
    match token_usage_cli::widgets_publish::publish_summary(&summary, url.as_deref().unwrap_or(&cfg.widgets.url)) {
        Ok(widget_url) => println!("{widget_url}"),
        Err(e) => errors.push(format!("widgets: {e}")),
    }
}

if do_gh_pages {
    if cfg.github_pages.repo.is_empty() {
        return Err("--github-pages requested but publish-config.github_pages.repo is empty".into());
    }
    match token_usage_cli::github_pages::publish(&cfg.github_pages.repo, &bundle, token_usage_cli::USAGE_CARD_JS) {
        Ok(page_url) => println!("{page_url}"),
        Err(e) => errors.push(format!("github-pages: {e}")),
    }
}
if let Some(dir) = dir {
    if let Err(e) = token_usage_cli::write_bundle(&dir, &bundle, !public) {
        errors.push(format!("dir: {e}"));
    } else if let Err(e) = std::fs::write(dir.join("usage-card.js"), token_usage_cli::USAGE_CARD_JS) {
        errors.push(format!("dir: {e}"));
    } else {
        let base = url.as_deref().unwrap_or(".");
        let snippets = token_usage_cli::publish_snippets(base);
        if std::fs::write(dir.join("snippets.md"), &snippets).is_ok() {
            println!("{snippets}");
        } else {
            errors.push("dir: failed to write snippets.md".into());
        }
    }
}

if let Some(gist) = gist {
    let mut gh_cfg = token_usage_cli::load_github_config(&store_path);
    let remembered = match gist.as_deref() {
        None | Some("") => gh_cfg.gist_id.clone(),
        Some(id) => Some(id.to_string()),
    };
    let work = std::env::temp_dir().join(format!("token-usage-publish-{}-{}", std::process::id(), generated_at));
    match token_usage_cli::push_gist(&bundle, remembered.as_deref(), public, &work) {
        Ok(gist_ref) => {
            let _ = std::fs::remove_dir_all(&work);
            let owner = gist_ref.owner.or(gh_cfg.gist_owner.clone()).or_else(token_usage_cli::gh_login);
            gh_cfg.gist_id = Some(gist_ref.id.clone());
            gh_cfg.gist_owner = owner.clone();
            let _ = token_usage_cli::save_github_config(&store_path, &gh_cfg);
            let base = match (url.as_deref(), owner.as_deref()) {
                (Some(base), _) => base.to_string(),
                (None, Some(owner)) => token_usage_cli::gist_raw_base(owner, &gist_ref.id),
                (None, None) => {
                    errors.push("gist: could not determine gist owner for raw URLs".into());
                    String::new()
                }
            };
            if !base.is_empty() {
                println!("{}", gist_ref.id);
                println!("{}", token_usage_cli::publish_snippets(&base));
            }
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&work);
            errors.push(format!("gist: {e}"));
        }
    }
}

if !errors.is_empty() {
    return Err(errors.join("; ").into());
}

`bundle_from_summary` is a new helper in `crates/cli/src/publish.rs` and is re-exported at `token_usage_cli::bundle_from_summary`. It builds a `PublishBundle` from an already-computed `UsageSummary` and observation list, so `bundle_from_store` can use it too.

`widgets_publish::publish_summary(summary, service_url)` is a new public helper (re-exported at `token_usage_cli::widgets_publish::publish_summary`) that loads the local identity, signs the summary, POSTs it, and returns the widget URL.

`publish_to_widgets(store, ...)` should be kept as a convenience wrapper that computes the summary and calls `publish_summary`.
### Acceptance
- `publish` with no flags and a default config publishes to widgets.
- `publish --widgets --github-pages` publishes to both if the repo is configured.
- `publish --dir <dir>` still writes files and ignores config.
- `publish --gist` still pushes to gist and ignores config.
- Multiple targets print one URL per target and do not stop on first success.

### Tests
- `crates/cli/tests/publish.rs`:
  - `publish_widgets_and_dir_together` (mock widgets server with a fake HTTP endpoint or use local service)
  - `publish_config_controls_default_targets`

---

## Phase 3: GitHub Pages publisher

### Files
- `crates/cli/src/github_pages.rs` (new)
- `crates/cli/src/lib.rs` (add `pub mod github_pages`)

### Implementation
The publisher needs to create or update a repo. Use the same `gh` binary that the gist flow already uses (`TOKEN_USAGE_GH` override supported).

`crates/cli/src/github_pages.rs`:

```rust
use std::path::{Path, PathBuf};
use std::process::Command;
use token_usage_cli::publish::PublishBundle;

fn gh_bin() -> PathBuf {
    std::env::var_os("TOKEN_USAGE_GH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("gh"))
}

fn run_gh(cmd: &mut Command) -> Result<String, String> {
    let out = cmd.output().map_err(|e| format!("failed to run gh: {e}"))?;
    if !out.status.success() {
        return Err(format!("gh failed: {}", String::from_utf8_lossy(&out.stderr)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn repo_exists(repo: &str) -> Result<bool, String> {
    let mut cmd = Command::new(gh_bin());
    cmd.args(["repo", "view", repo]);
    match run_gh(&mut cmd) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

pub fn publish(
    repo: &str,
    bundle: &PublishBundle,
    card_js: &str,
) -> Result<String, String> {
    let work = std::env::temp_dir().join(format!(
        "token-usage-ghpages-{}-{}",
        std::process::id(),
        token_usage_cli::unix_now()
    ));
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;

    std::fs::write(work.join("usage-summary.json"), &bundle.summary_json)
        .map_err(|e| e.to_string())?;
    std::fs::write(work.join("usage-badge.json"), &bundle.shields_json)
        .map_err(|e| e.to_string())?;
    std::fs::write(work.join("token-usage-card.js"), card_js)
        .map_err(|e| e.to_string())?;

    let summary_url = format!("https://{owner}.github.io/{name}/usage-summary.json", owner = repo.split('/').next().unwrap_or(""), name = repo.rsplit('/').next().unwrap_or(""));
    let html = token_usage_cli::website_embed_html(&summary_url);
    std::fs::write(
        work.join("index.html"),
        format!("<!doctype html><html><head><meta charset=\"utf-8\"/><title>token usage</title></head><body>{html}</body></html>"),
    )
    .map_err(|e| e.to_string())?;

    if !repo_exists(repo)? {
        let mut cmd = Command::new(gh_bin());
        cmd.args(["repo", "create", repo, "--public", "--source=.", "--push"])
            .current_dir(&work);
        run_gh(&mut cmd)?;
    } else {
        let clone = work.join(".clone");
        let mut cmd = Command::new(gh_bin());
        cmd.args(["repo", "clone", repo, clone.to_str().unwrap()]);
        run_gh(&mut cmd)?;

        for name in ["usage-summary.json", "usage-badge.json", "token-usage-card.js", "index.html"] {
            std::fs::copy(work.join(name), clone.join(name)).map_err(|e| e.to_string())?;
        }

        let mut commit = Command::new("git");
        commit.args(["-C", clone.to_str().unwrap(), "add", "."]);
        let _ = commit.output();

        let mut commit = Command::new("git");
        commit.args([
            "-C",
            clone.to_str().unwrap(),
            "-c",
            "user.name=token-usage",
            "-c",
            "user.email=noreply@localhost",
            "commit",
            "-m",
            "update token usage",
        ]);
        let out = commit.output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!("git commit failed: {}", String::from_utf8_lossy(&out.stderr)));
        }

        let mut push = Command::new("git");
        push.args(["-C", clone.to_str().unwrap(), "push"]);
        let out = push.output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!("git push failed: {}", String::from_utf8_lossy(&out.stderr)));
        }
    }

    Ok(format!("https://{owner}.github.io/{name}", owner = repo.split('/').next().unwrap_or(""), name = repo.rsplit('/').next().unwrap_or("")))
}
```

### Notes / risks
- `gh repo create --source=. --push` needs the directory to be a git repo; the flag makes it one and pushes. Verify with real `gh` once.
- `gh repo clone` followed by `git` commands requires `gh` auth and git config. Use `git -c` for user.name/user.email to avoid global git config.
- Pages can take a minute to deploy after the push; the tool just returns the expected URL.

### Acceptance
- `publish --github-pages` creates the repo if missing, or updates it if it exists.
- Pages branch contains `usage-summary.json`, `usage-badge.json`, `token-usage-card.js`, `index.html`.
- Returned URL is `https://<owner>.github.io/<name>`.

### Tests
- `crates/cli/tests/github_pages.rs` (new):
  - Fake `gh` and `git` scripts like `tests/publish.rs`.
  - `github_pages_creates_and_pushes_files`.
  - `github_pages_updates_existing_repo`.

---

## Phase 4: Update CLI help and docs

### Files
- `README.md` (add `publish` flags and config examples)
- `docs/living-specs/token-usage.md` (update scope to include widget service and GitHub Pages)

### Acceptance
- `token-usage-reporter publish --help` lists `--widgets`, `--no-widgets`, `--github-pages`, `--no-github-pages`, `--url`, `--dir`, `--gist`, `--public`.
- Living spec describes central service as default and GitHub Pages as opt-out.

---

## Phase 5: Verification

### Run
- `cargo test --workspace` in `.worktrees/feature-widgets`.
- Manual smoke test:
  1. Start local `token-usage-widgets-api`.
  2. Configure `~/.toktally/publish-config.json` with `github_pages.repo` set to a real or test repo.
  3. `token-usage-reporter publish --widgets --github-pages`.
  4. Curl both widget summary URL and GitHub Pages URL.

### Acceptance
- All existing tests still pass.
- New tests for publish config and GitHub Pages pass.
- Manual multi-target publish works end-to-end.

## Out of scope
- Deploying `widgets.mintychochip.dev` itself (server binary is ready, deploy is separate).
- Persisting widget service state to disk (separate future item).
