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

#[test]
fn sync_interval_rereads_a_changed_global_usage_file() {
    let dir = tempdir().unwrap();
    let store = dir.path().join("store.json");
    let home = dir.path().join("home");
    std::fs::create_dir_all(home.join(".claude")).unwrap();
    let usage = home.join(".claude/usage.json");
    std::fs::write(
        &usage,
        r#"{"kind":"global_usage","input_tokens":100,"output_tokens":10}"#,
    )
    .unwrap();

    let mut child = reporter()
        .arg("--store")
        .arg(&store)
        .arg("--home")
        .arg(&home)
        .args([
            "sync",
            "--harness",
            "claude-code",
            "--force",
            "--interval",
            "1",
        ])
        .spawn()
        .unwrap();

    let first = wait_for_input(&store, &home, 100, 8);
    assert!(first, "first tick must ingest 100 from usage.json");

    std::fs::write(
        &usage,
        r#"{"kind":"global_usage","input_tokens":250,"output_tokens":20}"#,
    )
    .unwrap();
    let second = wait_for_input(&store, &home, 250, 8);
    let _ = child.kill();
    let _ = child.wait();
    assert!(
        second,
        "later tick must ingest the updated 250 from usage.json"
    );
}

fn wait_for_input(store: &std::path::Path, home: &std::path::Path, input: u64, secs: u64) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    while std::time::Instant::now() < deadline {
        let get = reporter()
            .arg("--store")
            .arg(store)
            .arg("--home")
            .arg(home)
            .args([
                "get",
                "--harness",
                "claude-code",
                "--session",
                "__harness_global__",
            ])
            .output()
            .unwrap();
        if get.status.success() {
            let body = String::from_utf8_lossy(&get.stdout);
            if body.contains(&input.to_string()) {
                return true;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    false
}

#[test]
fn export_summary_is_chartable_and_has_no_session_ids() {
    let dir = tempdir().unwrap();
    let store = dir.path().join("store.json");
    let empty_home = dir.path().join("home");
    std::fs::create_dir_all(&empty_home).unwrap();
    let fixture = format!(
        "{}/../adapters/fixtures/hermes-session.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let expected: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&fixture).unwrap()).unwrap();
    let expected_input = expected["usage"]["input_tokens"].as_u64().unwrap();
    reporter()
        .arg("--store")
        .arg(&store)
        .arg("--home")
        .arg(&empty_home)
        .args(["ingest", "--adapter", "hermes", "--file", &fixture])
        .output()
        .unwrap();

    let summary_path = dir.path().join("usage-summary.json");
    let export = reporter()
        .env("TOKEN_USAGE_PRICES_FETCH", "0")
        .arg("--store")
        .arg(&store)
        .arg("--home")
        .arg(&empty_home)
        .args(["export", "--format", "summary", "--file"])
        .arg(&summary_path)
        .output()
        .unwrap();
    assert!(
        export.status.success(),
        "{}",
        String::from_utf8_lossy(&export.stderr)
    );
    let summary: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&summary_path).unwrap()).unwrap();
    assert_eq!(summary["input_tokens"].as_u64().unwrap(), expected_input);
    assert!(summary.get("session_id").is_none());
    assert!(summary["harnesses"].as_array().unwrap().iter().any(|h| {
        h["harness"] == "hermes" && h["input_tokens"].as_u64() == Some(expected_input)
    }));
}

#[test]
fn export_summary_estimates_cost_from_internal_price_file() {
    let dir = tempdir().unwrap();
    let store = dir.path().join("store.json");
    let empty_home = dir.path().join("home");
    std::fs::create_dir_all(&empty_home).unwrap();
    let fixture = format!(
        "{}/../adapters/fixtures/hermes-session.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let payload: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&fixture).unwrap()).unwrap();
    let expected_input = payload["usage"]["input_tokens"].as_u64().unwrap();
    let expected_output = payload["usage"]["output_tokens"].as_u64().unwrap();
    reporter()
        .arg("--store")
        .arg(&store)
        .arg("--home")
        .arg(&empty_home)
        .args(["ingest", "--adapter", "hermes", "--file", &fixture])
        .output()
        .unwrap();

    let prices = dir.path().join("prices.json");
    std::fs::write(
        &prices,
        r#"{"data":[{"id":"anthropic/claude-sonnet-4.6","pricing":{"prompt":"0.000003","completion":"0.000015"}}]}"#,
    )
    .unwrap();
    let summary_path = dir.path().join("usage-summary.json");
    let export = reporter()
        .env("TOKEN_USAGE_PRICES", &prices)
        .env("TOKEN_USAGE_PRICES_FETCH", "0")
        .arg("--store")
        .arg(&store)
        .arg("--home")
        .arg(&empty_home)
        .args(["export", "--format", "summary", "--file"])
        .arg(&summary_path)
        .output()
        .unwrap();
    assert!(
        export.status.success(),
        "{}",
        String::from_utf8_lossy(&export.stderr)
    );
    let summary: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&summary_path).unwrap()).unwrap();
    let cost = summary["estimated_cost_usd"].as_f64().expect("cost");
    let expected = expected_input as f64 * 0.000003 + expected_output as f64 * 0.000015;
    assert!(
        (cost - expected).abs() < 1e-12,
        "cost {cost} != {expected} from fixture + internal table"
    );
}
