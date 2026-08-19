//! Plugin-facing reporter: adapt a harness payload and ingest it.

use std::io::{self, Read};
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use token_usage_adapters::adapt;
use token_usage_cli::{WireHarnessSync, WireObservation, WireSyncStatus};
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
    /// Write every stored observation as JSONL (user-owned format).
    Export {
        /// Destination file; stdout when omitted.
        #[arg(long)]
        file: Option<PathBuf>,
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
    let store = FileStore::open(store_path)?;
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
        Command::Export { file } => {
            let mut out = String::new();
            for obs in store.list()? {
                let wire = WireObservation::from_observation(&obs);
                out.push_str(&serde_json::to_string(&wire)?);
                out.push('\n');
            }
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
        Command::Sync { harness, force } => {
            let now = unix_now();
            if let Some(name) = harness {
                let harness = Harness::parse(&name)?;
                if force || store.needs_first_sync(harness)? {
                    sync_harness(&store, harness, &roots, now)?;
                }
            } else if force {
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
