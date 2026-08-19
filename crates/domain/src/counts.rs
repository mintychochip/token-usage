//! Token counts carried by an observation.

use serde::{Deserialize, Serialize};

/// Optional extra counts that some hosts report (cache, reasoning).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ExtraCounts {
    /// Tokens served from a prompt cache, when the host reports them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<u64>,
    /// Tokens written into a prompt cache, when the host reports them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<u64>,
    /// Reasoning / thinking tokens, when the host reports them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<u64>,
}

impl ExtraCounts {
    /// True when no extra count is present.
    pub fn is_empty(&self) -> bool {
        self.cache_read.is_none() && self.cache_write.is_none() && self.reasoning.is_none()
    }
}

/// Input and output token totals, plus any extras the host supplied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageCounts {
    input_tokens: u64,
    output_tokens: u64,
    #[serde(default, skip_serializing_if = "ExtraCounts::is_empty")]
    extras: ExtraCounts,
}

impl UsageCounts {
    /// Construct counts with no extras.
    pub fn new(input_tokens: u64, output_tokens: u64) -> Self {
        Self {
            input_tokens,
            output_tokens,
            extras: ExtraCounts::default(),
        }
    }

    /// Attach optional extra counts.
    pub fn with_extras(mut self, extras: ExtraCounts) -> Self {
        self.extras = extras;
        self
    }

    /// Input (prompt) tokens.
    pub fn input_tokens(&self) -> u64 {
        self.input_tokens
    }

    /// Output (completion) tokens.
    pub fn output_tokens(&self) -> u64 {
        self.output_tokens
    }

    /// Optional extra counts.
    pub fn extras(&self) -> &ExtraCounts {
        &self.extras
    }
}
