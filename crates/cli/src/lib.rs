//! Thin HTTP layer over the durable usage store.

mod components;
mod pricing;
pub mod identity;
pub mod widgets_publish;
mod publish;
mod summary;
mod wire;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::Value;
use token_usage_adapters::adapt;
use token_usage_domain::{Harness, ObservationIdentity, SessionId};
use token_usage_store::{FileStore, StoreError};
use token_usage_sync::{sync_all, sync_all_needed, sync_harness, SyncRoots};
use tokio::net::TcpListener;

pub use components::{
    gist_raw_base, github_badge_markdown, join_published_url, publish_snippets,
    render_summary_card, website_embed_html, USAGE_CARD_JS,
};
pub use pricing::{estimate_cost_usd, load_price_table, parse_openrouter_prices, PriceTable};
pub use publish::{
    bundle_from_store, gh_login, load_github_config, pull_dir, pull_gist, push_gist,
    save_github_config, write_bundle, GistRef, GithubConfig, PublishBundle,
};
pub use summary::{
    shields_badge, summarize, summarize_priced, HarnessTotals, ShieldsBadge, UsageSummary,
};
pub use wire::{
    WireHarnessSync, WireObservation, WireSessionList, WireSyncRequest, WireSyncStatus,
};

/// Shared API state. `store` is `None` for a hosted adapt-only process.
#[derive(Clone)]
pub struct ApiState {
    store: Option<Arc<FileStore>>,
    roots: SyncRoots,
}

impl ApiState {
    /// Wrap an opened store. Harness files are read from `TOKEN_USAGE_HARNESS_HOME` or `$HOME`.
    pub fn new(store: FileStore) -> Self {
        Self {
            store: Some(Arc::new(store)),
            roots: SyncRoots::from_env(),
        }
    }

    /// Wrap a store with an explicit harness-home root (tests).
    pub fn with_roots(store: FileStore, roots: SyncRoots) -> Self {
        Self {
            store: Some(Arc::new(store)),
            roots,
        }
    }

    /// Hosted mode: adapt and validate only. Nothing is written.
    pub fn stateless() -> Self {
        Self {
            store: None,
            roots: SyncRoots::from_env(),
        }
    }
}

/// Router for the usage API.
pub fn app(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/adapt/{harness}", post(post_adapt))
        .route("/v1/observations", post(post_observation))
        .route("/v1/ingest/{harness}", post(post_ingest))
        .route("/v1/sessions", get(list_sessions))
        .route("/v1/sessions/{harness}/{session_id}", get(get_session))
        .route("/v1/sync", get(get_sync).post(post_sync))
        .with_state(state)
}

/// Bind `addr` (including `0` for an ephemeral port) and serve until the process exits.
pub async fn serve(store_path: PathBuf, addr: SocketAddr) -> Result<(), std::io::Error> {
    let store = FileStore::open(&store_path).map_err(io_from_store)?;
    let listener = TcpListener::bind(addr).await?;
    let bound = listener.local_addr()?;
    println!("listening on {bound}");
    let _ = std::io::Write::flush(&mut std::io::stdout());
    axum::serve(listener, app(ApiState::new(store))).await
}

/// Bind and serve an API that never opens a store file.
pub async fn serve_stateless(addr: SocketAddr) -> Result<(), std::io::Error> {
    let listener = TcpListener::bind(addr).await?;
    let bound = listener.local_addr()?;
    println!("listening on {bound} (stateless, no store)");
    let _ = std::io::Write::flush(&mut std::io::stdout());
    axum::serve(listener, app(ApiState::stateless())).await
}

async fn health() -> &'static str {
    "ok"
}

async fn post_adapt(
    Path(harness): Path<String>,
    Json(payload): Json<Value>,
) -> Result<(StatusCode, Json<WireObservation>), ApiError> {
    let harness = Harness::parse(&harness).map_err(ApiError::from)?;
    let observation = adapt(harness, &payload).map_err(ApiError::from)?;
    Ok((
        StatusCode::OK,
        Json(WireObservation::from_observation(&observation)),
    ))
}

async fn post_observation(
    State(state): State<ApiState>,
    Json(body): Json<WireObservation>,
) -> Result<(StatusCode, Json<WireObservation>), ApiError> {
    let observation = body.into_observation().map_err(ApiError::from)?;
    let Some(store) = state.store.as_ref() else {
        return Ok((
            StatusCode::OK,
            Json(WireObservation::from_observation(&observation)),
        ));
    };
    maybe_first_sync(&state, observation.identity().harness())?;
    let stored = store.ingest(observation)?;
    Ok((
        StatusCode::OK,
        Json(WireObservation::from_observation(&stored)),
    ))
}

