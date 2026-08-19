//! Drive the shipped API router and the token-usage-api binary.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tempfile::tempdir;
use token_usage_cli::{app, ApiState, WireObservation};
use token_usage_domain::{
    ExtraCounts, Harness, ObservationIdentity, ObservationSource, SessionId,
    SessionStoreCompleteness, UsageCounts, UsageObservation,
};
use token_usage_store::FileStore;
use token_usage_sync::SyncRoots;
use tower::ServiceExt;

fn sample_obs() -> UsageObservation {
    UsageObservation::new(
        ObservationIdentity::new(Harness::Codex, SessionId::parse("thr_api").unwrap()),
        UsageCounts::new(9000, 1100).with_extras(ExtraCounts {
            cache_read: Some(3000),
            cache_write: None,
            reasoning: None,
        }),
        ObservationSource::PluginReport,
        SessionStoreCompleteness::Complete,
    )
}

#[tokio::test]
async fn router_post_then_get_returns_submitted_counts() {
    let dir = tempdir().unwrap();
    let store = FileStore::open(dir.path().join("store.json")).unwrap();
    let empty_home = dir.path().join("home");
    std::fs::create_dir_all(&empty_home).unwrap();
    let router = app(ApiState::with_roots(store, SyncRoots { home: empty_home }));
    let wire = WireObservation::from_observation(&sample_obs());
    let body = serde_json::to_vec(&wire).unwrap();

    let post = router
        .clone()
        .oneshot(
            Request::post("/v1/observations")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(post.status(), StatusCode::OK);
    let posted: WireObservation =
        serde_json::from_slice(&post.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(posted.input_tokens, 9000);
    assert_eq!(posted.output_tokens, 1100);
    assert_eq!(posted.harness, Harness::Codex);
    assert_eq!(posted.session_id, "thr_api");

    let get = router
        .oneshot(
            Request::get("/v1/sessions/codex/thr_api")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get.status(), StatusCode::OK);
    let got: WireObservation =
        serde_json::from_slice(&get.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(got.input_tokens, 9000);
    assert_eq!(got.output_tokens, 1100);
    assert_eq!(got.session_id, "thr_api");
    assert_eq!(got.source, ObservationSource::PluginReport);
    assert!(got.last_synced_at.is_some());
}

#[test]
fn api_binary_roundtrip_returns_submitted_identity_and_counts() {
    let dir = tempdir().unwrap();
    let store = dir.path().join("store.json");
    let _guard = KillOnDrop(spawn_api(&store));
    let addr = read_listen_addr();
    let body = serde_json::json!({
        "harness": "grok",
        "session_id": "sess-launch",
        "input_tokens": 212362,
        "output_tokens": 44,
        "source": "plugin_report",
        "completeness": "unknown"
    });
    let (status, posted) = http_json("POST", &addr, "/v1/observations", Some(&body.to_string()));
    assert_eq!(status, 200, "post body: {posted}");
    assert!(posted.contains("212362"), "{posted}");
    assert!(posted.contains("sess-launch"), "{posted}");
    assert!(posted.contains("grok"), "{posted}");

    let (status, got) = http_json("GET", &addr, "/v1/sessions/grok/sess-launch", None);
    assert_eq!(status, 200, "get body: {got}");
    assert!(got.contains("212362"), "{got}");
    assert!(got.contains("44"), "{got}");
    assert!(got.contains("sess-launch"), "{got}");
    assert!(got.contains("unknown"), "{got}");
}

fn spawn_api(store: &std::path::Path) -> std::process::Child {
    let bin = env!("CARGO_BIN_EXE_token-usage-api");
    let home = store.parent().unwrap().join("harness-home");
    std::fs::create_dir_all(&home).unwrap();
    let mut child = Command::new(bin)
        .env("TOKEN_USAGE_STORE", store)
        .env("TOKEN_USAGE_BIND", "127.0.0.1:0")
        .env("TOKEN_USAGE_HARNESS_HOME", &home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn token-usage-api");
    let stdout = child.stdout.take().expect("stdout");
    // Stash the pipe on a side channel so read_listen_addr can consume it.
    LISTEN_STDOUT.lock().unwrap().replace(stdout);
    child
}

fn read_listen_addr() -> String {
    let stdout = LISTEN_STDOUT.lock().unwrap().take().expect("api stdout");
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = std::io::BufReader::new(stdout);
        let mut line = String::new();
        let _ = std::io::BufRead::read_line(&mut reader, &mut line);
        let _ = tx.send(line);
    });
    let line = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("api did not print a listening line");
    parse_listen_line(line.trim()).unwrap_or_else(|| panic!("bad listening line: {line:?}"))
}

static LISTEN_STDOUT: std::sync::Mutex<Option<std::process::ChildStdout>> =
    std::sync::Mutex::new(None);

fn parse_listen_line(line: &str) -> Option<String> {
    line.strip_prefix("listening on ")
        .map(|rest| rest.trim().to_string())
}

fn http_json(method: &str, addr: &str, path: &str, body: Option<&str>) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).unwrap_or_else(|e| panic!("connect {addr}: {e}"));
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let extra = match body {
        Some(payload) => format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            payload.len(),
            payload
        ),
        None => "\r\n".to_string(),
    };
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n{extra}"
    )
    .unwrap();
    let mut resp = String::new();
    stream.read_to_string(&mut resp).unwrap();
    let status = resp
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let body = resp.split("\r\n\r\n").nth(1).unwrap_or(&resp).to_string();
    (status, body)
}

struct KillOnDrop(std::process::Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
