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
    let raw = first_str(payload, &["session_id", "sessionId", "sessionID"])
        .or_else(|| payload.pointer("/session/id").and_then(Value::as_str))
        .ok_or(AdaptError::MissingSessionId)?;
    SessionId::parse(raw).map_err(|_| AdaptError::MissingSessionId)
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
