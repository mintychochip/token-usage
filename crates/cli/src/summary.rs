//! Chart- and gist-friendly rollups of stored observations.

use serde::{Deserialize, Serialize};
use toktally_domain::{Harness, ObservationSource, UsageObservation};

use crate::pricing::{estimate_cost_usd, PriceTable};

/// Totals for one harness across every stored session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessTotals {
    pub harness: Harness,
    pub sessions: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_synced_at: Option<u64>,
}

/// Token totals for one host model id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelTotals {
    pub model: String,
    pub sessions: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Token totals for one UTC calendar day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayTotals {
    /// Unix seconds at 00:00 UTC of the day.
    pub day: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Public snapshot someone can commit, gist, or chart. No session ids.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageSummary {
    pub generated_at: u64,
    pub harnesses: Vec<HarnessTotals>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// Derived from host model + internal price table. Omitted when unknown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_cost_usd: Option<f64>,
    /// Per-model breakdown, sorted by input tokens descending. Omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<ModelTotals>,
    /// Per-day totals, oldest first. Omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub days: Vec<DayTotals>,
}

/// shields.io endpoint badge JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShieldsBadge {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    pub label: String,
    pub message: String,
}

/// Roll up observations by harness. Session ids are omitted for public gists.
pub fn summarize(observations: &[UsageObservation], generated_at: u64) -> UsageSummary {
    summarize_priced(observations, generated_at, None)
}

/// Same rollup, with USD estimates when a price table is available.
pub fn summarize_priced(
    observations: &[UsageObservation],
    generated_at: u64,
    prices: Option<&PriceTable>,
) -> UsageSummary {
    let selected: Vec<&UsageObservation> = observations_for_summary(observations);
    let mut rows: Vec<HarnessTotals> = Vec::new();
    for obs in &selected {
        let harness = obs.identity().harness();
        if let Some(row) = rows.iter_mut().find(|row| row.harness == harness) {
            row.sessions += 1;
            row.input_tokens += obs.counts().input_tokens();
            row.output_tokens += obs.counts().output_tokens();
            row.last_synced_at = max_ts(row.last_synced_at, obs.last_synced_at());
        } else {
            rows.push(HarnessTotals {
                harness,
                sessions: 1,
                input_tokens: obs.counts().input_tokens(),
                output_tokens: obs.counts().output_tokens(),
                last_synced_at: obs.last_synced_at(),
            });
        }
    }
    rows.sort_by_key(|row| row.harness.as_str().to_string());
    let input_tokens = rows.iter().map(|r| r.input_tokens).sum();
    let output_tokens = rows.iter().map(|r| r.output_tokens).sum();
    let models = model_totals(&selected);
    let days = day_totals(&selected);
    let estimated_cost_usd = prices.and_then(|table| {
        let mut total = 0.0;
        let mut any = false;
        for obs in &selected {
            if let Some(cost) = estimate_cost_usd(table, obs.model(), obs.counts()) {
                total += cost;
                any = true;
            }
        }
        any.then_some(total)
    });
    UsageSummary {
        generated_at,
        harnesses: rows,
        input_tokens,
        output_tokens,
        estimated_cost_usd,
        models,
        days,
    }
}

/// Roll up totals per host model, sorted by input tokens descending.
fn model_totals(observations: &[&UsageObservation]) -> Vec<ModelTotals> {
    let mut rows: Vec<ModelTotals> = Vec::new();
    for obs in observations {
        let Some(model) = obs.model() else { continue };
        if let Some(row) = rows.iter_mut().find(|row| row.model == model) {
            row.sessions += 1;
            row.input_tokens += obs.counts().input_tokens();
            row.output_tokens += obs.counts().output_tokens();
        } else {
            rows.push(ModelTotals {
                model: model.to_string(),
                sessions: 1,
                input_tokens: obs.counts().input_tokens(),
                output_tokens: obs.counts().output_tokens(),
            });
        }
    }
    rows.sort_by(|a, b| b.input_tokens.cmp(&a.input_tokens));
    rows
}

/// Roll up totals per UTC calendar day, oldest first.
fn day_totals(observations: &[&UsageObservation]) -> Vec<DayTotals> {
    let mut rows: Vec<DayTotals> = Vec::new();
    for obs in observations {
        let Some(at) = obs.recorded_at() else { continue };
        let day = day_start_utc(at);
        if let Some(row) = rows.iter_mut().find(|row| row.day == day) {
            row.input_tokens += obs.counts().input_tokens();
            row.output_tokens += obs.counts().output_tokens();
        } else {
            rows.push(DayTotals {
                day,
                input_tokens: obs.counts().input_tokens(),
                output_tokens: obs.counts().output_tokens(),
            });
        }
    }
    rows.sort_by_key(|row| row.day);
    rows
}

/// Floor a Unix timestamp to the start of its UTC calendar day.
fn day_start_utc(unix_seconds: u64) -> u64 {
    let days = unix_seconds / 86400;
    days * 86400
}

fn observations_for_summary(observations: &[UsageObservation]) -> Vec<&UsageObservation> {
    let mut has_plugin = Vec::new();
    for obs in observations {
        if obs.source() == ObservationSource::PluginReport {
            let harness = obs.identity().harness();
            if !has_plugin.contains(&harness) {
                has_plugin.push(harness);
            }
        }
    }
    observations
        .iter()
        .filter(|obs| {
            obs.source() != ObservationSource::HarnessGlobalApproximation
                || !has_plugin.contains(&obs.identity().harness())
        })
        .collect()
}

/// Compact badge for README shields.io custom endpoints.
pub fn shields_badge(summary: &UsageSummary) -> ShieldsBadge {
    ShieldsBadge {
        schema_version: 1,
        label: "token usage".to_string(),
        message: format!(
            "{} in / {} out",
            compact_count(summary.input_tokens),
            compact_count(summary.output_tokens)
        ),
    }
}

fn max_ts(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (Some(x), None) => Some(x),
        (None, Some(y)) => Some(y),
        (None, None) => None,
    }
}

fn compact_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