async fn post_ingest(
    State(state): State<ApiState>,
    Path(harness): Path<String>,
    Json(payload): Json<Value>,
) -> Result<(StatusCode, Json<WireObservation>), ApiError> {
    let harness = Harness::parse(&harness).map_err(ApiError::from)?;
    let observation = adapt(harness, &payload).map_err(ApiError::from)?;
    let Some(store) = state.store.as_ref() else {
        return Ok((
            StatusCode::OK,
            Json(WireObservation::from_observation(&observation)),
        ));
    };
    maybe_first_sync(&state, harness)?;
    let stored = store.ingest(observation)?;
    Ok((
        StatusCode::OK,
        Json(WireObservation::from_observation(&stored)),
    ))
}

async fn get_session(
    State(state): State<ApiState>,
    Path((harness, session_id)): Path<(String, String)>,
) -> Result<Json<WireObservation>, ApiError> {
    let identity = ObservationIdentity::new(
        Harness::parse(&harness).map_err(ApiError::from)?,
        SessionId::parse(session_id).map_err(ApiError::from)?,
    );
    let store = state.store.as_ref().ok_or(ApiError::Stateless)?;
    match store.get(&identity)? {
        Some(obs) => Ok(Json(WireObservation::from_observation(&obs))),
        None => Err(ApiError::NotFound),
    }
}

async fn list_sessions(State(state): State<ApiState>) -> Result<Json<WireSessionList>, ApiError> {
    let store = state.store.as_ref().ok_or(ApiError::Stateless)?;
    maybe_first_sync_all(&state)?;
    let sessions = store
        .list()?
        .iter()
        .map(WireObservation::from_observation)
        .collect();
    Ok(Json(WireSessionList { sessions }))
}

async fn get_sync(State(state): State<ApiState>) -> Result<Json<WireSyncStatus>, ApiError> {
    let store = state.store.as_ref().ok_or(ApiError::Stateless)?;
    let harnesses = store
        .list_harness_syncs()?
        .into_iter()
        .map(|row| WireHarnessSync {
            harness: row.harness,
            last_synced_at: row.last_synced_at,
        })
        .collect();
    Ok(Json(WireSyncStatus { harnesses }))
}

async fn post_sync(
    State(state): State<ApiState>,
    Json(body): Json<WireSyncRequest>,
) -> Result<Json<WireSyncStatus>, ApiError> {
    let now = unix_now();
    if let Some(name) = body.harness {
        let harness = Harness::parse(&name).map_err(ApiError::from)?;
        let store = state.store.as_ref().ok_or(ApiError::Stateless)?;
        if body.force || store.needs_first_sync(harness)? {
            sync_harness(store, harness, &state.roots, now)?;
        }
    } else if body.force {
        let store = state.store.as_ref().ok_or(ApiError::Stateless)?;
        sync_all(store, &state.roots, now)?;
    } else {
        let store = state.store.as_ref().ok_or(ApiError::Stateless)?;
        sync_all_needed(store, &state.roots, now)?;
    }
    get_sync(State(state)).await
}

fn maybe_first_sync(state: &ApiState, harness: Harness) -> Result<(), ApiError> {
    let Some(store) = state.store.as_ref() else {
        return Ok(());
    };
    if store.needs_first_sync(harness)? {
        sync_harness(store, harness, &state.roots, unix_now())?;
    }
    Ok(())
}

fn maybe_first_sync_all(state: &ApiState) -> Result<(), ApiError> {
    let Some(store) = state.store.as_ref() else {
        return Ok(());
    };
    if store.list_harness_syncs()?.is_empty() {
        sync_all_needed(store, &state.roots, unix_now())?;
    }
    Ok(())
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// HTTP-facing errors.
#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    NotFound,
    Stateless,
    Internal(String),
}

impl From<token_usage_domain::DomainError> for ApiError {
    fn from(err: token_usage_domain::DomainError) -> Self {
        ApiError::BadRequest(err.to_string())
    }
}

impl From<token_usage_adapters::AdaptError> for ApiError {
    fn from(err: token_usage_adapters::AdaptError) -> Self {
        ApiError::BadRequest(err.to_string())
    }
}

impl From<StoreError> for ApiError {
    fn from(err: StoreError) -> Self {
        ApiError::Internal(err.to_string())
    }
}

impl From<token_usage_sync::SyncError> for ApiError {
    fn from(err: token_usage_sync::SyncError) -> Self {
        ApiError::Internal(err.to_string())
    }
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            ApiError::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            ApiError::Stateless => (
                StatusCode::NOT_IMPLEMENTED,
                "stateless api: no store".to_string(),
            ),
            ApiError::Internal(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
        };
        (status, message).into_response()
    }
}

fn io_from_store(err: StoreError) -> std::io::Error {
    std::io::Error::other(err)
}
