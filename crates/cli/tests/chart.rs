use toktally_cli::{chart_svg, summarize};
use toktally_domain::{
    Harness, ObservationIdentity, ObservationSource, SessionId, SessionStoreCompleteness,
    UsageCounts, UsageObservation,
};

fn observation(session: &str, input: u64, recorded_at: u64, model: &str) -> UsageObservation {
    UsageObservation::new(
        ObservationIdentity::new(Harness::Hermes, SessionId::parse(session).unwrap()),
        UsageCounts::new(input, 0),
        ObservationSource::PluginReport,
        SessionStoreCompleteness::Unknown,
    )
    .with_recorded_at(recorded_at)
    .with_model(model)
}

#[test]
fn chart_svg_renders_daily_provider_segments() {
    let observations = vec![
        observation("a", 100, 1_700_000_100, "openai-codex/gpt-5"),
        observation("b", 50, 1_700_000_200, "anthropic/claude-sonnet"),
        observation("c", 200, 1_700_086_500, "openai-codex/gpt-5"),
    ];
    let summary = summarize(&observations, 1);
    let svg = chart_svg(&summary);

    assert!(svg.contains("Nov 14"), "first day label missing: {svg}");
    assert!(svg.contains("Nov 15"), "second day label missing: {svg}");
    assert!(svg.contains("width=\"200\""), "expected bar width: {svg}");
    assert!(
        svg.contains("height=\"210\""),
        "expected tallest bar height: {svg}"
    );
    assert_eq!(
        svg.matches("<rect x=\"").count(),
        5,
        "three stacked segments and two legend swatches expected: {svg}"
    );
    assert!(svg.contains("openai-codex"));
    assert!(svg.contains("anthropic"));
}

#[test]
fn chart_svg_empty_summary_is_valid_svg() {
    let summary = summarize(&[], 1);
    let svg = chart_svg(&summary);

    assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(svg.ends_with("</svg>"));
    assert!(svg.contains("<rect width=\"100%\" height=\"100%\""));
    assert_eq!(svg.matches("<rect x=\"").count(), 0);
}
