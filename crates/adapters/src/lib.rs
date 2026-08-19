//! Harness payload adapters.
//!
//! Each adapter only translates JSON into a [`UsageObservation`]. Persistence
//! belongs to the store.

use serde_json::Value;
use token_usage_domain::{
    ExtraCounts, Harness, ObservationIdentity, ObservationSource, SessionId,
    SessionStoreCompleteness, UsageCounts, UsageObservation,
};

/// Failures while mapping a harness payload.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AdaptError {
    #[error("missing session id in payload")]
    MissingSessionId,
    #[error("missing token counts in payload")]
    MissingCounts,
}

/// Map a payload for a named harness into a domain observation.
pub fn adapt(harness: Harness, payload: &Value) -> Result<UsageObservation, AdaptError> {
    match harness {
        Harness::ClaudeCode => adapt_claude_code(payload),
        Harness::Codex => adapt_codex(payload),
        Harness::Grok => adapt_grok(payload),
        Harness::OhMyPi => adapt_oh_my_pi(payload),
        Harness::Jcode => adapt_jcode(payload),
        Harness::Hermes => adapt_hermes(payload),
        Harness::OpenCode => adapt_opencode(payload),
        Harness::GeminiCli => adapt_gemini_cli(payload),
        Harness::Aider => adapt_aider(payload),
        Harness::Goose => adapt_goose(payload),
        Harness::Amp => adapt_amp(payload),
        Harness::Droid => adapt_droid(payload),
        Harness::Cline => adapt_cline(payload),
        Harness::Pi => adapt_pi(payload),
    }
}

/// Claude Code Stop-hook or `/usage` snapshot.
pub fn adapt_claude_code(payload: &Value) -> Result<UsageObservation, AdaptError> {
    let global = is_global(payload);
    let session = if global {
        SessionId::harness_global()
    } else {
        session_id(payload)?
    };
    let usage = first_object(payload, &["usage", "token_usage"]).unwrap_or(payload);
    let counts = counts_from(
        usage,
        &["input_tokens", "inputTokens"],
        &["output_tokens", "outputTokens"],
        &[
            "cache_read_input_tokens",
            "cacheReadTokens",
            "cache_read",
        ],
        &[
            "cache_creation_input_tokens",
            "cache_write",
            "cacheWriteTokens",
        ],
        &["reasoning_tokens", "reasoning"],
    )?;
    Ok(UsageObservation::new(
        ObservationIdentity::new(Harness::ClaudeCode, session),
        counts,
        if global {
            ObservationSource::HarnessGlobalApproximation
        } else {
            ObservationSource::PluginReport
        },
        if global {
            SessionStoreCompleteness::Partial
        } else {
            SessionStoreCompleteness::Complete
        },
    ))
}

/// Codex Stop-hook usage snapshot.
pub fn adapt_codex(payload: &Value) -> Result<UsageObservation, AdaptError> {
    let usage = first_object(payload, &["token_usage", "usage"]).unwrap_or(payload);
    let counts = counts_from(
        usage,
        &["input_tokens", "inputTokens"],
        &["output_tokens", "outputTokens"],
        &["cached_input_tokens", "cache_read", "cacheReadTokens"],
        &["cache_write"],
        &["reasoning_tokens", "reasoning"],
    )?;
    Ok(UsageObservation::new(
        ObservationIdentity::new(Harness::Codex, session_id(payload)?),
        counts,
        ObservationSource::PluginReport,
        SessionStoreCompleteness::Complete,
    ))
}

/// Grok Build hook or `signals.json` fragment. Session store may be incomplete.
pub fn adapt_grok(payload: &Value) -> Result<UsageObservation, AdaptError> {
    let session = session_id(payload)?;
    let usage = first_object(payload, &["usage", "token_usage"]);
    let (counts, completeness) = if let Some(usage) = usage {
        if let Ok(counts) = counts_from(
            usage,
            &["input_tokens", "inputTokens"],
            &["output_tokens", "outputTokens"],
            &["cache_read_tokens", "cache_read"],
            &["cache_write"],
            &["reasoning_tokens"],
        ) {
            (counts, SessionStoreCompleteness::Partial)
        } else {
            grok_context_counts(payload)?
        }
    } else {
        grok_context_counts(payload)?
    };
    Ok(UsageObservation::new(
        ObservationIdentity::new(Harness::Grok, session),
        counts,
        ObservationSource::PluginReport,
        completeness,
    ))
}

