//! Drive the shipped token-usage-reporter binary on fixture payloads.

use std::io::Write;
use std::process::{Command, Stdio};

use tempfile::tempdir;

fn reporter() -> Command {
    Command::new(env!("CARGO_BIN_EXE_token-usage-reporter"))
}

#[test]
fn reporter_ingest_then_get_returns_fixture_counts() {
    let dir = tempdir().unwrap();
    let store = dir.path().join("store.json");
    let fixture = format!(
        "{}/../adapters/fixtures/claude-code-session.json",
        env!("CARGO_MANIFEST_DIR")
    );

    let empty_home = dir.path().join("home");
    std::fs::create_dir_all(&empty_home).unwrap();
    let ingest = reporter()
        .arg("--store")
        .arg(&store)
        .arg("--home")
        .arg(&empty_home)
        .args(["ingest", "--adapter", "claude-code", "--file", &fixture])
        .output()
        .unwrap();
    assert!(
        ingest.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&ingest.stderr)
    );
    let posted = String::from_utf8_lossy(&ingest.stdout);
    assert!(posted.contains("12345"), "{posted}");
    assert!(posted.contains("678"), "{posted}");
    assert!(posted.contains("cc-sess-1"), "{posted}");

    let get = reporter()
        .arg("--store")
        .arg(&store)
        .arg("--home")
        .arg(&empty_home)
        .args(["get", "--harness", "claude-code", "--session", "cc-sess-1"])
        .output()
        .unwrap();
    assert!(get.status.success());
    let body = String::from_utf8_lossy(&get.stdout);
    assert!(body.contains("12345"), "{body}");
    assert!(body.contains("678"), "{body}");
    assert!(body.contains("plugin_report"), "{body}");
}

#[test]
fn reporter_reads_stdin_for_plugin_hooks() {
    let dir = tempdir().unwrap();
    let store = dir.path().join("store.json");
    let fixture = std::fs::read_to_string(format!(
        "{}/../adapters/fixtures/jcode-session.json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();

    let empty_home = dir.path().join("home");
    std::fs::create_dir_all(&empty_home).unwrap();
    let mut child = reporter()
        .arg("--store")
        .arg(&store)
        .arg("--home")
        .arg(&empty_home)
        .args(["ingest", "--adapter", "jcode"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(fixture.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let body = String::from_utf8_lossy(&out.stdout);
    assert!(body.contains("4200"), "{body}");
    assert!(body.contains("900"), "{body}");
}

#[test]
fn first_ingest_syncs_every_historical_grok_session_then_the_hook() {
    let dir = tempdir().unwrap();
    let store = dir.path().join("store.json");
    let home = format!("{}/../sync/fixtures/home", env!("CARGO_MANIFEST_DIR"));
    let fixture = format!(
        "{}/../adapters/fixtures/grok-partial.json",
        env!("CARGO_MANIFEST_DIR")
    );

    let ingest = reporter()
        .arg("--store")
        .arg(&store)
        .arg("--home")
        .arg(&home)
        .args(["ingest", "--adapter", "grok", "--file", &fixture])
        .output()
        .unwrap();
    assert!(
        ingest.status.success(),
        "{}",
        String::from_utf8_lossy(&ingest.stderr)
    );
    let posted = String::from_utf8_lossy(&ingest.stdout);
    assert!(posted.contains("last_synced_at"), "{posted}");

    let list = reporter()
        .arg("--store")
        .arg(&store)
        .arg("--home")
        .arg(&home)
        .arg("list")
        .output()
        .unwrap();
    assert!(list.status.success());
    let body = String::from_utf8_lossy(&list.stdout);
    assert!(body.contains("sess-alpha"), "{body}");
    assert!(body.contains("sess-beta"), "{body}");
    assert!(
        body.contains("019f8886-1253-75f1-98e3-8ab6896f3296"),
        "{body}"
    );
    assert!(body.contains("1111"), "{body}");
    assert!(body.contains("2222"), "{body}");

    let status = reporter()
        .arg("--store")
        .arg(&store)
        .arg("--home")
        .arg(&home)
        .args(["sync", "--harness", "grok"])
        .output()
        .unwrap();
    assert!(status.status.success());
    let sync_body = String::from_utf8_lossy(&status.stdout);
    assert!(sync_body.contains("grok"), "{sync_body}");
    assert!(sync_body.contains("last_synced_at"), "{sync_body}");
}

#[test]
fn export_jsonl_round_trips_through_import_into_a_new_local_store() {
    let dir = tempdir().unwrap();
    let store = dir.path().join("store.json");
    let empty_home = dir.path().join("home");
    std::fs::create_dir_all(&empty_home).unwrap();
    let fixture = format!(
        "{}/../adapters/fixtures/hermes-session.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let ingest = reporter()
        .arg("--store")
        .arg(&store)
        .arg("--home")
        .arg(&empty_home)
        .args(["ingest", "--adapter", "hermes", "--file", &fixture])
        .output()
        .unwrap();
    assert!(
        ingest.status.success(),
        "{}",
        String::from_utf8_lossy(&ingest.stderr)
    );
    let expected: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&fixture).unwrap()).unwrap();
    let expected_input = expected["usage"]["input_tokens"].as_u64().unwrap();

    let jsonl = dir.path().join("usage.jsonl");
    let export = reporter()
        .arg("--store")
        .arg(&store)
        .arg("--home")
        .arg(&empty_home)
        .args(["export", "--file"])
        .arg(&jsonl)
        .output()
        .unwrap();
    assert!(
        export.status.success(),
        "{}",
        String::from_utf8_lossy(&export.stderr)
    );
    let dumped = std::fs::read_to_string(&jsonl).unwrap();
    assert!(dumped.contains("hermes-sess-1"), "{dumped}");

    let other = dir.path().join("other.json");
    let import = reporter()
        .arg("--store")
        .arg(&other)
        .arg("--home")
        .arg(&empty_home)
        .args(["import", "--file"])
        .arg(&jsonl)
        .output()
        .unwrap();
    assert!(
        import.status.success(),
        "{}",
        String::from_utf8_lossy(&import.stderr)
    );
    let get = reporter()
        .arg("--store")
        .arg(&other)
        .arg("--home")
        .arg(&empty_home)
        .args(["get", "--harness", "hermes", "--session", "hermes-sess-1"])
        .output()
        .unwrap();
    assert!(get.status.success());
    let body = String::from_utf8_lossy(&get.stdout);
    assert!(body.contains(&expected_input.to_string()), "{body}");
    assert!(body.contains("hermes-sess-1"), "{body}");
}
