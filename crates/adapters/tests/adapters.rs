//! Drive each shipped adapter on fixture payloads, then through the store.

use serde_json::Value;
use tempfile::tempdir;
use toktally_adapters::adapt;
use toktally_domain::{
    Harness, ObservationIdentity, ObservationSource, SessionId, SessionStoreCompleteness,
};
use toktally_store::FileStore;

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

fn ingest_and_reload(harness: Harness, payload: &Value) -> toktally_domain::UsageObservation {
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
    assert_eq!(
        loaded.source(),
        ObservationSource::HarnessGlobalApproximation
    );
    assert_eq!(
        loaded.identity().session_id().as_str(),
        SessionId::HARNESS_GLOBAL
    );
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
    assert_eq!(
        loaded.identity().session_id().as_str(),
        payload["sessionId"].as_str().unwrap()
    );
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
fn grok_compaction_extras_preserve_host_before_and_after() {
    let payload = fixture("grok-compacted.json");
    let loaded = ingest_and_reload(Harness::Grok, &payload);
    assert_eq!(
        loaded.counts().input_tokens(),
        payload["contextTokensUsed"].as_u64().unwrap()
    );
    assert_eq!(
        loaded.counts().extras().tokens_before,
        payload["totalTokensBeforeCompaction"].as_u64()
    );
    assert_eq!(
        loaded.counts().extras().tokens_after,
        payload["contextTokensUsed"].as_u64()
    );
    assert_eq!(
        loaded.model(),
        payload["primaryModelId"].as_str(),
        "Grok primaryModelId is the model to price"
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
fn hermes_post_api_request_preserves_fixture_counts() {
    let payload = fixture("hermes-session.json");
    let loaded = ingest_and_reload(Harness::Hermes, &payload);
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
        usage["cache_read_tokens"].as_u64()
    );
    assert_eq!(loaded.identity().session_id().as_str(), "hermes-sess-1");
    assert_eq!(loaded.source(), ObservationSource::PluginReport);
    assert_eq!(
        loaded.model(),
        payload["model"].as_str(),
        "host model must be kept so cost can be looked up"
    );
}

#[test]
fn opencode_session_event_preserves_nested_token_counts() {
    let payload = fixture("opencode-session.json");
    let loaded = ingest_and_reload(Harness::OpenCode, &payload);
    let tokens = &payload["properties"]["tokens"];
    assert_eq!(
        loaded.counts().input_tokens(),
        tokens["input"].as_u64().unwrap()
    );
    assert_eq!(
        loaded.counts().output_tokens(),
        tokens["output"].as_u64().unwrap()
    );
    assert_eq!(
        loaded.counts().extras().cache_read,
        tokens["cache"]["read"].as_u64()
    );
    assert_eq!(loaded.identity().session_id().as_str(), "oc-sess-1");
}

#[test]
fn gemini_cli_stats_preserve_fixture_counts() {
    let payload = fixture("gemini-cli-session.json");
    let loaded = ingest_and_reload(Harness::GeminiCli, &payload);
    let tokens = &payload["stats"]["tokens"];
    assert_eq!(
        loaded.counts().input_tokens(),
        tokens["input"].as_u64().unwrap()
    );
    assert_eq!(
        loaded.counts().output_tokens(),
        tokens["output"].as_u64().unwrap()
    );
    assert_eq!(
        loaded.counts().extras().cache_read,
        tokens["cached"].as_u64()
    );
}

#[test]
fn aider_tokens_dump_preserves_fixture_counts() {
    let payload = fixture("aider-session.json");
    let loaded = ingest_and_reload(Harness::Aider, &payload);
    assert_eq!(
        loaded.counts().input_tokens(),
        payload["tokens"]["input"].as_u64().unwrap()
    );
    assert_eq!(
        loaded.counts().output_tokens(),
        payload["tokens"]["output"].as_u64().unwrap()
    );
}

#[test]
fn goose_session_export_preserves_fixture_counts() {
    let payload = fixture("goose-session.json");
    let loaded = ingest_and_reload(Harness::Goose, &payload);
    assert_eq!(
        loaded.counts().input_tokens(),
        payload["input_tokens"].as_u64().unwrap()
    );
    assert_eq!(
        loaded.counts().output_tokens(),
        payload["output_tokens"].as_u64().unwrap()
    );
    assert_eq!(loaded.identity().session_id().as_str(), "20251228_72");
}

#[test]
fn amp_session_preserves_fixture_counts() {
    let payload = fixture("amp-session.json");
    let loaded = ingest_and_reload(Harness::Amp, &payload);
    assert_eq!(
        loaded.counts().input_tokens(),
        payload["usage"]["input_tokens"].as_u64().unwrap()
    );
    assert_eq!(
        loaded.counts().output_tokens(),
        payload["usage"]["output_tokens"].as_u64().unwrap()
    );
}

#[test]
fn droid_stop_hook_preserves_fixture_counts() {
    let payload = fixture("droid-session.json");
    let loaded = ingest_and_reload(Harness::Droid, &payload);
    assert_eq!(
        loaded.counts().input_tokens(),
        payload["usage"]["input_tokens"].as_u64().unwrap()
    );
    assert_eq!(loaded.identity().session_id().as_str(), "droid-sess-1");
}

#[test]
fn cline_task_usage_preserves_tokens_in_out() {
    let payload = fixture("cline-session.json");
    let loaded = ingest_and_reload(Harness::Cline, &payload);
    assert_eq!(
        loaded.counts().input_tokens(),
        payload["usage"]["tokensIn"].as_u64().unwrap()
    );
    assert_eq!(
        loaded.counts().output_tokens(),
        payload["usage"]["tokensOut"].as_u64().unwrap()
    );
    assert_eq!(loaded.identity().session_id().as_str(), "cline-task-1");
}

#[test]
fn pi_session_stats_preserve_fixture_counts() {
    let payload = fixture("pi-session.json");
    let loaded = ingest_and_reload(Harness::Pi, &payload);
    assert_eq!(
        loaded.counts().input_tokens(),
        payload["tokens"]["input"].as_u64().unwrap()
    );
    assert_eq!(
        loaded.counts().output_tokens(),
        payload["tokens"]["output"].as_u64().unwrap()
    );
    assert_eq!(loaded.identity().session_id().as_str(), "pi-sess-1");
}

#[test]
fn all_adapters_feed_one_store_without_colliding() {
    let (_dir, store) = store();
    let cases = [
        (Harness::ClaudeCode, "claude-code-session.json"),
        (Harness::Codex, "codex-session.json"),
        (Harness::Grok, "grok-partial.json"),
        (Harness::OhMyPi, "oh-my-pi-session.json"),
        (Harness::Jcode, "jcode-session.json"),
        (Harness::Hermes, "hermes-session.json"),
        (Harness::OpenCode, "opencode-session.json"),
        (Harness::GeminiCli, "gemini-cli-session.json"),
        (Harness::Aider, "aider-session.json"),
        (Harness::Goose, "goose-session.json"),
        (Harness::Amp, "amp-session.json"),
        (Harness::Droid, "droid-session.json"),
        (Harness::Cline, "cline-session.json"),
        (Harness::Pi, "pi-session.json"),
    ];
    for (harness, name) in cases {
        let obs = adapt(harness, &fixture(name)).unwrap();
        store.ingest(obs).unwrap();
    }
    assert_eq!(store.list().unwrap().len(), cases.len());

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
