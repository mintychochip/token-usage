//! Drive the shipped install.sh and update.sh scripts.
#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::tempdir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn bin_dir() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_toktally"))
        .parent()
        .expect("bin dir")
        .to_path_buf()
}

fn run_script(script: &str, prefix: &Path, extra_env: &[(&str, &Path)]) -> std::process::Output {
    let mut cmd = Command::new(repo_root().join("scripts").join(script));
    cmd.env("PREFIX", prefix)
        .env("TOKTALLY_SKIP_BUILD", "1")
        .env("TOKTALLY_SKIP_PULL", "1")
        .env("TOKTALLY_BIN_DIR", bin_dir())
        .env("TOKTALLY_SRC", repo_root());
    for (key, value) in extra_env {
        cmd.env(key, value);
    }
    cmd.output().unwrap_or_else(|e| panic!("run {script}: {e}"))
}

#[test]
fn install_help_exits_zero() {
    let out = Command::new(repo_root().join("scripts/install.sh"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("--prefix"), "{stdout}");
}

#[test]
fn install_puts_reporter_and_api_on_the_prefix_path() {
    let dir = tempdir().unwrap();
    let prefix = dir.path().join("prefix");
    let out = run_script("install.sh", &prefix, &[]);
    assert!(
        out.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );

    let reporter = prefix.join("bin/toktally");
    let api = prefix.join("bin/toktally-api");
    assert!(reporter.is_file(), "missing {}", reporter.display());
    assert!(api.is_file(), "missing {}", api.display());

    let version = Command::new(&reporter).arg("--help").output().unwrap();
    assert!(version.status.success());
    let help = String::from_utf8_lossy(&version.stdout);
    assert!(help.contains("ingest"), "{help}");

    let plugins = prefix.join("share/toktally/plugins/hermes/scripts/report.sh");
    assert!(
        plugins.is_file(),
        "missing plugin wrapper {}",
        plugins.display()
    );
    let wrapper = fs::read_to_string(&plugins).unwrap();
    assert!(wrapper.contains("toktally"));
}

#[test]
fn installed_reporter_ingests_a_fixture() {
    let dir = tempdir().unwrap();
    let prefix = dir.path().join("prefix");
    let out = run_script("install.sh", &prefix, &[]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let store = dir.path().join("store.json");
    let fixture = repo_root().join("crates/adapters/fixtures/hermes-session.json");
    let ingest = Command::new(prefix.join("bin/toktally"))
        .arg("--store")
        .arg(&store)
        .args(["ingest", "--adapter", "hermes", "--file"])
        .arg(&fixture)
        .output()
        .unwrap();
    assert!(
        ingest.status.success(),
        "{}",
        String::from_utf8_lossy(&ingest.stderr)
    );
    let body = String::from_utf8_lossy(&ingest.stdout);
    assert!(body.contains("18000"), "{body}");
    assert!(body.contains("hermes-sess-1"), "{body}");
}

#[test]
fn update_reinstalls_into_the_same_prefix() {
    let dir = tempdir().unwrap();
    let prefix = dir.path().join("prefix");
    let first = run_script("install.sh", &prefix, &[]);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );

    fs::remove_file(prefix.join("bin/toktally")).unwrap();
    let updated = run_script("update.sh", &prefix, &[]);
    assert!(
        updated.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&updated.stderr),
        String::from_utf8_lossy(&updated.stdout)
    );
    assert!(prefix.join("bin/toktally").is_file());
    assert!(prefix.join("bin/toktally-api").is_file());
}
