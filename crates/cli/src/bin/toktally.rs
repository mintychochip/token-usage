//! Plugin-facing reporter: adapt a harness payload and ingest it.

use std::io::{self, Read};
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use base64::Engine;
use toktally_adapters::adapt;
use toktally_cli::{
    bundle_from_summary, gh_login, gist_raw_base, github_pages,
    load_github_config, load_price_table, publish_config, publish_snippets, pull_dir, pull_gist,
    push_gist, save_github_config, shields_badge, summarize_priced,
    widgets_publish, write_bundle, WireHarnessSync, WireObservation, WireSyncStatus, USAGE_CARD_JS,
};
use toktally_domain::{Harness, ObservationIdentity, SessionId};
use toktally_store::FileStore;
use toktally_sync::{sync_all, sync_all_needed, sync_harness, SyncRoots};

#[derive(Parser)]
#[command(
    name = "toktally",
    about = "Track token usage across all your AI coding agents"
)]
struct Cli {
    /// Path to the durable store (JSON file).
    #[arg(long, env = "TOKTALLY_STORE", global = true)]
    store: Option<PathBuf>,
    /// Root that contains `.grok`, `.pi`, `.omp`, and other harness dirs.
    #[arg(long, env = "TOKTALLY_HARNESS_HOME", global = true)]
    home: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Read a harness payload from stdin or `--file` and ingest it.
    Ingest {
        /// Named harness adapter to apply.
        #[arg(long)]
        adapter: String,
        /// Optional payload file; stdin is used when omitted.
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// Read the stored total for a harness/session identity.
    Get {
        #[arg(long)]
        harness: String,
        #[arg(long)]
        session: String,
    },
    /// List every stored identity.
    List,
    /// Show the machine's widget identity (UUID and public key).
    Identity {
        #[arg(long)]
        show_secret: bool,
    },
    /// Write stored usage as a file you can gist, commit, or chart.
    Export {
        /// Destination file; stdout when omitted.
        #[arg(long)]
        file: Option<PathBuf>,
        /// `jsonl` (full sessions), `summary` (per-harness totals, no session ids), or `shields`.
        #[arg(long, default_value = "jsonl")]
        format: String,
    },
    /// Read JSONL observations into the local store.
    Import {
        #[arg(long)]
        file: PathBuf,
    },
    /// Scan existing harness session stores (all sessions, not just the active one).
    Sync {
        /// Limit the scan to one harness. Default: every named harness that needs first sync.
        #[arg(long)]
        harness: Option<String>,
        /// Scan even if this harness was synced before.
        #[arg(long)]
        force: bool,
        /// Re-run the scan every N seconds (includes `{harness}/usage.json` global snapshots).
        #[arg(long)]
        interval: Option<u64>,
    },
    /// Push the local store to a directory, a GitHub gist (`gh`), the widget service, or GitHub Pages.
    Publish {
        /// Directory to write.
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Gist id to update. Pass `--gist` alone to create or reuse `github.json`.
        #[arg(long, num_args = 0..=1, default_missing_value = "")]
        gist: Option<String>,
        /// Omit `usage.jsonl` (summary + badge only). Use for a public gist or repo.
        #[arg(long)]
        public: bool,
        /// Public base URL of the published files (GitHub Pages, raw gist, widget service, …).
        #[arg(long)]
        url: Option<String>,
        /// Publish the summary to the configured widget service.
        #[arg(long)]
        widgets: bool,
        /// Skip the widget service for this run.
        #[arg(long, conflicts_with = "widgets")]
        no_widgets: bool,
        /// Publish the summary to the configured GitHub Pages repo.
        #[arg(long)]
        github_pages: bool,
        /// Skip GitHub Pages for this run.
        #[arg(long, conflicts_with = "github_pages")]
        no_github_pages: bool,
    },
    /// Import sessions from a published directory or gist into the local store.
    Pull {
        #[arg(long, conflicts_with = "gist")]
        dir: Option<PathBuf>,
        /// Gist id to fetch. Pass `--gist` alone to reuse `github.json`.
        #[arg(long, num_args = 0..=1, default_missing_value = "")]
        gist: Option<String>,
    },
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    if let Command::Identity { show_secret } = cli.command {
            let id = toktally_cli::identity::load_or_generate()?;
            println!("uuid: {}", id.uuid);
            println!("public_key: {}", base64::engine::general_purpose::STANDARD.encode(&id.public_key));
            if show_secret {
                println!(
                    "secret_path: {}",
                    toktally_cli::identity::key_dir().join("identity.sec").display()
                );
            }
            return Ok(());
        }
        let store_path = cli
            .store
            .or_else(|| std::env::var_os("TOKEN_USAGE_STORE").map(PathBuf::from))
            .unwrap_or_else(default_store_path);
        let store = FileStore::open(&store_path)?;
        let roots = cli
            .home
            .or_else(|| std::env::var_os("TOKEN_USAGE_HARNESS_HOME").map(PathBuf::from))
            .map(|home| SyncRoots { home })
            .unwrap_or_else(SyncRoots::from_env);
        match cli.command {
            Command::Ingest { adapter, file } => {
                let harness = Harness::parse(&adapter)?;
                if store.needs_first_sync(harness)? {
                    sync_harness(&store, harness, &roots, unix_now())?;
                }
                let raw = match file {
                    Some(path) => std::fs::read_to_string(path)?,
                    None => {
                        let mut buf = String::new();
                        io::stdin().read_to_string(&mut buf)?;
                        buf
                    }
                };
                let payload: serde_json::Value = serde_json::from_str(&raw)?;
                let observation = adapt(harness, &payload)?;
                let stored = store.ingest(observation)?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&WireObservation::from_observation(&stored))?
                );
            }
            Command::Get { harness, session } => {
                let identity =
                    ObservationIdentity::new(Harness::parse(&harness)?, SessionId::parse(session)?);
                match store.get(&identity)? {
                    Some(obs) => {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&WireObservation::from_observation(&obs))?
                        );
                    }
                    None => {
                        return Err("not found".into());
                    }
                }
            }
            Command::Export { file, format } => {
                let listed = store.list()?;
                let out = match format.as_str() {
                    "jsonl" => {
                        let mut buf = String::new();
                        for obs in &listed {
                            buf.push_str(&serde_json::to_string(&WireObservation::from_observation(
                                obs,
                            ))?);
                            buf.push('\n');
                        }
                        buf
                    }
                    "summary" => {
                        let prices = toktally_cli::load_price_table(&store_path);
                        let summary = summarize_priced(&listed, unix_now(), prices.as_ref());
                        format!("{}\n", serde_json::to_string_pretty(&summary)?)
                    }
                    "shields" => {
                        let prices = toktally_cli::load_price_table(&store_path);
                        let badge =
                            shields_badge(&summarize_priced(&listed, unix_now(), prices.as_ref()));
                        format!("{}\n", serde_json::to_string_pretty(&badge)?)
                    }
                    other => return Err(format!("unknown export format: {other}").into()),
                };
                match file {
                    Some(path) => std::fs::write(path, out)?,
                    None => print!("{out}"),
                }
            }
            Command::Import { file } => {
                let raw = std::fs::read_to_string(file)?;
                for line in raw.lines() {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    let wire: WireObservation = serde_json::from_str(line)?;
                    store.ingest(wire.into_observation()?)?;
                }
                let sessions: Vec<_> = store
                    .list()?
                    .iter()
                    .map(WireObservation::from_observation)
                    .collect();
                println!("{}", serde_json::to_string_pretty(&sessions)?);
            }
            Command::List => {
                if store.list_harness_syncs()?.is_empty() {
                    sync_all_needed(&store, &roots, unix_now())?;
                }
                let sessions: Vec<_> = store
                    .list()?
                    .iter()
                    .map(WireObservation::from_observation)
                    .collect();
                println!("{}", serde_json::to_string_pretty(&sessions)?);
            }
            Command::Sync {
                harness,
                force,
                interval,
            } => {
                if matches!(interval, Some(0)) {
                    return Err("interval must be greater than 0".into());
                }
                loop {
                    let now = unix_now();
                    // A timer is a repeating scan; --interval implies --force after the first tick.
                    let rescan = force || interval.is_some();
                    if let Some(name) = harness.as_deref() {
                        let harness = Harness::parse(name)?;
                        if rescan || store.needs_first_sync(harness)? {
                            sync_harness(&store, harness, &roots, now)?;
                        }
                    } else if rescan {
                        sync_all(&store, &roots, now)?;
                    } else {
                        sync_all_needed(&store, &roots, now)?;
                    }
                    let status = WireSyncStatus {
                        harnesses: store
                            .list_harness_syncs()?
                            .into_iter()
                            .map(|row| WireHarnessSync {
                                harness: row.harness,
                                last_synced_at: row.last_synced_at,
                            })
                            .collect(),
                    };
                    println!("{}", serde_json::to_string_pretty(&status)?);
                    match interval {
                        Some(secs) => std::thread::sleep(std::time::Duration::from_secs(secs)),
                        None => break,
                    }
                }
            }
            Command::Publish {
                dir,
                gist,
                public,
                url,
                widgets,
                no_widgets,
                github_pages,
                no_github_pages,
            } => {
                let cfg =
                    publish_config::load_or_create(&publish_config::default_config_path())?;

                let explicit_target = dir.is_some() || gist.is_some();
                let do_widgets =
                    (widgets || (!explicit_target && cfg.widgets.enabled)) && !no_widgets;
                let do_gh_pages = (github_pages
                    || (!explicit_target && cfg.github_pages.enabled))
                    && !no_github_pages;
                let do_dir = dir.is_some();
                let do_gist =
                    gist.is_some() || (!do_dir && !do_gh_pages && !do_widgets && !explicit_target);

                if !do_widgets && !do_gh_pages && !do_dir && !do_gist {
                    return Err("no publish target selected or enabled".into());
                }

                let generated_at = unix_now();
                let prices = load_price_table(&store_path);
                let listed = store.list().map_err(|e| e.to_string())?;
                let summary = summarize_priced(&listed, generated_at, prices.as_ref());
                let bundle = bundle_from_summary(&summary, &listed)?;

                let mut errors: Vec<String> = Vec::new();

                if do_widgets {
                    let service_url = url
                        .as_deref()
                        .unwrap_or(&cfg.widgets.url);
                    match widgets_publish::publish_summary(&summary, service_url) {
                        Ok(widget_url) => println!("{widget_url}"),
                        Err(e) => errors.push(format!("widgets: {e}")),
                    }
                }

                if do_gh_pages {
                    if cfg.github_pages.repo.is_empty() {
                        errors.push(
                            "github-pages: --github-pages requested but publish-config.github_pages.repo is empty".into(),
                        );
                    } else {
                        match github_pages::publish(
                            &cfg.github_pages.repo,
                            &bundle,
                            USAGE_CARD_JS,
                            generated_at,
                        ) {
                            Ok(page_url) => println!("{page_url}"),
                            Err(e) => errors.push(format!("github-pages: {e}")),
                        }
                    }
                }

                if let Some(dir) = dir {
                    if let Err(e) = write_bundle(&dir, &bundle, !public) {
                        errors.push(format!("dir: {e}"));
                    } else if let Err(e) = std::fs::write(dir.join("usage-card.js"), USAGE_CARD_JS) {
                        errors.push(format!("dir: {e}"));
                    } else {
                        let base = url.as_deref().unwrap_or(".");
                        let snippets = publish_snippets(base);
                        if std::fs::write(dir.join("snippets.md"), &snippets).is_ok() {
                            println!("{snippets}");
                        } else {
                            errors.push("dir: failed to write snippets.md".into());
                        }
                    }
                }

                if do_gist {
                    let mut gh_cfg = load_github_config(&store_path);
                    let remembered = match gist.as_deref() {
                        None | Some("") => gh_cfg.gist_id.clone(),
                        Some(id) => Some(id.to_string()),
                    };
                    let work = std::env::temp_dir().join(format!(
                        "token-usage-publish-{}-{}",
                        std::process::id(),
                        generated_at
                    ));
                    match push_gist(&bundle, remembered.as_deref(), public, &work) {
                        Ok(gist_ref) => {
                            let _ = std::fs::remove_dir_all(&work);
                            let owner = gist_ref
                                .owner
                                .or(gh_cfg.gist_owner.clone())
                                .or_else(gh_login);
                            gh_cfg.gist_id = Some(gist_ref.id.clone());
                            gh_cfg.gist_owner = owner.clone();
                            let _ = save_github_config(&store_path, &gh_cfg);
                            let base = match (url.as_deref(), owner.as_deref()) {
                                (Some(base), _) => base.to_string(),
                                (None, Some(owner)) => gist_raw_base(owner, &gist_ref.id),
                                (None, None) => {
                                    errors.push(
                                        "gist: could not determine gist owner for raw URLs".into(),
                                    );
                                    String::new()
                                }
                            };
                            if !base.is_empty() {
                                println!("{}", gist_ref.id);
                                println!("{}", publish_snippets(&base));
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
            }
            Command::Pull { dir, gist } => {
                if let Some(dir) = dir {
                    pull_dir(&store, &dir)?;
                    return Ok(());
                }
                let cfg = load_github_config(&store_path);
                let id = match gist.as_deref() {
                    None | Some("") => cfg.gist_id,
                    Some(id) => Some(id.to_string()),
                };
                let id = id.ok_or("no gist id; pass --gist ID or publish first")?;
                pull_gist(&store, &id)?;
            }
            Command::Identity { .. } => {
                unreachable!("identity handled before store is opened")
            }
        }
        Ok(())
    }

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn default_store_path() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".toktally")
        .join("store.json")
}
