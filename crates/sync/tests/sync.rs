//! Drive discovery and first-use sync against fixture harness stores.

use std::path::PathBuf;
use std::fs;

use toktally_adapters::adapt;
use toktally_sync::SyncError;


use tempfile::tempdir;
use toktally_domain::{Harness, ObservationIdentity, SessionId};
use toktally_store::FileStore;
use toktally_sync::{discover, sync_all_needed, sync_harness, SyncRoots};

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
fn every_harness_discovers_at_least_one_fixture_payload() {
    for harness in Harness::all() {
        let payloads = discover(harness, &roots()).unwrap();
        assert!(
            !payloads.is_empty(),
            "{harness:?} should have at least one discoverable fixture payload"
        );
    }
}

#[test]
fn aggregate_jsonl_preserves_cache_write_and_reasoning_extras() {
    let dir = tempdir().unwrap();
    let path = dir.path().join(".omp/agent/sessions/session.jsonl");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        concat!(
            "{\"type\":\"session\",\"id\":\"extras\",\"timestamp\":\"2026-01-01T00:00:00Z\"}\n",
            "{\"usage\":{\"input\":1,\"output\":2,\"cache_write\":3,\"reasoning\":4}}\n"
        ),
    )
    .unwrap();
    let payloads = discover(
        Harness::OhMyPi,
        &SyncRoots {
            home: dir.path().to_path_buf(),
        },
    )
    .unwrap();
    assert_eq!(payloads.len(), 1);
    let observation = adapt(Harness::OhMyPi, &payloads[0]).unwrap();
    assert_eq!(observation.counts().extras().cache_write, Some(3));
    assert_eq!(observation.counts().extras().reasoning, Some(4));
}

#[test]
fn unreadable_session_directory_surfaces_io_error() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let unreadable = dir.path().join(".jcode/unreadable");
        fs::create_dir_all(&unreadable).unwrap();
        let mut permissions = fs::metadata(&unreadable).unwrap().permissions();
        permissions.set_mode(0o000);
        fs::set_permissions(&unreadable, permissions).unwrap();
        let probe = fs::read_dir(&unreadable);
        if probe.is_ok() {
            return;
        }
        let result = discover(
            Harness::Jcode,
            &SyncRoots {
                home: dir.path().to_path_buf(),
            },
        );
        assert!(matches!(result, Err(SyncError::Io(_))), "{result:?}");
    }
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
fn claude_usage_json_is_ingested_as_a_global_approximation() {
    let dir = tempdir().unwrap();
    let home = dir.path().join("home");
    std::fs::create_dir_all(home.join(".claude")).unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../adapters/fixtures/claude-code-global.json");
    let expected: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&fixture).unwrap()).unwrap();
    std::fs::copy(&fixture, home.join(".claude/usage.json")).unwrap();

    let store = FileStore::open(dir.path().join("store.json")).unwrap();
    let report = sync_harness(
        &store,
        Harness::ClaudeCode,
        &SyncRoots { home },
        1_700_000_100,
    )
    .unwrap();
    assert!(
        report.ingested >= 1,
        "global usage.json must be ingested, got {report:?}"
    );

    let loaded = store
        .get(&ObservationIdentity::new(
            Harness::ClaudeCode,
            SessionId::harness_global(),
        ))
        .unwrap()
        .expect("claude-code __harness_global__");
    assert_eq!(
        loaded.counts().input_tokens(),
        expected["input_tokens"].as_u64().unwrap()
    );
    assert_eq!(
        loaded.counts().output_tokens(),
        expected["output_tokens"].as_u64().unwrap()
    );
    assert_eq!(
        loaded.source(),
        toktally_domain::ObservationSource::HarnessGlobalApproximation
    );
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

#[test]
fn sync_harness_leaves_needs_first_sync_true_when_every_payload_fails_adaptation() {
    let home = tempdir().unwrap();
    let sess = home.path().join(".grok/sessions/proj/sess-poison");
    std::fs::create_dir_all(&sess).unwrap();
    std::fs::write(sess.join("signals.json"), r#"{"boom": true}"#).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let store = FileStore::open(dir.path().join("store.json")).unwrap();
    let roots = SyncRoots {
        home: home.path().to_path_buf(),
    };

    let report = sync_harness(&store, Harness::Grok, &roots, 42).unwrap();
    assert_eq!(report.ingested, 0);
    assert_eq!(report.skipped, 1);
    assert!(
        store.needs_first_sync(Harness::Grok).unwrap(),
        "failed scans must not permanently blackhole the harness"
    );
}
