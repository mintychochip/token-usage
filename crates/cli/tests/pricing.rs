//! Cost is derived from host model + an internal price table. Users do not submit $/token.

use tempfile::tempdir;
use toktally_cli::{estimate_cost_usd, parse_openrouter_prices, summarize_priced, WireObservation};
use toktally_domain::{
    Harness, ObservationIdentity, ObservationSource, SessionId, SessionStoreCompleteness,
    UsageCounts, UsageObservation,
};
use toktally_store::FileStore;

fn openrouter_fixture() -> serde_json::Value {
    serde_json::json!({
        "data": [
            {
                "id": "anthropic/claude-sonnet-4.6",
                "pricing": { "prompt": "0.000003", "completion": "0.000015" }
            },
            {
                "id": "x-ai/grok-4.6",
                "pricing": { "prompt": "0.000002", "completion": "0.000010" }
            },
            {
                "id": "anthropic/claude-opus-5",
                "pricing": { "prompt": "0.000005", "completion": "0.000025" }
            },
            {
                "id": "anthropic/claude-opus-4",
                "pricing": { "prompt": "0.000015", "completion": "0.000075" }
            }
        ]
    })
}

#[test]
fn unknown_model_or_missing_model_has_no_cost() {
    let table = parse_openrouter_prices(&openrouter_fixture()).unwrap();
    let counts = UsageCounts::new(100, 10);
    assert_eq!(estimate_cost_usd(&table, None, &counts), None);
    assert_eq!(
        estimate_cost_usd(&table, Some("totally-unknown-model"), &counts),
        None
    );
}

#[test]
fn openrouter_prices_estimate_usd_from_host_model_and_counts() {
    let table = parse_openrouter_prices(&openrouter_fixture()).unwrap();
    let fixture: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!(
            "{}/../adapters/fixtures/hermes-session.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap();
    let input = fixture["usage"]["input_tokens"].as_u64().unwrap();
    let output = fixture["usage"]["output_tokens"].as_u64().unwrap();
    let model = fixture["model"].as_str().unwrap();
    let cost = estimate_cost_usd(&table, Some(model), &UsageCounts::new(input, output))
        .expect("known model must have a cost");
    let expected = input as f64 * 0.000003 + output as f64 * 0.000015;
    assert!(
        (cost - expected).abs() < 1e-12,
        "cost {cost} != {expected} from fixture tokens"
    );
}

#[test]
fn grok_alias_matches_openrouter_id() {
    let table = parse_openrouter_prices(&openrouter_fixture()).unwrap();
    let cost = estimate_cost_usd(&table, Some("grok-4.6"), &UsageCounts::new(1000, 0));
    assert_eq!(cost, Some(1000.0 * 0.000002));
}

#[test]
fn context_window_suffix_resolves_to_base_model() {
    let table = parse_openrouter_prices(&openrouter_fixture()).unwrap();
    let counts = UsageCounts::new(1000, 0);
    let opus5 = 1000.0 * 0.000005;
    for host_id in [
        "opus-5-1m",
        "claude-opus-5-1m",
        "anthropic/claude-opus-5-1m",
        "opus-5-200k",
        "OPUS-5-1M",
    ] {
        let cost = estimate_cost_usd(&table, Some(host_id), &counts);
        assert_eq!(cost, Some(opus5), "{host_id} should price as opus-5");
    }
}

#[test]
fn variant_does_not_resolve_to_a_sibling_model() {
    let table = parse_openrouter_prices(&openrouter_fixture()).unwrap();
    let cost = estimate_cost_usd(&table, Some("opus-5-1m"), &UsageCounts::new(1000, 0));
    assert_eq!(cost, Some(1000.0 * 0.000005));
    assert_ne!(cost, Some(1000.0 * 0.000015));
}

#[test]
fn exact_priced_variant_wins_over_the_base_model() {
    let table = parse_openrouter_prices(&serde_json::json!({
        "data": [
            {
                "id": "anthropic/claude-opus-5",
                "pricing": { "prompt": "0.000005", "completion": "0.000025" }
            },
            {
                "id": "anthropic/claude-opus-5-1m",
                "pricing": { "prompt": "0.000010", "completion": "0.000040" }
            }
        ]
    }))
    .unwrap();
    assert_eq!(
        estimate_cost_usd(&table, Some("opus-5-1m"), &UsageCounts::new(1000, 0)),
        Some(1000.0 * 0.000010)
    );
    assert_eq!(
        estimate_cost_usd(&table, Some("opus-5"), &UsageCounts::new(1000, 0)),
        Some(1000.0 * 0.000005)
    );
}

#[test]
fn priced_summary_uses_internal_table_not_user_rates() {
    let dir = tempdir().unwrap();
    let store = FileStore::open(dir.path().join("store.json")).unwrap();
    let fixture: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!(
            "{}/../adapters/fixtures/hermes-session.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap();
    let input = fixture["usage"]["input_tokens"].as_u64().unwrap();
    let output = fixture["usage"]["output_tokens"].as_u64().unwrap();
    let model = fixture["model"].as_str().unwrap();
    store
        .ingest(
            UsageObservation::new(
                ObservationIdentity::new(
                    Harness::Hermes,
                    SessionId::parse("hermes-sess-1").unwrap(),
                ),
                UsageCounts::new(input, output),
                ObservationSource::PluginReport,
                SessionStoreCompleteness::Complete,
            )
            .with_model(model),
        )
        .unwrap();

    let table = parse_openrouter_prices(&openrouter_fixture()).unwrap();
    let summary = summarize_priced(&store.list().unwrap(), 1, Some(&table));
    let expected = input as f64 * 0.000003 + output as f64 * 0.000015;
    let cost = summary
        .estimated_cost_usd
        .expect("summary must include cost");
    assert!((cost - expected).abs() < 1e-12, "{cost} != {expected}");
}

#[test]
fn wire_round_trip_keeps_model_and_does_not_persist_user_price() {
    let obs = UsageObservation::new(
        ObservationIdentity::new(Harness::Hermes, SessionId::parse("h").unwrap()),
        UsageCounts::new(10, 1),
        ObservationSource::PluginReport,
        SessionStoreCompleteness::Complete,
    )
    .with_model("anthropic/claude-sonnet-4.6");
    let wire = WireObservation::from_observation(&obs);
    assert_eq!(wire.model.as_deref(), Some("anthropic/claude-sonnet-4.6"));
    let text = serde_json::to_string(&wire).unwrap();
    assert!(
        !text.contains("price") && !text.contains("usd_per"),
        "users do not submit rates: {text}"
    );
    let back = wire.into_observation().unwrap();
    assert_eq!(back.model(), Some("anthropic/claude-sonnet-4.6"));
}
