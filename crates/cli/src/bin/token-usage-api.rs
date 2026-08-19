//! HTTP entry point for the token-usage store.

use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    let bind = env::var("TOKEN_USAGE_BIND").unwrap_or_else(|_| "127.0.0.1:9473".to_string());
    let addr: SocketAddr = bind.parse().unwrap_or_else(|err| {
        eprintln!("invalid TOKEN_USAGE_BIND {bind:?}: {err}");
        std::process::exit(2);
    });
    let stateless = env::var("TOKEN_USAGE_STATELESS")
        .map(|v| matches!(v.as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);
    let result = if stateless {
        token_usage_cli::serve_stateless(addr).await
    } else {
        let store = env::var("TOKEN_USAGE_STORE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_store_path());
        token_usage_cli::serve(store, addr).await
    };
    if let Err(err) = result {
        eprintln!("api error: {err}");
        std::process::exit(1);
    }
}

fn default_store_path() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".token-usage")
        .join("store.json")
}
