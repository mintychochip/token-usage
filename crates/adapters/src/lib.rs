//! Harness payload adapters.
//!
//! Each adapter only translates JSON into a [`UsageObservation`]. Persistence
//! belongs to the store.

use serde_json::Value;
use toktally_domain::{
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
    let observation = match harness {
        Harness::ClaudeCode => adapt_claude_code(payload)?,
        Harness::Codex => adapt_codex(payload)?,
        Harness::Grok => adapt_grok(payload)?,
        Harness::OhMyPi => adapt_oh_my_pi(payload)?,
        Harness::Jcode => adapt_jcode(payload)?,
        Harness::Hermes => adapt_hermes(payload)?,
        Harness::OpenCode => adapt_opencode(payload)?,
        Harness::GeminiCli => adapt_gemini_cli(payload)?,
        Harness::Aider => adapt_aider(payload)?,
        Harness::Goose => adapt_goose(payload)?,
        Harness::Amp => adapt_amp(payload)?,
        Harness::Droid => adapt_droid(payload)?,
        Harness::Cline => adapt_cline(payload)?,
        Harness::Pi => adapt_pi(payload)?,
    };
    Ok(attach_recorded_at(attach_model(observation, payload), payload))
}

fn attach_model(observation: UsageObservation, payload: &Value) -> UsageObservation {
    match extract_model(payload) {
        Some(model) => observation.with_model(model),
        None => observation,
    }
}

fn attach_recorded_at(observation: UsageObservation, payload: &Value) -> UsageObservation {
    match extract_recorded_at(payload) {
        Some(at) => observation.with_recorded_at(at),
        None => observation,
    }
}

fn extract_recorded_at(payload: &Value) -> Option<u64> {
    payload
        .get("recordedAt")
        .and_then(Value::as_u64)
        .or_else(|| {
            payload
                .get("timestamp")
                .and_then(Value::as_str)
                .and_then(parse_iso_timestamp)
        })
}

fn extract_model(payload: &Value) -> Option<String> {
    first_str(
        payload,
        &[
            "model",
            "model_id",
            "modelId",
            "primaryModelId",
            "primary_model",
        ],
    )
    .or_else(|| payload.pointer("/session/model").and_then(Value::as_str))
    .or_else(|| {
        payload
            .get("modelsUsed")
            .and_then(Value::as_array)
            .and_then(|arr| arr.iter().find_map(Value::as_str))
    })
    .map(str::to_string)
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
        &["cache_read_input_tokens", "cacheReadTokens", "cache_read"],
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
    let compaction_count = first_u64(payload, &["compactionCount"]).unwrap_or(0);
    let before = first_u64(payload, &["totalTokensBeforeCompaction", "tokens_before"])
        .filter(|&n| n > 0 || compaction_count > 0);
    let after = first_u64(payload, &["tokens_after"]).or(before.map(|_| input));
    let counts = UsageCounts::new(input, 0).with_extras(ExtraCounts {
        tokens_before: before,
        tokens_after: after,
        ..ExtraCounts::default()
    });
    Ok((counts, SessionStoreCompleteness::Unknown))
}

fn is_global(payload: &Value) -> bool {
    matches!(
        payload.get("kind").and_then(Value::as_str),
        Some("global_usage")
    ) || matches!(payload.get("scope").and_then(Value::as_str), Some("global"))
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
    .or_else(|| {
        payload
            .pointer("/properties/sessionId")
            .and_then(Value::as_str)
    })
    .or_else(|| {
        payload
            .pointer("/properties/session_id")
            .and_then(Value::as_str)
    })
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
        &[
            "cache_write",
            "cacheWriteTokens",
            "cache_creation_input_tokens",
        ],
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
                tokens_before: extras.tokens_before,
                tokens_after: extras.tokens_after,
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
    keys.iter()
        .find_map(|key| payload.get(*key).and_then(Value::as_str))
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
        tokens_before: first_u64(usage, &["tokens_before", "totalTokensBeforeCompaction"]),
        tokens_after: first_u64(usage, &["tokens_after"]),
    }))
}

/// Parse an ISO-8601 timestamp (e.g. `2026-07-11T04:53:19.238Z`) to Unix seconds.
fn parse_iso_timestamp(raw: &str) -> Option<u64> {
    let raw = raw.trim();
    let body = raw.strip_suffix('Z').unwrap_or(raw);
    let (date, time) = body.split_once('T')?;
    let mut parts = date.split('-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: i64 = parts.next()?.parse().ok()?;
    let day: i64 = parts.next()?.parse().ok()?;
    let mut time_parts = time.split(':');
    let hour: i64 = time_parts.next()?.parse().ok()?;
    let minute: i64 = time_parts.next()?.parse().ok()?;
    let sec_raw = time_parts.next()?;
    let sec: i64 = sec_raw.split('.').next()?.parse().ok()?;
    Some(ymd_hms_to_unix(year, month, day, hour, minute, sec))
}

/// Days-from-civil algorithm (Howard Hinnant) -> Unix seconds (UTC).
fn ymd_hms_to_unix(year: i64, month: i64, day: i64, hour: i64, min: i64, sec: i64) -> u64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    (days * 86400 + hour * 3600 + min * 60 + sec) as u64
}
