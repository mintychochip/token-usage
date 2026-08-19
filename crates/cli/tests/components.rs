//! GitHub and website paste snippets built from a real store-backed summary.

use tempfile::tempdir;
use token_usage_cli::{
    github_badge_markdown, render_summary_card, summarize, website_embed_html, USAGE_CARD_JS,
};
use token_usage_domain::{
    Harness, ObservationIdentity, ObservationSource, SessionId, SessionStoreCompleteness,
    UsageCounts, UsageObservation,
};
use token_usage_store::FileStore;

#[test]
fn github_badge_markdown_points_shields_at_the_badge_json_url() {
    let dir = tempdir().unwrap();
    let store = FileStore::open(dir.path().join("store.json")).unwrap();
    store
        .ingest(UsageObservation::new(
            ObservationIdentity::new(Harness::Hermes, SessionId::parse("h").unwrap()),
            UsageCounts::new(18000, 2200),
            ObservationSource::PluginReport,
            SessionStoreCompleteness::Complete,
        ))
        .unwrap();
    let _summary = summarize(&store.list().unwrap(), 1);
    let badge_url = "https://example.com/usage/usage-badge.json";
    let md = github_badge_markdown(badge_url);
    assert!(
        md.contains("img.shields.io"),
        "GitHub snippet must use shields.io: {md}"
    );
    assert!(
        md.contains("endpoint"),
        "must use the shields endpoint badge: {md}"
    );
    assert!(
        md.contains("usage-badge.json") || md.contains("example.com"),
        "must point at the published badge JSON: {md}"
    );
    assert!(
        !md.contains("session_id") && !md.contains("hermes-sess"),
        "badge snippet must not leak session ids: {md}"
    );
}

#[test]
fn website_embed_fetches_summary_json_and_omits_session_ids() {
    let summary_url = "https://example.com/usage/usage-summary.json";
    let html = website_embed_html(summary_url);
    assert!(
        html.contains("usage-summary.json"),
        "embed must load the published summary: {html}"
    );
    assert!(
        !html.contains("session_id"),
        "embed markup must not include session_id: {html}"
    );
    assert!(
        !html.contains("require(") && !html.contains("module.exports"),
        "embed must run in a browser, not Node: {html}"
    );
    assert!(
        !html.contains("script src") && !html.contains("<script src"),
        "gist raw cannot serve JS; embed must inline the script: {html}"
    );
    assert!(
        html.contains("<script>") && (html.contains("fetch") || html.contains("data-summary-url")),
        "inlined snippet must still fetch the summary: {html}"
    );
}

#[test]
fn render_summary_card_uses_store_totals_without_session_ids() {
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
    store
        .ingest(UsageObservation::new(
            ObservationIdentity::new(
                Harness::Hermes,
                SessionId::parse(fixture["session_id"].as_str().unwrap()).unwrap(),
            ),
            UsageCounts::new(input, output),
            ObservationSource::PluginReport,
            SessionStoreCompleteness::Complete,
        ))
        .unwrap();
    let summary = summarize(&store.list().unwrap(), 1);
    let card = render_summary_card(&summary);
    assert!(card.contains(&input.to_string()), "{card}");
    assert!(card.contains(&output.to_string()), "{card}");
    assert!(
        !card.contains("session_id") && !card.contains("hermes-sess"),
        "card must not leak session ids: {card}"
    );
    if let Some(cost) = summary.estimated_cost_usd {
        let _ = cost;
    }
}

#[test]
fn shipped_usage_card_script_is_browser_only() {
    assert!(
        USAGE_CARD_JS.contains("fetch") || USAGE_CARD_JS.contains("data-summary-url"),
        "script must load published JSON"
    );
    assert!(
        !USAGE_CARD_JS.contains("session_id"),
        "script must not mention session_id"
    );
    assert!(
        !USAGE_CARD_JS.contains("require(") && !USAGE_CARD_JS.contains("module.exports"),
        "script must not use Node module APIs"
    );
}
