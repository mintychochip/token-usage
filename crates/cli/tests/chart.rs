//! Regression tests for the README chart SVG.

use toktally_cli::{chart_svg, DayTotals, ProviderDayTotals, UsageSummary};

fn day(day: u64, provs: &[(&str, u64)]) -> DayTotals {
    let providers: Vec<ProviderDayTotals> = provs
        .iter()
        .map(|(name, tokens)| ProviderDayTotals {
            provider: (*name).to_string(),
            input_tokens: *tokens,
        })
        .collect();
    DayTotals {
        day,
        input_tokens: providers.iter().map(|p| p.input_tokens).sum(),
        output_tokens: 0,
        providers,
    }
}

fn summary(days: Vec<DayTotals>) -> UsageSummary {
    UsageSummary {
        generated_at: 1_787_800_000,
        harnesses: vec![],
        input_tokens: days.iter().map(|d| d.input_tokens).sum(),
        output_tokens: days.iter().map(|d| d.output_tokens).sum(),
        estimated_cost_usd: Some(12.5),
        models: vec![],
        days,
    }
}

fn fill_colors(svg: &str) -> Vec<String> {
    svg.match_indices("fill=\"#")
        .map(|(i, _)| svg[i + 6..i + 13].to_string())
        .collect()
}

#[test]
fn distinct_providers_never_share_a_bar_color() {
    let names = [
        "anthropic",
        "openai",
        "codex",
        "google",
        "gemini",
        "xai",
        "acme",
        "widgetco",
        "provider-a",
        "provider-b",
        "provider-c",
        "provider-d",
        "provider-e",
        "provider-f",
        "provider-g",
        "provider-h",
    ];
    let provs: Vec<(&str, u64)> = names
        .iter()
        .enumerate()
        .map(|(i, n)| (*n, 10_000 + i as u64 * 1_000))
        .collect();
    let svg = chart_svg(&summary(vec![day(1_787_000_000, &provs)]));

    let colors: Vec<&str> = svg
        .split("fill=\"")
        .skip(1)
        .filter_map(|part| part.split('"').next())
        .collect();
    let mut unique = colors.clone();
    unique.sort();
    unique.dedup();
    // One bar segment plus one legend swatch per provider, all distinct.
    assert_eq!(
        unique.len(),
        names.len(),
        "each provider needs its own color: {unique:?}"
    );
}

#[test]
fn known_providers_keep_their_brand_color() {
    let svg = chart_svg(&summary(vec![day(1_787_000_000, &[("anthropic", 5_000)])]));
    assert!(
        svg.contains("#D97757"),
        "anthropic should render in its brand color: {svg}"
    );
}

#[test]
fn date_labels_are_thinned_so_they_cannot_overlap() {
    let days: Vec<DayTotals> = (0..90)
        .map(|i| day(1_787_000_000 + i * 86400, &[("openai", 1_000 + i)]))
        .collect();
    let svg = chart_svg(&summary(days));

    let labels = svg.matches("text-anchor=\"middle\"").count();
    // The plot is ~650px wide and each label needs ~54px of room.
    assert!(
        labels <= 13,
        "90 days must not draw 90 date labels, drew {labels}"
    );
    assert!(labels >= 4, "chart still needs date context, drew {labels}");
}

#[test]
fn gridline_labels_are_round_numbers() {
    let svg = chart_svg(&summary(vec![day(1_787_000_000, &[("openai", 3_321)])]));
    for tick in ["0", "1k", "2k", "3k", "4k"] {
        assert!(
            svg.contains(&format!(">{tick}</text>")),
            "expected a {tick} gridline label: {svg}"
        );
    }
}

#[test]
fn chart_adapts_to_the_reader_theme_and_stays_transparent() {
    let svg = chart_svg(&summary(vec![day(1_787_000_000, &[("openai", 5_000)])]));
    assert!(
        svg.contains("prefers-color-scheme: dark"),
        "chart must restyle for dark READMEs: {svg}"
    );
    assert!(
        !svg.contains("fill=\"#121212\""),
        "chart must not paint an opaque background: {svg}"
    );
}

#[test]
fn empty_summary_renders_a_header_instead_of_an_empty_plot() {
    let svg = chart_svg(&summary(vec![]));
    assert!(svg.contains("toktally"), "header is still expected: {svg}");
    assert!(!svg.contains("<rect"), "no bars without days: {svg}");
}