/// oh-my-pi session stats or global aggregation.
pub fn adapt_oh_my_pi(payload: &Value) -> Result<UsageObservation, AdaptError> {
    let global = is_global(payload);
    let session = if global {
        SessionId::harness_global()
    } else {
        session_id(payload)?
    };
    let usage = first_object(payload, &["stats", "usage", "totals"]).unwrap_or(payload);
    let counts = counts_from(
        usage,
        &["inputTokens", "input_tokens"],
        &["outputTokens", "output_tokens"],
        &["cacheReadTokens", "cache_read"],
        &["cacheWriteTokens", "cache_write"],
        &["reasoningTokens", "reasoning"],
    )?;
    Ok(UsageObservation::new(
        ObservationIdentity::new(Harness::OhMyPi, session),
        counts,
        if global {
            ObservationSource::HarnessGlobalApproximation
        } else {
            ObservationSource::PluginReport
        },
        if global {
            SessionStoreCompleteness::Partial
        } else {
            SessionStoreCompleteness::Complete
        },
    ))
}

/// jcode usage snapshot (prompt/completion or input/output).
pub fn adapt_jcode(payload: &Value) -> Result<UsageObservation, AdaptError> {
    let usage = first_object(payload, &["usage", "token_usage"]).unwrap_or(payload);
    let counts = counts_from(
        usage,
        &["prompt_tokens", "input_tokens", "inputTokens"],
        &["completion_tokens", "output_tokens", "outputTokens"],
        &["cache_read", "cached_tokens"],
        &["cache_write"],
        &["reasoning_tokens"],
    )?;
    Ok(UsageObservation::new(
        ObservationIdentity::new(Harness::Jcode, session_id(payload)?),
        counts,
        ObservationSource::PluginReport,
        SessionStoreCompleteness::Complete,
    ))
}

/// Hermes Agent `post_api_request` usage snapshot.
pub fn adapt_hermes(payload: &Value) -> Result<UsageObservation, AdaptError> {
    adapt_standard(Harness::Hermes, payload)
}

/// OpenCode session event (`session.idle` / plugin event envelope).
pub fn adapt_opencode(payload: &Value) -> Result<UsageObservation, AdaptError> {
    adapt_standard(Harness::OpenCode, payload)
}

/// Gemini CLI AfterAgent /stats snapshot.
pub fn adapt_gemini_cli(payload: &Value) -> Result<UsageObservation, AdaptError> {
    adapt_standard(Harness::GeminiCli, payload)
}

/// Aider `/tokens` or session usage dump.
pub fn adapt_aider(payload: &Value) -> Result<UsageObservation, AdaptError> {
    adapt_standard(Harness::Aider, payload)
}

/// Goose session export (Open Plugins hook or JSON export).
pub fn adapt_goose(payload: &Value) -> Result<UsageObservation, AdaptError> {
    adapt_standard(Harness::Goose, payload)
}

/// Sourcegraph Amp session usage.
pub fn adapt_amp(payload: &Value) -> Result<UsageObservation, AdaptError> {
    adapt_standard(Harness::Amp, payload)
}

/// Factory Droid Stop-hook usage.
pub fn adapt_droid(payload: &Value) -> Result<UsageObservation, AdaptError> {
    adapt_standard(Harness::Droid, payload)
}

/// Cline task usage (`tokensIn` / `tokensOut`).
pub fn adapt_cline(payload: &Value) -> Result<UsageObservation, AdaptError> {
    adapt_standard(Harness::Cline, payload)
}

/// Pi `get_session_stats` tokens object.
pub fn adapt_pi(payload: &Value) -> Result<UsageObservation, AdaptError> {
    adapt_standard(Harness::Pi, payload)
}

fn adapt_standard(harness: Harness, payload: &Value) -> Result<UsageObservation, AdaptError> {
    let global = is_global(payload);
    let session = if global {
        SessionId::harness_global()
    } else {
        session_id(payload)?
    };
    let usage = usage_object(payload);
    let counts = extract_counts(usage)?;
    Ok(UsageObservation::new(
        ObservationIdentity::new(harness, session),
        counts,
        if global {
            ObservationSource::HarnessGlobalApproximation
        } else {
            ObservationSource::PluginReport
        },
        if global {
            SessionStoreCompleteness::Partial
        } else {
            SessionStoreCompleteness::Complete
        },
    ))
}

