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

    let ingest = reporter()
        .arg("--store")
        .arg(&store)
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

    let mut child = reporter()
        .arg("--store")
        .arg(&store)
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
