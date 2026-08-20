//! The recommended remote payload is the shipped WireObservation, not a new schema.
//! Plugins stay an exec of the reporter; FileStore is the state that would move remote.

use std::fs;

use toktally_cli::WireObservation;
use toktally_domain::{
    ExtraCounts, Harness, ObservationIdentity, ObservationSource, SessionId,
    SessionStoreCompleteness, UsageCounts, UsageObservation,
};

fn sample() -> UsageObservation {
    UsageObservation::new(
        ObservationIdentity::new(Harness::Grok, SessionId::parse("sess-alpha").unwrap()),
        UsageCounts::new(1111, 0).with_extras(ExtraCounts {
            cache_read: Some(3),
            cache_write: None,
            reasoning: None,
            ..ExtraCounts::default()
        }),
        ObservationSource::PluginReport,
        SessionStoreCompleteness::Unknown,
    )
    .with_last_synced_at(1_700_000_042)
}

#[test]
fn remote_report_fields_are_the_existing_wire_observation() {
    let json = serde_json::to_value(WireObservation::from_observation(&sample())).unwrap();
    for key in [
        "harness",
        "session_id",
        "input_tokens",
        "output_tokens",
        "source",
        "completeness",
        "extras",
        "last_synced_at",
    ] {
        assert!(
            json.get(key).is_some(),
            "WireObservation JSON missing {key}: {json}"
        );
    }
    assert_eq!(json["harness"], "grok");
    assert_eq!(json["session_id"], "sess-alpha");
    assert_eq!(json["input_tokens"], 1111);
    assert_eq!(json["output_tokens"], 0);
    assert_eq!(json["source"], "plugin_report");
    assert_eq!(json["completeness"], "unknown");
    assert_eq!(json["extras"]["cache_read"], 3);
    assert_eq!(json["last_synced_at"], 1_700_000_042);
}

#[test]
fn remote_format_doc_recommends_github_and_keeps_plugins_stateless() {
    let path = format!("{}/../../docs/remote-format.md", env!("CARGO_MANIFEST_DIR"));
    let doc = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    assert!(
        doc.contains("POST /v1/observations"),
        "{path} must cover the HTTP API"
    );
    assert!(
        doc.contains("s3://") || doc.contains("gist"),
        "{path} must cover a keyed object-store or gist document"
    );
    assert!(
        doc.contains("JSONL") || doc.contains("jsonl"),
        "{path} must cover an append-only JSONL log"
    );
    assert!(
        doc.contains("exec") && doc.contains("toktally"),
        "{path} must cite the plugin wrapper exec"
    );
    assert!(
        doc.contains("FileStore"),
        "{path} must name FileStore as the state being fronted"
    );
    assert!(
        doc.contains("Recommendation"),
        "{path} must contain a Recommendation section"
    );
    assert!(
        doc.contains("publish --gist") && doc.contains("WireObservation"),
        "recommendation must name GitHub gist publish and WireObservation"
    );
    for field in [
        "harness",
        "session_id",
        "input_tokens",
        "output_tokens",
        "source",
        "completeness",
    ] {
        assert!(
            doc.contains(field),
            "recommendation must list shipped field {field}"
        );
    }
}

#[test]
fn host_wrappers_still_exec_reporter_and_do_not_hold_the_store() {
    let root = format!("{}/../..", env!("CARGO_MANIFEST_DIR"));
    let body = fs::read_to_string(format!("{root}/plugins/grok/scripts/report.sh")).unwrap();
    assert!(body.contains("exec"));
    assert!(body.contains("toktally"));
    assert!(body.contains("ingest"));
    assert!(
        !body.contains("store.json") && !body.contains("FileStore"),
        "plugin wrapper must not manage FileStore"
    );
}
