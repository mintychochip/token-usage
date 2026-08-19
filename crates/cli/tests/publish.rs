//! Publish local usage to a directory or a GitHub gist; pull it back.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::tempdir;

fn reporter() -> Command {
    Command::new(env!("CARGO_BIN_EXE_token-usage-reporter"))
}

fn ingest_hermes(store: &Path, home: &Path) {
    let fixture = format!(
        "{}/../adapters/fixtures/hermes-session.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let out = reporter()
        .arg("--store")
        .arg(store)
        .arg("--home")
        .arg(home)
        .args(["ingest", "--adapter", "hermes", "--file", &fixture])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn write_fake_gh(dir: &Path) -> PathBuf {
    let script = dir.join("fake-gh");
    fs::write(
        &script,
        r#"#!/usr/bin/env python3
import json, os, shutil, sys
from pathlib import Path

state = Path(os.environ["FAKE_GH_STATE"])
(state / "args.log").parent.mkdir(parents=True, exist_ok=True)
with (state / "args.log").open("a") as f:
    f.write(" ".join(sys.argv[1:]) + "\n")

args = sys.argv[1:]
gists = state / "gists"
gists.mkdir(parents=True, exist_ok=True)

def copy_files(dest, files):
    dest.mkdir(parents=True, exist_ok=True)
    for src in files:
        shutil.copy(src, dest / Path(src).name)

def file_args(argv):
    files = []
    i = 0
    while i < len(argv):
        a = argv[i]
        if a == "-d":
            i += 2
            continue
        if a.startswith("-"):
            i += 1
            continue
        files.append(a)
        i += 1
    return files

if args[:2] == ["gist", "create"]:
    rest = args[2:]
    nid_path = state / "next_id"
    n = int(nid_path.read_text()) if nid_path.exists() else 1
    nid_path.write_text(str(n + 1))
    gist_id = f"gist{n}"
    dest = gists / gist_id
    copy_files(dest, file_args(rest))
    print(f"https://gist.github.com/mintychochip/{gist_id}")
    sys.exit(0)

if args[:2] == ["gist", "edit"]:
    gist_id = args[2]
    dest = gists / gist_id
    copy_files(dest, file_args(args[3:]))
    sys.exit(0)

if args[:1] == ["api"] and args[1] == "user":
    print(json.dumps({"login": "mintychochip"}))
    sys.exit(0)

if args[:1] == ["api"] and args[1].startswith("gists/"):
    gist_id = args[1].split("/", 1)[1]
    dest = gists / gist_id
    files = {}
    if dest.is_dir():
        for p in dest.iterdir():
            files[p.name] = {"content": p.read_text()}
    print(json.dumps({"files": files, "owner": {"login": "mintychochip"}}))
    sys.exit(0)

sys.stderr.write("unexpected fake-gh invocation: " + " ".join(args) + "\n")
sys.exit(2)
"#,
    )
    .unwrap();
    let mut perms = fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script, perms).unwrap();
    script
}

#[test]
fn publish_dir_writes_summary_badge_and_jsonl_then_pull_restores() {
    let dir = tempdir().unwrap();
    let store = dir.path().join("store.json");
    let home = dir.path().join("home");
    fs::create_dir_all(&home).unwrap();
    ingest_hermes(&store, &home);

    let bundle = dir.path().join("bundle");
    let publish = reporter()
        .arg("--store")
        .arg(&store)
        .arg("--home")
        .arg(&home)
        .args(["publish", "--dir"])
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        publish.status.success(),
        "{}",
        String::from_utf8_lossy(&publish.stderr)
    );

    let summary = fs::read_to_string(bundle.join("usage-summary.json")).unwrap();
    assert!(summary.contains("18000"), "{summary}");
    assert!(
        !summary.contains("\"session_id\""),
        "public summary must not leak session ids: {summary}"
    );
    let badge = fs::read_to_string(bundle.join("usage-badge.json")).unwrap();
    assert!(badge.contains("schemaVersion"), "{badge}");
    let jsonl = fs::read_to_string(bundle.join("usage.jsonl")).unwrap();
    assert!(jsonl.contains("hermes-sess-1"), "{jsonl}");

    let other = dir.path().join("other.json");
    let pull = reporter()
        .arg("--store")
        .arg(&other)
        .arg("--home")
        .arg(&home)
        .args(["pull", "--dir"])
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        pull.status.success(),
        "{}",
        String::from_utf8_lossy(&pull.stderr)
    );
    let get = reporter()
        .arg("--store")
        .arg(&other)
        .arg("--home")
        .arg(&home)
        .args(["get", "--harness", "hermes", "--session", "hermes-sess-1"])
        .output()
        .unwrap();
    assert!(get.status.success());
    let body = String::from_utf8_lossy(&get.stdout);
    assert!(body.contains("18000"), "{body}");
    assert!(body.contains("hermes-sess-1"), "{body}");
}

