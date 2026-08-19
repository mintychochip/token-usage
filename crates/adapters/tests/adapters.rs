//! Drive each shipped adapter on fixture payloads, then through the store.

use serde_json::Value;
use tempfile::tempdir;
use token_usage_adapters::adapt;
use token_usage_domain::{
    Harness, ObservationIdentity, ObservationSource, SessionId, SessionStoreCompleteness,
};
use token_usage_store::FileStore;

fn fixture(name: &str) -> Value {
    let path = format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse {path}: {e}"))
}

fn store() -> (tempfile::TempDir, FileStore) {
    let dir = tempdir().unwrap();
    let store = FileStore::open(dir.path().join("store.json")).unwrap();
    (dir, store)
}

fn ingest_and_reload(harness: Harness, payload: &Value) -> token_usage_domain::UsageObservation {
    let (_dir, store) = store();
    let observation = adapt(harness, payload).expect("adapt");
    store.ingest(observation.clone()).unwrap();
    store
        .get(observation.identity())
        .unwrap()
        .expect("read back")
}

#[test]
fn claude_code_session_hook_preserves_fixture_counts() {
    let payload = fixture("claude-code-session.json");
    let loaded = ingest_and_reload(Harness::ClaudeCode, &payload);
    let usage = &payload["usage"];
    assert_eq!(
        loaded.counts().input_tokens(),
        usage["input_tokens"].as_u64().unwrap()
    );
    assert_eq!(
        loaded.counts().output_tokens(),
        usage["output_tokens"].as_u64().unwrap()
    );
    assert_eq!(
        loaded.counts().extras().cache_read,
        usage["cache_read_input_tokens"].as_u64()
    );
    assert_eq!(loaded.identity().session_id().as_str(), "cc-sess-1");
    assert_eq!(loaded.source(), ObservationSource::PluginReport);
    assert_eq!(loaded.completeness(), SessionStoreCompleteness::Complete);
}

#[test]
fn claude_code_global_snapshot_is_a_harness_approximation() {
    let payload = fixture("claude-code-global.json");
    let loaded = ingest_and_reload(Harness::ClaudeCode, &payload);
    assert_eq!(
        loaded.counts().input_tokens(),
        payload["input_tokens"].as_u64().unwrap()
    );
    assert_eq!(
        loaded.counts().output_tokens(),
        payload["output_tokens"].as_u64().unwrap()
    );
    assert_eq!(loaded.source(), ObservationSource::HarnessGlobalApproximation);
    assert_eq!(loaded.identity().session_id().as_str(), SessionId::HARNESS_GLOBAL);
}

#[test]
fn codex_stop_hook_preserves_fixture_counts() {
    let payload = fixture("codex-session.json");
    let loaded = ingest_and_reload(Harness::Codex, &payload);
    let usage = &payload["token_usage"];
    assert_eq!(
        loaded.counts().input_tokens(),
        usage["input_tokens"].as_u64().unwrap()
    );
    assert_eq!(
        loaded.counts().output_tokens(),
        usage["output_tokens"].as_u64().unwrap()
    );
    assert_eq!(
        loaded.counts().extras().cache_read,
        usage["cached_input_tokens"].as_u64()
    );
    assert_eq!(loaded.source(), ObservationSource::PluginReport);
    assert_eq!(loaded.completeness(), SessionStoreCompleteness::Complete);
}

#[test]
fn grok_signals_fragment_is_partial_or_unknown() {
    let payload = fixture("grok-partial.json");
    let loaded = ingest_and_reload(Harness::Grok, &payload);
    assert_eq!(
        loaded.counts().input_tokens(),
        payload["contextTokensUsed"].as_u64().unwrap()
    );
    assert_eq!(loaded.identity().session_id().as_str(), payload["sessionId"].as_str().unwrap());
    assert_eq!(loaded.source(), ObservationSource::PluginReport);
    assert!(
        matches!(
            loaded.completeness(),
            SessionStoreCompleteness::Partial | SessionStoreCompleteness::Unknown
        ),
        "Grok fragments must not claim a complete session store"
    );
}

#[test]
fn oh_my_pi_session_stats_preserve_fixture_counts() {
    let payload = fixture("oh-my-pi-session.json");
    let loaded = ingest_and_reload(Harness::OhMyPi, &payload);
    let stats = &payload["stats"];
    assert_eq!(
        loaded.counts().input_tokens(),
        stats["inputTokens"].as_u64().unwrap()
    );
    assert_eq!(
        loaded.counts().output_tokens(),
        stats["outputTokens"].as_u64().unwrap()
    );
    assert_eq!(
        loaded.counts().extras().cache_read,
        stats["cacheReadTokens"].as_u64()
    );
    assert_eq!(loaded.source(), ObservationSource::PluginReport);
    assert_eq!(loaded.completeness(), SessionStoreCompleteness::Complete);
}

#[test]
fn jcode_usage_preserves_prompt_and_completion_tokens() {
    let payload = fixture("jcode-session.json");
    let loaded = ingest_and_reload(Harness::Jcode, &payload);
    let usage = &payload["usage"];
    assert_eq!(
        loaded.counts().input_tokens(),
        usage["prompt_tokens"].as_u64().unwrap()
    );
    assert_eq!(
        loaded.counts().output_tokens(),
        usage["completion_tokens"].as_u64().unwrap()
    );
    assert_eq!(loaded.source(), ObservationSource::PluginReport);
    assert_eq!(loaded.completeness(), SessionStoreCompleteness::Complete);
}

#[test]
fn five_adapters_feed_one_store_without_colliding() {
    let (_dir, store) = store();
    let cases = [
        (Harness::ClaudeCode, "claude-code-session.json"),
        (Harness::Codex, "codex-session.json"),
        (Harness::Grok, "grok-partial.json"),
        (Harness::OhMyPi, "oh-my-pi-session.json"),
        (Harness::Jcode, "jcode-session.json"),
    ];
    for (harness, name) in cases {
        let obs = adapt(harness, &fixture(name)).unwrap();
        store.ingest(obs).unwrap();
    }
    assert_eq!(store.list().unwrap().len(), 5);

    let first = adapt(Harness::Codex, &fixture("codex-session.json")).unwrap();
    let mut updated_payload = fixture("codex-session.json");
    updated_payload["token_usage"]["input_tokens"] = serde_json::json!(9100);
    let second = adapt(Harness::Codex, &updated_payload).unwrap();
    store.ingest(second).unwrap();
    let listed = store.list().unwrap();
    let codex: Vec<_> = listed
        .iter()
        .filter(|row| row.identity().harness() == Harness::Codex)
        .collect();
    assert_eq!(codex.len(), 1);
    assert_eq!(codex[0].counts().input_tokens(), 9100);
    assert_eq!(
        store
            .get(&ObservationIdentity::new(
                Harness::Codex,
                first.identity().session_id().clone(),
            ))
            .unwrap()
            .unwrap()
            .counts()
            .input_tokens(),
        9100
    );
}
