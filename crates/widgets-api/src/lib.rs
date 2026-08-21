//! Minimal widget service for token-usage summaries.
//!
//! POST `/api/v1/publish` — upload a signed summary
//! GET `/u/:uuid/usage-summary.json` — fetch the latest summary
//!
//! State is persisted under [`default_state_dir`] (override with `WIDGETS_API_STATE_DIR`).
//! The HTTP server binds to `127.0.0.1:9474` by default (override with `WIDGETS_API_BIND`).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use base64::Engine;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs,
    io,
    path::PathBuf,
    sync::Arc,
};
use tempfile::NamedTempFile;

const CARD_JS: &str = toktally_cli::USAGE_CARD_JS;
const DEFAULT_BIND: &str = "127.0.0.1:9474";

/// Default bind address when `WIDGETS_API_BIND` is unset.
pub fn default_bind() -> String {
    std::env::var("WIDGETS_API_BIND").unwrap_or_else(|_| DEFAULT_BIND.into())
}

/// Default on-disk state directory when `WIDGETS_API_STATE_DIR` is unset.
pub fn default_state_dir() -> PathBuf {
    std::env::var_os("WIDGETS_API_STATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("toktally-widgets-api"))
}

#[derive(Clone)]
pub struct AppState {
    pub state_dir: PathBuf,
    summaries: Arc<Mutex<HashMap<String, Value>>>,
    public_keys: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct PersistedWidget {
    public_key: String,
    summary: Value,
}

impl AppState {
    fn new(state_dir: PathBuf) -> Self {
        Self {
            state_dir,
            summaries: Arc::new(Mutex::new(HashMap::new())),
            public_keys: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn widget_path(&self, uuid: &str) -> PathBuf {
        self.state_dir.join(format!("{uuid}.json"))
    }

    fn load_widget_from_disk(&self, uuid: &str) -> Option<PersistedWidget> {
        let path = self.widget_path(uuid);
        let data = fs::read_to_string(&path).ok()?;
        serde_json::from_str(&data).ok()
    }

    fn ensure_loaded(&self, uuid: &str) {
        let needs_load = {
            let summaries = self.summaries.lock();
            let public_keys = self.public_keys.lock();
            !summaries.contains_key(uuid) && !public_keys.contains_key(uuid)
        };
        if !needs_load {
            return;
        }

        let Some(record) = self.load_widget_from_disk(uuid) else {
            return;
        };

        let Ok(public_key) = base64::engine::general_purpose::STANDARD.decode(&record.public_key)
        else {
            return;
        };

        self.public_keys
            .lock()
            .entry(uuid.to_string())
            .or_insert(public_key);
        self.summaries
            .lock()
            .entry(uuid.to_string())
            .or_insert(record.summary);
    }

    fn persist_widget(&self, uuid: &str, public_key: &[u8], summary: &Value) -> io::Result<()> {
        fs::create_dir_all(&self.state_dir)?;
        let record = PersistedWidget {
            public_key: base64::engine::general_purpose::STANDARD.encode(public_key),
            summary: summary.clone(),
        };

        let path = self.widget_path(uuid);
        let tmp = NamedTempFile::new_in(&self.state_dir)?;
        serde_json::to_writer_pretty(tmp.as_file(), &record)?;
        tmp.as_file().sync_all()?;
        tmp.persist(&path).map_err(|e| e.error)?;
        Ok(())
    }
}

/// Build the widget API router and shared state.
///
/// When `state_dir` is `None`, [`default_state_dir`] is used.
pub fn build_app(state_dir: Option<PathBuf>) -> (Router, AppState) {
    let state = AppState::new(state_dir.unwrap_or_else(default_state_dir));
    let app = Router::new()
        .route("/api/v1/publish", post(publish))
        .route("/token-usage-card.js", get(get_card_js))
        .route("/u/{uuid}/usage-summary.json", get(get_summary))
        .route("/u/{uuid}", get(get_profile))
        .with_state(state.clone());
    (app, state)
}

async fn publish(State(state): State<AppState>, Json(body): Json<Value>) -> StatusCode {
    let (uuid, summary) = match toktally_cli::widgets_publish::verify_publish_body(&body) {
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

    state.ensure_loaded(&uuid);

    let mut public_keys = state.public_keys.lock();
    if let Some(stored) = public_keys.get(&uuid) {
        if stored != &public_key {
            return StatusCode::FORBIDDEN;
        }
    } else {
        public_keys.insert(uuid.clone(), public_key.clone());
    }
    drop(public_keys);

    state.summaries.lock().insert(uuid.clone(), summary.clone());

    if state.persist_widget(&uuid, &public_key, &summary).is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    StatusCode::OK
}

async fn get_summary(
    State(state): State<AppState>,
    Path(uuid): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    state.ensure_loaded(&uuid);
    let summaries = state.summaries.lock();
    summaries
        .get(&uuid)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn get_card_js() -> (
    [(axum::http::header::HeaderName, &'static str); 1],
    &'static str,
) {
    (
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        CARD_JS,
    )
}

async fn get_profile(Path(uuid): Path<String>) -> Html<String> {
    let summary_url = format!("/u/{uuid}/usage-summary.json");
    let card = toktally_cli::website_embed_html(&summary_url);
    Html(format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"/><title>token usage</title></head><body>{card}</body></html>"
    ))
}

pub async fn serve(bind: impl AsRef<str>, state_dir: Option<PathBuf>) -> io::Result<()> {
    let (app, _) = build_app(state_dir);
    let listener = tokio::net::TcpListener::bind(bind.as_ref()).await?;
    println!("listening on {}", bind.as_ref());
    axum::serve(listener, app).await.map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn malformed_widget_file_is_absent() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("bad.json"), b"not-json").unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        assert!(state.load_widget_from_disk("bad").is_none());
    }

    #[test]
    fn persist_and_reload_widget() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let summary = json!({"input_tokens": 42});
        state
            .persist_widget("uuid-1", &[1, 2, 3], &summary)
            .unwrap();

        let loaded = state.load_widget_from_disk("uuid-1").unwrap();
        assert_eq!(loaded.summary, summary);
        assert_eq!(
            loaded.public_key,
            base64::engine::general_purpose::STANDARD.encode([1, 2, 3])
        );
    }
}