fn grok_context_counts(
    payload: &Value,
) -> Result<(UsageCounts, SessionStoreCompleteness), AdaptError> {
    let input = first_u64(payload, &["contextTokensUsed", "context_tokens_used"])
        .ok_or(AdaptError::MissingCounts)?;
    Ok((
        UsageCounts::new(input, 0),
        SessionStoreCompleteness::Unknown,
    ))
}

fn is_global(payload: &Value) -> bool {
    matches!(payload.get("kind").and_then(Value::as_str), Some("global_usage"))
        || matches!(payload.get("scope").and_then(Value::as_str), Some("global"))
}

fn session_id(payload: &Value) -> Result<SessionId, AdaptError> {
    let raw = first_str(
        payload,
        &[
            "session_id",
            "sessionId",
            "sessionID",
            "taskId",
            "task_id",
            "id",
        ],
    )
    .or_else(|| payload.pointer("/session/id").and_then(Value::as_str))
    .or_else(|| payload.pointer("/properties/sessionId").and_then(Value::as_str))
    .or_else(|| payload.pointer("/properties/session_id").and_then(Value::as_str))
    .ok_or(AdaptError::MissingSessionId)?;
    SessionId::parse(raw).map_err(|_| AdaptError::MissingSessionId)
}

fn usage_object(payload: &Value) -> &Value {
    for pointer in [
        "/usage",
        "/token_usage",
        "/tokens",
        "/stats/tokens",
        "/stats",
        "/properties/tokens",
        "/totals",
    ] {
        if let Some(value) = payload.pointer(pointer).filter(|value| value.is_object()) {
            if looks_like_counts(value) {
                return value;
            }
        }
    }
    payload
}

fn looks_like_counts(value: &Value) -> bool {
    [
        "input_tokens",
        "inputTokens",
        "prompt_tokens",
        "tokensIn",
        "input",
        "output_tokens",
        "outputTokens",
        "completion_tokens",
        "tokensOut",
        "output",
    ]
    .iter()
    .any(|key| value.get(*key).is_some())
}

fn extract_counts(usage: &Value) -> Result<UsageCounts, AdaptError> {
    let cache_read = first_u64(
        usage,
        &[
            "cache_read_input_tokens",
            "cache_read_tokens",
            "cached_input_tokens",
            "cacheReadTokens",
            "cache_read",
            "cached",
        ],
    )
    .or_else(|| usage.pointer("/cache/read").and_then(Value::as_u64));
    counts_from(
        usage,
        &[
            "input_tokens",
            "inputTokens",
            "prompt_tokens",
            "tokensIn",
            "input",
        ],
        &[
            "output_tokens",
            "outputTokens",
            "completion_tokens",
            "tokensOut",
            "output",
        ],
        &[],
        &["cache_write", "cacheWriteTokens", "cache_creation_input_tokens"],
        &["reasoning_tokens", "reasoning"],
    )
    .map(|counts| {
        let extras = counts.extras().clone();
        if cache_read.is_none() && extras.is_empty() {
            counts
        } else {
            counts.with_extras(ExtraCounts {
                cache_read: cache_read.or(extras.cache_read),
                cache_write: extras.cache_write,
                reasoning: extras.reasoning,
            })
        }
    })
}

fn first_object<'a>(payload: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter()
        .find_map(|key| payload.get(key))
        .filter(|value| value.is_object())
}

fn first_str<'a>(payload: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| payload.get(*key).and_then(Value::as_str))
}

fn first_u64(payload: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| {
        payload.get(*key).and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_i64().and_then(|n| u64::try_from(n).ok()))
        })
    })
}

fn counts_from(
    usage: &Value,
    input_keys: &[&str],
    output_keys: &[&str],
    cache_read_keys: &[&str],
    cache_write_keys: &[&str],
    reasoning_keys: &[&str],
) -> Result<UsageCounts, AdaptError> {
    let input = first_u64(usage, input_keys).ok_or(AdaptError::MissingCounts)?;
    let output = first_u64(usage, output_keys).ok_or(AdaptError::MissingCounts)?;
    Ok(UsageCounts::new(input, output).with_extras(ExtraCounts {
        cache_read: first_u64(usage, cache_read_keys),
        cache_write: first_u64(usage, cache_write_keys),
        reasoning: first_u64(usage, reasoning_keys),
    }))
}
