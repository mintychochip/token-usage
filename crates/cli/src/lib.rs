//! Thin HTTP layer over the durable usage store.

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
use tokio::net::TcpListener;

pub use wire::{WireObservation, WireSessionList};

/// Shared API state: one file-backed store.
#[derive(Clone)]
pub struct ApiState {
    store: Arc<FileStore>,
}

impl ApiState {
    /// Wrap an opened store.
    pub fn new(store: FileStore) -> Self {
        Self {
            store: Arc::new(store),
        }
    }
}

/// Router for the usage API.
pub fn app(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/observations", post(post_observation))
        .route("/v1/ingest/{harness}", post(post_ingest))
        .route("/v1/sessions", get(list_sessions))
        .route("/v1/sessions/{harness}/{session_id}", get(get_session))
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

async fn health() -> &'static str {
    "ok"
}

async fn post_observation(
    State(state): State<ApiState>,
    Json(body): Json<WireObservation>,
) -> Result<(StatusCode, Json<WireObservation>), ApiError> {
    let observation = body.into_observation().map_err(ApiError::from)?;
    let stored = state.store.ingest(observation)?;
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
    let stored = state.store.ingest(observation)?;
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
    match state.store.get(&identity)? {
        Some(obs) => Ok(Json(WireObservation::from_observation(&obs))),
        None => Err(ApiError::NotFound),
    }
}

async fn list_sessions(State(state): State<ApiState>) -> Result<Json<WireSessionList>, ApiError> {
    let sessions = state
        .store
        .list()?
        .iter()
        .map(WireObservation::from_observation)
        .collect();
    Ok(Json(WireSessionList { sessions }))
}

/// HTTP-facing errors.
#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    NotFound,
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

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            ApiError::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            ApiError::Internal(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
        };
        (status, message).into_response()
    }
}

fn io_from_store(err: StoreError) -> std::io::Error {
    std::io::Error::other(err)
}
