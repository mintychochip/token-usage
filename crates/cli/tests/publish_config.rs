use std::path::PathBuf;

use token_usage_cli::publish_config::{default_config_path, load_or_create, save, PublishConfig};

#[test]
fn publish_config_defaults_and_round_trip() {
    let dir = std::env::temp_dir().join(format!(
        "toktally-publish-config-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let path = dir.join("publish-config.json");

    let cfg = load_or_create(&path).unwrap();
    assert!(cfg.widgets.enabled);
    assert_eq!(cfg.widgets.url, "https://widgets.mintychochip.dev");
    assert!(!cfg.github_pages.enabled);
    assert!(cfg.github_pages.repo.is_empty());

    let mut cfg = cfg;
    cfg.github_pages.enabled = true;
    cfg.github_pages.repo = "mintychochip/token-usage-pages".into();
    save(&path, &cfg).unwrap();

    let loaded = load_or_create(&path).unwrap();
    assert!(loaded.github_pages.enabled);
    assert_eq!(loaded.github_pages.repo, "mintychochip/token-usage-pages");
}
