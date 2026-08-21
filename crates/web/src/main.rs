//! HTTP web entry point for the toktally store.
//!
//! This binary delegates to the HTTP implementation in this crate and uses
//! the local JSON `FileStore`. SQLite/multi-tenant backends are left for a
//! later slice.

use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    let bind = env::var("TOKTALLY_BIND")
        .or_else(|_| env::var("TOKEN_USAGE_BIND"))
        .unwrap_or_else(|_| "127.0.0.1:9473".to_string());
    let addr: SocketAddr = bind.parse().unwrap_or_else(|err| {
        eprintln!("invalid TOKTALLY_BIND {bind:?}: {err}");
        std::process::exit(2);
    });
    let stateless = env::var("TOKTALLY_STATELESS")
        .or_else(|_| env::var("TOKEN_USAGE_STATELESS"))
        .map(|v| matches!(v.as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);
    let result = if stateless {
        toktally_web::serve_stateless(addr).await
    } else {
        let store = env::var("TOKTALLY_STORE")
            .or_else(|_| env::var("TOKEN_USAGE_STORE"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_store_path());
        toktally_web::serve(store, addr).await
    };
    if let Err(err) = result {
        eprintln!("web error: {err}");
        std::process::exit(1);
    }
}

fn default_store_path() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".toktally")
        .join("store.json")
}
