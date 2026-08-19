//! Named coding-agent harnesses that can report usage.

use serde::{Deserialize, Serialize};

use crate::DomainError;

/// A coding-agent host that can emit token-usage observations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Harness {
    /// Anthropic Claude Code.
    ClaudeCode,
    /// OpenAI Codex.
    Codex,
    /// xAI Grok Build.
    Grok,
    /// oh-my-pi (`omp`).
    OhMyPi,
    /// jcode.
    Jcode,
}

impl Harness {
    /// Every named harness, in stable display order.
    pub const fn all() -> [Harness; 5] {
        [
            Harness::ClaudeCode,
            Harness::Codex,
            Harness::Grok,
            Harness::OhMyPi,
            Harness::Jcode,
        ]
    }

    /// Canonical kebab-case slug used on the wire and in the store.
    pub const fn as_str(self) -> &'static str {
        match self {
            Harness::ClaudeCode => "claude-code",
            Harness::Codex => "codex",
            Harness::Grok => "grok",
            Harness::OhMyPi => "oh-my-pi",
            Harness::Jcode => "jcode",
        }
    }

    /// Parse a slug or common alias (`Claude Code`, `omp`, `grok-build`, …).
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let normalized: String = raw
            .trim()
            .chars()
            .map(|c| {
                if c.is_ascii_whitespace() || c == '_' {
                    '-'
                } else {
                    c.to_ascii_lowercase()
                }
            })
            .collect();
        match normalized.as_str() {
            "claude-code" | "claude" => Ok(Harness::ClaudeCode),
            "codex" => Ok(Harness::Codex),
            "grok" | "grok-build" => Ok(Harness::Grok),
            "oh-my-pi" | "omp" => Ok(Harness::OhMyPi),
            "jcode" => Ok(Harness::Jcode),
            _ => Err(DomainError::UnknownHarness(raw.to_string())),
        }
    }
}

impl std::fmt::Display for Harness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Harness {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Harness::parse(s)
    }
}
