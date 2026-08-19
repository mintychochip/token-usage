//! Host wrappers must exec the Rust reporter rather than re-implement ingest.

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const WRAPPERS: &[&str] = &[
    "plugins/claude-code/scripts/report.sh",
    "plugins/codex/scripts/report.sh",
    "plugins/grok/scripts/report.sh",
    "plugins/oh-my-pi/scripts/report.sh",
    "plugins/jcode/scripts/report.sh",
    "plugins/hermes/scripts/report.sh",
    "plugins/opencode/scripts/report.sh",
    "plugins/gemini-cli/scripts/report.sh",
    "plugins/aider/scripts/report.sh",
    "plugins/goose/scripts/report.sh",
    "plugins/amp/scripts/report.sh",
    "plugins/droid/scripts/report.sh",
    "plugins/cline/scripts/report.sh",
    "plugins/pi/scripts/report.sh",
];

#[test]
fn host_wrappers_exist_and_invoke_the_rust_reporter() {
    for rel in WRAPPERS {
        let path = repo_root().join(rel);
        let body = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        assert!(
            body.contains("token-usage-reporter"),
            "{rel} must exec token-usage-reporter"
        );
        assert!(
            !body.contains("input_tokens") && !body.contains("serde_json"),
            "{rel} must not re-implement ingest"
        );
    }
}

#[test]
fn plugin_manifests_exist_for_all_named_harnesses() {
    let manifests = [
        "plugins/claude-code/.claude-plugin/plugin.json",
        "plugins/codex/.codex-plugin/plugin.json",
        "plugins/grok/plugin.json",
        "plugins/oh-my-pi/.omp-plugin/plugin.json",
        "plugins/jcode/plugin.json",
        "plugins/hermes/plugin.json",
        "plugins/opencode/package.json",
        "plugins/gemini-cli/gemini-extension.json",
        "plugins/aider/plugin.json",
        "plugins/goose/plugin.json",
        "plugins/amp/plugin.json",
        "plugins/droid/.factory-plugin/plugin.json",
        "plugins/cline/plugin.json",
        "plugins/pi/package.json",
    ];
    for rel in manifests {
        let path = repo_root().join(rel);
        assert!(path.is_file(), "missing {rel}");
        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(parsed.get("name").is_some(), "{rel} needs a name");
    }
}
