//! Publish to GitHub Pages using a fake `gh` and `git`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn ingest_fixture(store: &Path, home: &Path) {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../adapters/fixtures/oh-my-pi-session.json");
    fs::create_dir_all(home).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_token-usage-reporter"))
        .env("TOKEN_USAGE_STORE", store)
        .env("TOKEN_USAGE_HARNESS_HOME", home)
        .args(["ingest", "--adapter", "oh-my-pi", "--file"])
        .arg(&fixture)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

fn write_fake_gh(dir: &Path, state: &Path) -> PathBuf {
    let script = dir.join("fake-gh");
    let py = format!(
        r#"#!/usr/bin/env python3
import os, shutil, sys
from pathlib import Path

state = Path("{state}")
state.mkdir(parents=True, exist_ok=True)

args = sys.argv[1:]

if args[:2] == ["repo", "view"]:
    repo = args[2]
    repo_dir = state / repo
    if repo_dir.is_dir():
        print(f"viewed {{repo}}")
        sys.exit(0)
    print(f"not found: {{repo}}", file=sys.stderr)
    sys.exit(1)

if args[:2] == ["repo", "create"]:
    repo = args[2]
    src = Path.cwd()
    dest = state / repo
    dest.mkdir(parents=True, exist_ok=True)
    for p in src.iterdir():
        if p.name == ".git":
            continue
        if p.is_dir():
            shutil.copytree(p, dest / p.name, dirs_exist_ok=True)
        else:
            shutil.copy2(p, dest / p.name)
    print(f"created {{repo}}")
    sys.exit(0)

if args[:2] == ["repo", "clone"]:
    repo = args[2]
    dest = Path(args[3])
    src = state / repo
    if not src.is_dir():
        print(f"source repo does not exist: {{repo}}", file=sys.stderr)
        sys.exit(1)
    shutil.copytree(src, dest, dirs_exist_ok=True)
    print(f"cloned {{repo}}")
    sys.exit(0)

print("unexpected fake-gh invocation: " + " ".join(args), file=sys.stderr)
sys.exit(2)
"#,
        state = state.display()
    );
    fs::write(&script, py).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script, perms).unwrap();
    }
    script
}

fn write_fake_git(dir: &Path) -> PathBuf {
    let script = dir.join("fake-git");
    let py = r#"#!/usr/bin/env python3
import sys
from pathlib import Path

args = sys.argv[1:]

cwd = Path.cwd()
i = 0
while i < len(args):
    if args[i] == "-C":
        cwd = Path(args[i + 1])
        i += 2
        continue
    if args[i].startswith("-c"):
        i += 2
        continue
    break

sub = args[i] if i < len(args) else ""

if sub == "init":
    (cwd / ".git").mkdir(parents=True, exist_ok=True)
    print("Initialized empty Git repository")
    sys.exit(0)

if sub == "add":
    sys.exit(0)

if sub == "commit":
    log = cwd / ".git" / "commit-log"
    try:
        idx = args.index("-m") + 1
    except ValueError:
        idx = -1
    msg = args[idx] if 0 <= idx < len(args) else "commit"
    log.write_text(msg + "\n")
    print(f"[{msg}]")
    sys.exit(0)

if sub == "push":
    sys.exit(0)

print("unexpected fake-git invocation: " + " ".join(args), file=sys.stderr)
sys.exit(2)
"#;
    fs::write(&script, py).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script, perms).unwrap();
    }
    script
}

#[test]
fn github_pages_publishes_bundle_on_first_run() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("store.json");
    let home = dir.path().join("home");
    let key_dir = dir.path().join("keys");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&key_dir).unwrap();
    ingest_fixture(&store, &home);

    let gh_state = dir.path().join("gh");
    fs::create_dir_all(&gh_state).unwrap();
    let fake_gh = write_fake_gh(dir.path(), &gh_state);
    let fake_git = write_fake_git(dir.path());

    let mut paths = vec![fake_git.parent().unwrap().to_path_buf()];
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path));
    }
    let path = std::env::join_paths(paths).unwrap();

    let config = dir.path().join("publish-config.json");
    fs::write(
        &config,
        r#"{"widgets":{"enabled":false,"url":""},"github_pages":{"enabled":true,"repo":"mintychochip/token-usage-pages"}}"#,
    )
    .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_token-usage-reporter"))
        .env("PATH", &path)
        .env("TOKEN_USAGE_GH", &fake_gh)
        .env("TOKEN_USAGE_STORE", &store)
        .env("TOKEN_USAGE_HARNESS_HOME", &home)
        .env("TOKTALLY_IDENTITY_DIR", &key_dir)
        .args(["publish", "--github-pages"])
        .output()
        .unwrap();

    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("https://mintychochip.github.io/token-usage-pages"),
        "stdout: {stdout}"
    );

    let repo = gh_state.join("mintychochip/token-usage-pages");
    assert!(repo.join("index.html").is_file());
    assert!(repo.join("usage-summary.json").is_file());
    assert!(repo.join("usage-badge.json").is_file());
    assert!(repo.join("token-usage-card.js").is_file());
    let summary = fs::read_to_string(repo.join("usage-summary.json")).unwrap();
    assert!(summary.contains("oh-my-pi"));
}
