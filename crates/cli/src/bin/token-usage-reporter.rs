//! Plugin-facing reporter: adapt a harness payload and ingest it.

use std::io::{self, Read};
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use token_usage_adapters::adapt;
use token_usage_cli::WireObservation;
use token_usage_domain::{Harness, ObservationIdentity, SessionId};
use token_usage_store::FileStore;

#[derive(Parser)]
#[command(
    name = "token-usage-reporter",
    about = "Report harness token usage into the shared store"
)]
struct Cli {
    /// Path to the durable store (JSON file).
    #[arg(long, env = "TOKEN_USAGE_STORE", global = true)]
    store: Option<PathBuf>,
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
    match cli.command {
        Command::Ingest { adapter, file } => {
            let harness = Harness::parse(&adapter)?;
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
        Command::List => {
            let sessions: Vec<_> = store
                .list()?
                .iter()
                .map(WireObservation::from_observation)
                .collect();
            println!("{}", serde_json::to_string_pretty(&sessions)?);
        }
    }
    Ok(())
}

fn default_store_path() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".token-usage")
        .join("store.json")
}
