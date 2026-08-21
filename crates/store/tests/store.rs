//! Drive the shipped FileStore: ingest, read-back, merge, distinct harnesses.

use tempfile::tempdir;
use toktally_domain::{
    ExtraCounts, Harness, ObservationIdentity, ObservationSource, SessionId,
    SessionStoreCompleteness, UsageCounts, UsageObservation,
};
use toktally_store::FileStore;

fn identity(harness: Harness, session: &str) -> ObservationIdentity {
    ObservationIdentity::new(harness, SessionId::parse(session).unwrap())
}

fn observation(
    harness: Harness,
    session: &str,
    input: u64,
    output: u64,
    source: ObservationSource,
    completeness: SessionStoreCompleteness,
) -> UsageObservation {
    UsageObservation::new(
        identity(harness, session),
        UsageCounts::new(input, output).with_extras(ExtraCounts {
            cache_read: Some(7),
            cache_write: None,
            reasoning: None,
            ..ExtraCounts::default()
        }),
        source,
        completeness,
    )
}

fn open_store() -> (tempfile::TempDir, FileStore) {
    let dir = tempdir().unwrap();
    let store = FileStore::open(dir.path().join("store.json")).unwrap();
    (dir, store)
}

#[test]
fn ingest_then_read_returns_submitted_input_and_output_counts() {
    let (_dir, store) = open_store();
    let submitted = observation(
        Harness::ClaudeCode,
        "cc-sess-1",
        12345,
        678,
        ObservationSource::PluginReport,
        SessionStoreCompleteness::Complete,
    );
    store.ingest(submitted.clone()).unwrap();

    let loaded = store.get(submitted.identity()).unwrap().expect("stored");
    assert_eq!(loaded.counts().input_tokens(), 12345);
    assert_eq!(loaded.counts().output_tokens(), 678);
    assert_eq!(loaded.counts().extras().cache_read, Some(7));
}

#[test]
fn second_report_for_same_identity_updates_totals_instead_of_duplicating() {
    let (_dir, store) = open_store();
    let first = observation(
        Harness::Codex,
        "thr_same",
        100,
        20,
        ObservationSource::PluginReport,
        SessionStoreCompleteness::Complete,
    );
    let second = observation(
        Harness::Codex,
        "thr_same",
        250,
        40,
        ObservationSource::PluginReport,
        SessionStoreCompleteness::Complete,
    );
    store.ingest(first).unwrap();
    store.ingest(second.clone()).unwrap();

    let listed = store.list().unwrap();
    let matching: Vec<_> = listed
        .iter()
        .filter(|row| row.identity() == second.identity())
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "same identity must not produce a second independent total"
    );
    assert_eq!(matching[0].counts().input_tokens(), 250);
    assert_eq!(matching[0].counts().output_tokens(), 40);
}

#[test]
fn different_harnesses_stay_distinct_and_are_queryable_together() {
    let (_dir, store) = open_store();
    store
        .ingest(observation(
            Harness::Grok,
            "shared-id",
            10,
            1,
            ObservationSource::PluginReport,
            SessionStoreCompleteness::Unknown,
        ))
        .unwrap();
    store
        .ingest(observation(
            Harness::Jcode,
            "shared-id",
            99,
            8,
            ObservationSource::PluginReport,
            SessionStoreCompleteness::Complete,
        ))
        .unwrap();

    let listed = store.list().unwrap();
    assert_eq!(listed.len(), 2);
    let grok = store
        .get(&identity(Harness::Grok, "shared-id"))
        .unwrap()
        .unwrap();
    let jcode = store
        .get(&identity(Harness::Jcode, "shared-id"))
        .unwrap()
        .unwrap();
    assert_eq!(grok.counts().input_tokens(), 10);
    assert_eq!(jcode.counts().input_tokens(), 99);
    assert_ne!(grok.identity().harness(), jcode.identity().harness());
}

#[test]
fn source_and_completeness_are_preserved() {
    let (_dir, store) = open_store();
    let global = observation(
        Harness::OhMyPi,
        SessionId::HARNESS_GLOBAL,
        250_000,
        40_000,
        ObservationSource::HarnessGlobalApproximation,
        SessionStoreCompleteness::Partial,
    );
    store.ingest(global.clone()).unwrap();
    let loaded = store.get(global.identity()).unwrap().unwrap();
    assert_eq!(
        loaded.source(),
        ObservationSource::HarnessGlobalApproximation
    );
    assert_eq!(loaded.completeness(), SessionStoreCompleteness::Partial);

    let grok = observation(
        Harness::Grok,
        "frag-1",
        212362,
        0,
        ObservationSource::PluginReport,
        SessionStoreCompleteness::Unknown,
    );
    store.ingest(grok.clone()).unwrap();
    let loaded_grok = store.get(grok.identity()).unwrap().unwrap();
    assert_eq!(loaded_grok.source(), ObservationSource::PluginReport);
    assert_eq!(
        loaded_grok.completeness(),
        SessionStoreCompleteness::Unknown
    );
}

