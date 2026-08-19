//! Plugin-facing reporter: adapt a harness payload and ingest it.

use std::io::{self, Read};
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use token_usage_adapters::adapt;
use token_usage_cli::{
    bundle_from_store, load_github_config, publish_snippets, pull_dir, pull_gist, push_gist,
    save_github_config, shields_badge, summarize_priced, write_bundle, WireHarnessSync,
    WireObservation, WireSyncStatus, USAGE_CARD_JS,
};
use token_usage_domain::{Harness, ObservationIdentity, SessionId};
use token_usage_store::FileStore;
use token_usage_sync::{sync_all, sync_all_needed, sync_harness, SyncRoots};

#[derive(Parser)]
#[command(
    name = "token-usage-reporter",
    about = "Report harness token usage into the shared store"
)]
struct Cli {
    /// Path to the durable store (JSON file).
    #[arg(long, env = "TOKEN_USAGE_STORE", global = true)]
    store: Option<PathBuf>,
    /// Root that contains `.grok`, `.pi`, `.omp`, and other harness dirs.
    #[arg(long, env = "TOKEN_USAGE_HARNESS_HOME", global = true)]
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
    /// Push the local store to a directory or a GitHub gist (`gh`).
    Publish {
        /// Directory to write (commit this, or host it on GitHub Pages).
        #[arg(long, conflicts_with = "gist")]
        dir: Option<PathBuf>,
        /// Gist id to update. Pass `--gist` alone to create or reuse `github.json`.
        #[arg(long, num_args = 0..=1, default_missing_value = "")]
        gist: Option<String>,
        /// Omit `usage.jsonl` (summary + badge only). Use for a public gist or repo.
        #[arg(long)]
        public: bool,
        /// Public base URL of the published files (GitHub Pages, raw gist, …).
        #[arg(long)]
        url: Option<String>,
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
    let store_path = cli.store.unwrap_or_else(default_store_path);
    let store = FileStore::open(&store_path)?;
    let roots = cli
        .home
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
                    let prices = token_usage_cli::load_price_table(&store_path);
                    let summary = summarize_priced(&listed, unix_now(), prices.as_ref());
                    format!("{}\n", serde_json::to_string_pretty(&summary)?)
                }
                "shields" => {
                    let prices = token_usage_cli::load_price_table(&store_path);
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
        } => {
            let bundle = bundle_from_store(&store, unix_now())?;
            if let Some(dir) = dir {
                write_bundle(&dir, &bundle, !public)?;
                std::fs::write(dir.join("usage-card.js"), USAGE_CARD_JS)?;
                let base = url.as_deref().unwrap_or(".");
                let snippets = publish_snippets(base);
                std::fs::write(dir.join("snippets.md"), &snippets)?;
                println!("{snippets}");
                return Ok(());
            }
            let mut cfg = load_github_config(&store_path);
            let remembered = match gist.as_deref() {
                None | Some("") => cfg.gist_id.clone(),
                Some(id) => Some(id.to_string()),
            };
            let work = std::env::temp_dir().join(format!(
                "token-usage-publish-{}-{}",
                std::process::id(),
                unix_now()
            ));
            let gist_id = push_gist(&bundle, remembered.as_deref(), public, &work)?;
            let _ = std::fs::remove_dir_all(&work);
            cfg.gist_id = Some(gist_id.clone());
            save_github_config(&store_path, &cfg)?;
            let base = url.unwrap_or_else(|| format!("https://gist.github.com/{gist_id}/raw"));
            println!("{gist_id}");
            println!("{}", publish_snippets(&base));
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
        .join(".token-usage")
        .join("store.json")
}
