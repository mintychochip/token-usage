use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use base64::Engine;
use serde_json::{json, Value};
use tempfile::tempdir;
use toktally_cli::identity::load_or_generate_in;
use toktally_widgets_api::{build_app, default_state_dir};
use tower::ServiceExt;

#[tokio::test]
async fn publish_survives_restart_and_pins_key() {
    let dir = tempdir().unwrap();
    let id_a = load_or_generate_in(&tempdir().unwrap().path().join("a")).unwrap();
    let id_b = load_or_generate_in(&tempdir().unwrap().path().join("b")).unwrap();

    let (app, _) = build_app(Some(dir.path().to_path_buf()));
    let body = signed_body_for(&id_a, json!({"input_tokens": 1}));
    assert_eq!(post(&app, body).await.status(), StatusCode::OK);

    let (app2, _) = build_app(Some(dir.path().to_path_buf()));
    let body = signed_body_for(&id_a, json!({"input_tokens": 2}));
    assert_eq!(post(&app2, body).await.status(), StatusCode::OK);

    let mismatched = signed_body_for_uuid(&id_b, &id_a.uuid, json!({"input_tokens": 3}));
    assert_eq!(
        post(&app2, mismatched).await.status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn default_state_dir_contains_widget_file_after_publish() {
    let dir = tempdir().unwrap();
    std::env::set_var("WIDGETS_API_STATE_DIR", dir.path());

    let id = load_or_generate_in(&tempdir().unwrap().path().join("default-dir")).unwrap();
    let (app, state) = build_app(None);
    assert_eq!(state.state_dir, default_state_dir());

    let body = signed_body_for(&id, json!({"input_tokens": 1}));
    assert_eq!(post(&app, body).await.status(), StatusCode::OK);
    assert!(state.state_dir.join(format!("{}.json", id.uuid)).exists());

    std::env::remove_var("WIDGETS_API_STATE_DIR");
}

#[tokio::test]
async fn malformed_state_file_is_treated_as_absent() {
    let dir = tempdir().unwrap();
    let id = load_or_generate_in(&tempdir().unwrap().path().join("malformed")).unwrap();
    std::fs::write(dir.path().join(format!("{}.json", id.uuid)), b"not-json").unwrap();

    let (app, _) = build_app(Some(dir.path().to_path_buf()));
    let body = signed_body_for(&id, json!({"input_tokens": 1}));
    assert_eq!(post(&app, body).await.status(), StatusCode::OK);
}

fn signed_body_for(identity: &toktally_cli::identity::Identity, summary: Value) -> Value {
    signed_body_for_uuid(identity, &identity.uuid, summary)
}

fn signed_body_for_uuid(
    identity: &toktally_cli::identity::Identity,
    uuid: &str,
    summary: Value,
) -> Value {
    let mut body = json!({
        "uuid": uuid,
        "public_key": base64::engine::general_purpose::STANDARD.encode(&identity.public_key),
        "summary": summary,
    });
    let signature = identity.sign_json(&body).unwrap();
    body["signature"] =
        json!(base64::engine::general_purpose::STANDARD.encode(&signature));
    body
}

async fn post(app: &Router, body: Value) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::post("/api/v1/publish")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}
