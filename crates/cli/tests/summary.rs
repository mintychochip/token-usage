//! Drive summarize() on observations that went through the real store.

use tempfile::tempdir;
use token_usage_cli::{shields_badge, summarize};
use token_usage_domain::{
    Harness, ObservationIdentity, ObservationSource, SessionId, SessionStoreCompleteness,
    UsageCounts, UsageObservation,
};
use token_usage_store::FileStore;

#[test]
fn summary_adds_sessions_per_harness_and_omits_session_ids() {
    let dir = tempdir().unwrap();
    let store = FileStore::open(dir.path().join("store.json")).unwrap();
    store
        .ingest_at(
            UsageObservation::new(
                ObservationIdentity::new(Harness::Grok, SessionId::parse("a").unwrap()),
                UsageCounts::new(100, 10),
                ObservationSource::PluginReport,
                SessionStoreCompleteness::Unknown,
            ),
            10,
        )
        .unwrap();
    store
        .ingest_at(
            UsageObservation::new(
                ObservationIdentity::new(Harness::Grok, SessionId::parse("b").unwrap()),
                UsageCounts::new(50, 5),
                ObservationSource::PluginReport,
                SessionStoreCompleteness::Unknown,
            ),
            20,
        )
        .unwrap();
    store
        .ingest_at(
            UsageObservation::new(
                ObservationIdentity::new(Harness::Hermes, SessionId::parse("h").unwrap()),
                UsageCounts::new(7, 1),
                ObservationSource::PluginReport,
                SessionStoreCompleteness::Complete,
            ),
            15,
        )
        .unwrap();

    let listed = store.list().unwrap();
    let summary = summarize(&listed, 99);
    assert_eq!(summary.generated_at, 99);
    assert_eq!(summary.input_tokens, 157);
    assert_eq!(summary.output_tokens, 16);
    let grok = summary
        .harnesses
        .iter()
        .find(|h| h.harness == Harness::Grok)
        .unwrap();
    assert_eq!(grok.sessions, 2);
    assert_eq!(grok.input_tokens, 150);
    assert_eq!(grok.last_synced_at, Some(20));
    let text = serde_json::to_string(&summary).unwrap();
    assert!(
        !text.contains("\"session_id\""),
        "public summary must not leak session ids: {text}"
    );

    let badge = shields_badge(&summary);
    assert_eq!(badge.schema_version, 1);
    assert!(badge.message.contains("157") || badge.message.contains("in"));
}

#[test]
fn summary_saturates_at_u64_max_instead_of_wrapping_or_panicking() {
    let dir = tempdir().unwrap();
    let store = FileStore::open(dir.path().join("store.json")).unwrap();
    for session in ["a", "b"] {
        store
            .ingest(UsageObservation::new(
                ObservationIdentity::new(Harness::Codex, SessionId::parse(session).unwrap()),
                UsageCounts::new(u64::MAX, u64::MAX),
                ObservationSource::PluginReport,
                SessionStoreCompleteness::Complete,
            ))
            .unwrap();
    }
    let summary = summarize(&store.list().unwrap(), 1);
    assert_eq!(summary.input_tokens, u64::MAX, "must saturate, not wrap");
    assert_eq!(summary.output_tokens, u64::MAX);
    let codex = summary
        .harnesses
        .iter()
        .find(|h| h.harness == Harness::Codex)
        .unwrap();
    assert_eq!(codex.input_tokens, u64::MAX);
    assert_eq!(codex.sessions, 2);
    // A saturating summary must still serialize (no NaN/overflow panic).
    let _ = serde_json::to_string(&summary).unwrap();
}

#[test]
fn summary_skips_global_approximation_when_session_reports_exist() {
    let dir = tempdir().unwrap();
    let store = FileStore::open(dir.path().join("store.json")).unwrap();
    store
        .ingest_at(
            UsageObservation::new(
                ObservationIdentity::new(Harness::Hermes, SessionId::parse("sess").unwrap()),
                UsageCounts::new(18000, 2200),
                ObservationSource::PluginReport,
                SessionStoreCompleteness::Complete,
            )
            .with_model("anthropic/claude-sonnet-4.6"),
            10,
        )
        .unwrap();
    store
        .ingest_at(
            UsageObservation::new(
                ObservationIdentity::new(Harness::Hermes, SessionId::harness_global()),
                UsageCounts::new(18000, 2200),
                ObservationSource::HarnessGlobalApproximation,
                SessionStoreCompleteness::Partial,
            )
            .with_model("anthropic/claude-sonnet-4.6"),
            11,
        )
        .unwrap();

    let summary = summarize(&store.list().unwrap(), 1);
    assert_eq!(summary.input_tokens, 18000);
    assert_eq!(summary.output_tokens, 2200);
    assert_eq!(summary.harnesses[0].sessions, 1);
}
