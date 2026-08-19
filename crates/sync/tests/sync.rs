//! Drive discovery and first-use sync against fixture harness stores.

use std::path::PathBuf;

use tempfile::tempdir;
use token_usage_domain::{Harness, ObservationIdentity, SessionId};
use token_usage_store::FileStore;
use token_usage_sync::{discover, sync_all_needed, sync_harness, SyncRoots};

fn fixture_home() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/home")
}

fn roots() -> SyncRoots {
    SyncRoots {
        home: fixture_home(),
    }
}

fn open_store() -> (tempfile::TempDir, FileStore) {
    let dir = tempdir().unwrap();
    let store = FileStore::open(dir.path().join("store.json")).unwrap();
    (dir, store)
}

#[test]
fn grok_discovery_finds_every_session_not_just_one() {
    let payloads = discover(Harness::Grok, &roots()).unwrap();
    assert_eq!(payloads.len(), 2, "expected both sess-alpha and sess-beta");
    let ids: Vec<_> = payloads
        .iter()
        .map(|p| p["sessionId"].as_str().unwrap().to_string())
        .collect();
    assert!(ids.contains(&"sess-alpha".to_string()));
    assert!(ids.contains(&"sess-beta".to_string()));
}

#[test]
fn grok_sync_ingests_both_sessions_and_records_last_synced() {
    let (_dir, store) = open_store();
    assert!(store.needs_first_sync(Harness::Grok).unwrap());
    let report = sync_harness(&store, Harness::Grok, &roots(), 1_700_000_042).unwrap();
    assert_eq!(report.ingested, 2);
    assert_eq!(report.last_synced_at, 1_700_000_042);
    assert!(!store.needs_first_sync(Harness::Grok).unwrap());
    assert_eq!(
        store.harness_last_synced(Harness::Grok).unwrap(),
        Some(1_700_000_042)
    );

    let alpha = store
        .get(&ObservationIdentity::new(
            Harness::Grok,
            SessionId::parse("sess-alpha").unwrap(),
        ))
        .unwrap()
        .unwrap();
    let beta = store
        .get(&ObservationIdentity::new(
            Harness::Grok,
            SessionId::parse("sess-beta").unwrap(),
        ))
        .unwrap()
        .unwrap();
    assert_eq!(alpha.counts().input_tokens(), 1111);
    assert_eq!(beta.counts().input_tokens(), 2222);
    assert_eq!(alpha.last_synced_at(), Some(1_700_000_042));
    assert_eq!(beta.last_synced_at(), Some(1_700_000_042));
}

#[test]
fn pi_jsonl_sums_every_turn_in_the_historical_session() {
    let (_dir, store) = open_store();
    let report = sync_harness(&store, Harness::Pi, &roots(), 9).unwrap();
    assert_eq!(report.ingested, 1);
    let loaded = store
        .get(&ObservationIdentity::new(
            Harness::Pi,
            SessionId::parse("pi-hist").unwrap(),
        ))
        .unwrap()
        .unwrap();
    assert_eq!(loaded.counts().input_tokens(), 150);
    assert_eq!(loaded.counts().output_tokens(), 30);
    assert_eq!(loaded.counts().extras().cache_read, Some(12));
}

#[test]
fn oh_my_pi_historical_session_is_synced() {
    let (_dir, store) = open_store();
    let report = sync_harness(&store, Harness::OhMyPi, &roots(), 11).unwrap();
    assert_eq!(report.ingested, 1);
    let loaded = store
        .get(&ObservationIdentity::new(
            Harness::OhMyPi,
            SessionId::parse("omp-hist").unwrap(),
        ))
        .unwrap()
        .unwrap();
    assert_eq!(loaded.counts().input_tokens(), 80);
    assert_eq!(loaded.counts().output_tokens(), 8);
}

#[test]
fn first_use_syncs_only_harnesses_that_have_never_been_scanned() {
    let (_dir, store) = open_store();
    sync_harness(&store, Harness::Grok, &roots(), 1).unwrap();
    let reports = sync_all_needed(&store, &roots(), 2).unwrap();
    assert!(
        reports.iter().all(|r| r.harness != Harness::Grok),
        "grok already synced; first-use must not scan it again"
    );
    assert!(reports.iter().any(|r| r.harness == Harness::Pi));
    assert_eq!(store.harness_last_synced(Harness::Grok).unwrap(), Some(1));
    assert_eq!(store.harness_last_synced(Harness::Pi).unwrap(), Some(2));
}
