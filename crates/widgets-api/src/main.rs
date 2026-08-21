//! Minimal widget service for token-usage summaries.
//!
//! POST /api/v1/publish   — upload a signed summary
//! GET  /u/:uuid/usage-summary.json — fetch the latest summary

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use base64::Engine;
use parking_lot::Mutex;
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};

const CARD_JS: &str = token_usage_cli::USAGE_CARD_JS;

#[derive(Default, Clone)]
struct AppState {
    summaries: Arc<Mutex<HashMap<String, Value>>>,
    public_keys: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

async fn publish(State(state): State<AppState>, Json(body): Json<Value>) -> StatusCode {
    let (uuid, summary) = match token_usage_cli::widgets_publish::verify_publish_body(&body) {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_REQUEST,
    };

    let Some(public_key_b64) = body.get("public_key").and_then(|v| v.as_str()) else {
        return StatusCode::BAD_REQUEST;
    };

    let public_key = match base64::engine::general_purpose::STANDARD.decode(public_key_b64) {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_REQUEST,
    };

    let mut public_keys = state.public_keys.lock();
    if let Some(stored) = public_keys.get(&uuid) {
        if stored != &public_key {
            return StatusCode::FORBIDDEN;
        }
    } else {
        public_keys.insert(uuid.clone(), public_key);
    }

    let mut summaries = state.summaries.lock();
    summaries.insert(uuid, summary);

    StatusCode::OK
}

async fn get_summary(
    State(state): State<AppState>,
    Path(uuid): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let summaries = state.summaries.lock();
    summaries
        .get(&uuid)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}
async fn get_card_js() -> ([(axum::http::header::HeaderName, &'static str); 1], &'static str) {
    ([(axum::http::header::CONTENT_TYPE, "application/javascript")], CARD_JS)
}

async fn get_profile(Path(uuid): Path<String>) -> Html<String> {
    let summary_url = format!("/u/{uuid}/usage-summary.json");
    let card = token_usage_cli::website_embed_html(&summary_url);
    Html(format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"/><title>token usage</title></head><body>{card}</body></html>"
    ))
}

#[tokio::main]
async fn main() {
    let state = AppState::default();
    let app = Router::new()
        .route("/api/v1/publish", post(publish))
        .route("/token-usage-card.js", get(get_card_js))
        .route("/u/{uuid}/usage-summary.json", get(get_summary))
        .route("/u/{uuid}", get(get_profile))
        .with_state(state);

    let bind = std::env::var("WIDGETS_API_BIND").unwrap_or_else(|_| "0.0.0.0:9474".into());
    let listener = tokio::net::TcpListener::bind(&bind).await.unwrap();
    println!("listening on {bind}");
    axum::serve(listener, app).await.unwrap();
}
