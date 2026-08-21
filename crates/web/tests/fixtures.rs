//! Drive the toktally web server with every adapter fixture.

use std::path::PathBuf;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tempfile::tempdir;
use toktally_domain::Harness;
use toktally_store::FileStore;
use toktally_sync::SyncRoots;
use toktally_web::{app, ApiState, WireObservation, WireSessionList};
use tower::ServiceExt;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../adapters/fixtures")
}

fn harness_from_filename(name: &str) -> Harness {
    let stem = name.strip_suffix(".json").unwrap_or(name);
    let mut parts: Vec<_> = stem.split('-').collect();
    const SUFFIXES: &[&str] = &["session", "global", "compacted", "partial"];
    if let Some(last) = parts.last() {
        if SUFFIXES.contains(last) {
            parts.pop();
        }
    }
    let slug = parts.join("-");
    Harness::parse(&slug).unwrap_or_else(|_| panic!("unknown harness for fixture {name:?}: {slug}"))
}

fn fixture_names() -> Vec<String> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(fixture_dir()).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            names.push(path.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    names.sort();
    names
}

#[tokio::test]
async fn all_adapter_fixtures_round_trip_through_web_api() {
    let dir = tempdir().unwrap();
    let store = FileStore::open(dir.path().join("store.json")).unwrap();
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let router = app(ApiState::with_roots(store, SyncRoots { home }));

    let names = fixture_names();
    assert!(!names.is_empty(), "expected adapter fixtures");

    for name in &names {
        let harness = harness_from_filename(name);
        let payload = std::fs::read_to_string(fixture_dir().join(name)).unwrap();
        let response = router
            .clone()
            .oneshot(
                Request::post(format!("/v1/ingest/{}", harness.as_str()))
                    .header("content-type", "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "ingest {name} for {harness} failed"
        );

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let posted: WireObservation = serde_json::from_slice(&body).unwrap();

        let get = router
            .clone()
            .oneshot(
                Request::get(format!(
                    "/v1/sessions/{}/{}",
                    posted.harness.as_str(),
                    posted.session_id
                ))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(get.status(), StatusCode::OK, "get {name} failed");
        let got: WireObservation =
            serde_json::from_slice(&get.into_body().collect().await.unwrap().to_bytes()).unwrap();

        assert_eq!(got.harness, posted.harness);
        assert_eq!(got.session_id, posted.session_id);
        assert_eq!(got.input_tokens, posted.input_tokens);
        assert_eq!(got.output_tokens, posted.output_tokens);
        assert_eq!(got.source, posted.source);
        assert_eq!(got.completeness, posted.completeness);
    }

    let list = router
        .oneshot(Request::get("/v1/sessions").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let listed: WireSessionList =
        serde_json::from_slice(&list.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(
        listed.sessions.len(),
        names.len(),
        "list must return one observation per fixture"
    );
    for name in &names {
        let harness = harness_from_filename(name);
        assert!(
            listed.sessions.iter().any(|s| s.harness == harness),
            "list must contain a session for {name}"
        );
    }
}
