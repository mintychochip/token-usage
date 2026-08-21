//! Persisted target selection for `token-usage-reporter publish`.
//!
//! The default config enables the central widgets service and disables
//! GitHub Pages. Users may opt in by editing `~/.toktally/publish-config.json`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetsTarget {
    pub enabled: bool,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubPagesTarget {
    pub enabled: bool,
    pub repo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishConfig {
    pub widgets: WidgetsTarget,
    pub github_pages: GithubPagesTarget,
}

impl Default for PublishConfig {
    fn default() -> Self {
        Self {
            widgets: WidgetsTarget {
                enabled: true,
                url: "https://widgets.mintychochip.dev".into(),
            },
            github_pages: GithubPagesTarget {
                enabled: false,
                repo: String::new(),
            },
        }
    }
}

/// Returns the user's publish-config path inside the toktally home.
///
/// Uses `TOKTALLY_IDENTITY_DIR` for test overrides; otherwise `~/.toktally`.
pub fn default_config_path() -> PathBuf {
    crate::identity::key_dir()
        .parent()
        .expect("key_dir always has a parent")
        .join("publish-config.json")
}

/// Load the config, or write and return the default if it does not exist.
pub fn load_or_create(path: &Path) -> Result<PublishConfig, String> {
    if !path.exists() {
        let cfg = PublishConfig::default();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(
            path,
            serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        return Ok(cfg);
    }
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

/// Persist an updated config.
pub fn save(path: &Path, cfg: &PublishConfig) -> Result<(), String> {
    std::fs::write(
        path,
        serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