#[test]
fn reopen_reads_the_same_totals() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("store.json");
    let first = FileStore::open(&path).unwrap();
    first
        .ingest(observation(
            Harness::ClaudeCode,
            "persist-me",
            42,
            6,
            ObservationSource::PluginReport,
            SessionStoreCompleteness::Complete,
        ))
        .unwrap();
    drop(first);

    let reopened = FileStore::open(&path).unwrap();
    let loaded = reopened
        .get(&identity(Harness::ClaudeCode, "persist-me"))
        .unwrap()
        .unwrap();
    assert_eq!(loaded.counts().input_tokens(), 42);
    assert_eq!(loaded.counts().output_tokens(), 6);
}

#[test]
fn ingest_stamps_last_synced_at_and_later_ingest_updates_it() {
    let (_dir, store) = open_store();
    let first = store
        .ingest_at(
            observation(
                Harness::Hermes,
                "h1",
                10,
                1,
                ObservationSource::PluginReport,
                SessionStoreCompleteness::Complete,
            ),
            1_700_000_000,
        )
        .unwrap();
    assert_eq!(first.last_synced_at(), Some(1_700_000_000));
    let second = store
        .ingest_at(
            observation(
                Harness::Hermes,
                "h1",
                20,
                2,
                ObservationSource::PluginReport,
                SessionStoreCompleteness::Complete,
            ),
            1_700_000_500,
        )
        .unwrap();
    assert_eq!(second.last_synced_at(), Some(1_700_000_500));
    let loaded = store
        .get(&identity(Harness::Hermes, "h1"))
        .unwrap()
        .unwrap();
    assert_eq!(loaded.last_synced_at(), Some(1_700_000_500));
    assert_eq!(loaded.counts().input_tokens(), 20);
}

#[test]
fn harness_first_sync_is_recorded_once_until_updated() {
    let (_dir, store) = open_store();
    assert!(store.needs_first_sync(Harness::Grok).unwrap());
    store
        .record_harness_sync(Harness::Grok, 1_700_000_111)
        .unwrap();
    assert!(!store.needs_first_sync(Harness::Grok).unwrap());
    assert_eq!(
        store.harness_last_synced(Harness::Grok).unwrap(),
        Some(1_700_000_111)
    );
    assert!(store.needs_first_sync(Harness::Codex).unwrap());
    let listed = store.list_harness_syncs().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].harness, Harness::Grok);
}

/// Path of the advisory lock sidecar guarding cross-process access.
fn dir_lock_path(store: &FileStore) -> std::path::PathBuf {
    store.path().with_extension("json.lock")
}

#[test]
fn ingest_blocks_while_another_process_holds_the_store_lock() {
    let (_dir, store) = open_store();
    let guard = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(dir_lock_path(&store))
        .unwrap();
    fs4::fs_std::FileExt::lock_exclusive(&guard).unwrap();

    let handle = std::thread::spawn(move || {
        store
            .ingest(observation(
                Harness::ClaudeCode,
                "locked-1",
                5,
                6,
                ObservationSource::PluginReport,
                SessionStoreCompleteness::Complete,
            ))
            .unwrap();
    });
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(
        !handle.is_finished(),
        "ingest must block while the lock file is held by another writer"
    );
    fs4::fs_std::FileExt::unlock(&guard).unwrap();
    handle.join().unwrap();
}

#[test]
fn parallel_store_instances_on_one_path_do_not_lose_updates() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("store.json");
    let handles: Vec<_> = (0..8)
        .map(|worker| {
            let path = path.clone();
            std::thread::spawn(move || {
                let store = FileStore::open(path).unwrap();
                for i in 0..25 {
                    store
                        .ingest(observation(
                            Harness::ClaudeCode,
                            &format!("w{worker}-s{i}"),
                            1,
                            1,
                            ObservationSource::PluginReport,
                            SessionStoreCompleteness::Complete,
                        ))
                        .unwrap();
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    let store = FileStore::open(path).unwrap();
    assert_eq!(
        store.list().unwrap().len(),
        200,
        "no update may be lost across instances"
    );
}

#[test]
fn reads_work_on_a_brand_new_store_without_a_sidecar() {
    let dir = tempdir().unwrap();
    let store = FileStore::open(dir.path().join("fresh.json")).unwrap();
    assert!(
        !dir.path().join("fresh.json.lock").exists(),
        "sidecar must not be created eagerly"
    );
    assert_eq!(store.list().unwrap().len(), 0);
    assert!(store.needs_first_sync(Harness::Pi).unwrap());
}

#[test]
fn reingest_without_metadata_preserves_stored_model_and_recorded_at() {
    let (_dir, store) = open_store();
    let rich = observation(
        Harness::Pi,
        "meta-sess",
        100,
        20,
        ObservationSource::PluginReport,
        SessionStoreCompleteness::Complete,
    )
    .with_recorded_at(1_700_000_000)
    .with_model("pi/gpt-9");
    store.ingest(rich).unwrap();

    let bare = observation(
        Harness::Pi,
        "meta-sess",
        150,
        30,
        ObservationSource::PluginReport,
        SessionStoreCompleteness::Complete,
    );
    store.ingest(bare).unwrap();

    let loaded = store
        .get(&identity(Harness::Pi, "meta-sess"))
        .unwrap()
        .expect("stored");
    assert_eq!(
        loaded.counts().input_tokens(),
        150,
        "totals still take the newest value"
    );
    assert_eq!(loaded.model(), Some("pi/gpt-9"));
    assert_eq!(loaded.recorded_at(), Some(1_700_000_000));
}