#[test]
fn public_publish_dir_omits_session_jsonl_and_cannot_be_pulled() {
    let dir = tempdir().unwrap();
    let store = dir.path().join("store.json");
    let home = dir.path().join("home");
    fs::create_dir_all(&home).unwrap();
    ingest_hermes(&store, &home);

    let bundle = dir.path().join("public");
    let publish = reporter()
        .arg("--store")
        .arg(&store)
        .arg("--home")
        .arg(&home)
        .args(["publish", "--public", "--dir"])
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        publish.status.success(),
        "{}",
        String::from_utf8_lossy(&publish.stderr)
    );
    assert!(bundle.join("usage-summary.json").is_file());
    assert!(bundle.join("usage-badge.json").is_file());
    assert!(
        !bundle.join("usage.jsonl").exists(),
        "public publish must not include session jsonl"
    );

    let other = dir.path().join("other.json");
    let pull = reporter()
        .arg("--store")
        .arg(&other)
        .arg("--home")
        .arg(&home)
        .args(["pull", "--dir"])
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(!pull.status.success());
    let err = String::from_utf8_lossy(&pull.stderr);
    assert!(
        err.contains("usage.jsonl") || err.contains("summary-only"),
        "{err}"
    );
}

#[test]
fn gist_publish_remembers_id_edits_on_second_push_and_pulls_back() {
    let dir = tempdir().unwrap();
    let store = dir.path().join("store.json");
    let home = dir.path().join("home");
    fs::create_dir_all(&home).unwrap();
    ingest_hermes(&store, &home);

    let gh_state = dir.path().join("gh");
    fs::create_dir_all(&gh_state).unwrap();
    let fake_gh = write_fake_gh(dir.path());

    let publish = reporter()
        .env("TOKEN_USAGE_GH", &fake_gh)
        .env("FAKE_GH_STATE", &gh_state)
        .arg("--store")
        .arg(&store)
        .arg("--home")
        .arg(&home)
        .args(["publish", "--gist"])
        .output()
        .unwrap();
    assert!(
        publish.status.success(),
        "{}",
        String::from_utf8_lossy(&publish.stderr)
    );
    let stdout = String::from_utf8_lossy(&publish.stdout);
    assert!(stdout.contains("gist1"), "{stdout}");
    let cfg = fs::read_to_string(dir.path().join("github.json")).unwrap();
    assert!(cfg.contains("gist1"), "{cfg}");
    assert!(
        gh_state.join("gists/gist1/usage.jsonl").is_file(),
        "secret gist should include sessions"
    );

    let again = reporter()
        .env("TOKEN_USAGE_GH", &fake_gh)
        .env("FAKE_GH_STATE", &gh_state)
        .arg("--store")
        .arg(&store)
        .arg("--home")
        .arg(&home)
        .args(["publish", "--gist"])
        .output()
        .unwrap();
    assert!(
        again.status.success(),
        "{}",
        String::from_utf8_lossy(&again.stderr)
    );
    let log = fs::read_to_string(gh_state.join("args.log")).unwrap();
    assert!(log.contains("gist create"), "{log}");
    assert!(log.contains("gist edit gist1"), "{log}");
    assert!(
        !log.contains("gist create") || log.matches("gist create").count() == 1,
        "second publish must edit, not create another gist: {log}"
    );

    let other = dir.path().join("other.json");
    let pull = reporter()
        .env("TOKEN_USAGE_GH", &fake_gh)
        .env("FAKE_GH_STATE", &gh_state)
        .arg("--store")
        .arg(&other)
        .arg("--home")
        .arg(&home)
        .args(["pull", "--gist"])
        .output()
        .unwrap();
    assert!(
        pull.status.success(),
        "{}",
        String::from_utf8_lossy(&pull.stderr)
    );
    let get = reporter()
        .arg("--store")
        .arg(&other)
        .arg("--home")
        .arg(&home)
        .args(["get", "--harness", "hermes", "--session", "hermes-sess-1"])
        .output()
        .unwrap();
    let body = String::from_utf8_lossy(&get.stdout);
    assert!(get.status.success(), "{body}");
    assert!(body.contains("18000"), "{body}");
}

#[test]
fn public_gist_omits_jsonl() {
    let dir = tempdir().unwrap();
    let store = dir.path().join("store.json");
    let home = dir.path().join("home");
    fs::create_dir_all(&home).unwrap();
    ingest_hermes(&store, &home);

    let gh_state = dir.path().join("gh");
    fs::create_dir_all(&gh_state).unwrap();
    let fake_gh = write_fake_gh(dir.path());

    let publish = reporter()
        .env("TOKEN_USAGE_GH", &fake_gh)
        .env("FAKE_GH_STATE", &gh_state)
        .arg("--store")
        .arg(&store)
        .arg("--home")
        .arg(&home)
        .args(["publish", "--gist", "--public"])
        .output()
        .unwrap();
    assert!(
        publish.status.success(),
        "{}",
        String::from_utf8_lossy(&publish.stderr)
    );
    assert!(gh_state.join("gists/gist1/usage-summary.json").is_file());
    assert!(
        !gh_state.join("gists/gist1/usage.jsonl").exists(),
        "public gist must not include session jsonl"
    );
}

#[test]
fn gist_publish_snippets_use_raw_githubusercontent_url_and_inline_js() {
    let dir = tempdir().unwrap();
    let store = dir.path().join("store.json");
    let home = dir.path().join("home");
    fs::create_dir_all(&home).unwrap();
    ingest_hermes(&store, &home);

    let gh_state = dir.path().join("gh");
    fs::create_dir_all(&gh_state).unwrap();
    let fake_gh = write_fake_gh(dir.path());

    let publish = reporter()
        .env("TOKEN_USAGE_GH", &fake_gh)
        .env("FAKE_GH_STATE", &gh_state)
        .arg("--store")
        .arg(&store)
        .arg("--home")
        .arg(&home)
        .args(["publish", "--gist", "--public"])
        .output()
        .unwrap();
    assert!(
        publish.status.success(),
        "{}",
        String::from_utf8_lossy(&publish.stderr)
    );
    let stdout = String::from_utf8_lossy(&publish.stdout);
    assert!(
        stdout.contains("gist.githubusercontent.com/mintychochip/gist1/raw"),
        "badge must use gist raw host+owner+id: {stdout}"
    );
    assert!(
        !stdout.contains("gist.github.com/gist1/raw"),
        "gist.github.com/{{id}}/raw 404s: {stdout}"
    );
    assert!(
        stdout.contains("usage-badge.json") && stdout.contains("usage-summary.json"),
        "snippets must name the published files: {stdout}"
    );
    assert!(
        !stdout.contains("usage-card.js"),
        "website paste must not depend on a gist-hosted .js file: {stdout}"
    );
    assert!(
        stdout.contains("<script>") && !stdout.contains("<script src"),
        "website paste must inline the card script: {stdout}"
    );
    assert!(
        !gh_state.join("gists/gist1/usage-card.js").exists(),
        "do not upload usage-card.js to the gist"
    );
}

#[test]
fn publish_dir_prints_github_and_website_snippets_twice() {
    let dir = tempdir().unwrap();
    let store = dir.path().join("store.json");
    let home = dir.path().join("home");
    fs::create_dir_all(&home).unwrap();
    ingest_hermes(&store, &home);
    let bundle = dir.path().join("bundle");
    let base = "https://example.com/usage";
    for _ in 0..2 {
        let publish = reporter()
            .arg("--store")
            .arg(&store)
            .arg("--home")
            .arg(&home)
            .args(["publish", "--dir"])
            .arg(&bundle)
            .args(["--url", base])
            .output()
            .unwrap();
        assert!(
            publish.status.success(),
            "{}",
            String::from_utf8_lossy(&publish.stderr)
        );
        let stdout = String::from_utf8_lossy(&publish.stdout);
        let combined = format!(
            "{stdout}\n{}",
            fs::read_to_string(bundle.join("snippets.md")).unwrap_or_default()
        );
        assert!(
            combined.contains("img.shields.io"),
            "must emit GitHub shields markdown: {combined}"
        );
        assert!(
            combined.contains("usage-badge.json"),
            "badge snippet must reference usage-badge.json: {combined}"
        );
        assert!(
            combined.contains("usage-summary.json"),
            "website snippet must reference usage-summary.json: {combined}"
        );
        assert!(
            !combined.contains("session_id"),
            "snippets must not leak session ids: {combined}"
        );
        assert!(
            bundle.join("usage-card.js").is_file(),
            "published dir must include the website script"
        );
    }
}
