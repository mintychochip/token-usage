//! Copy-paste GitHub and website snippets over published usage JSON.

use crate::summary::UsageSummary;

/// Browser script that fetches `usage-summary.json` and fills `.token-usage-card`.
pub const USAGE_CARD_JS: &str = include_str!("../../../embed/usage-card.js");

/// Shields.io endpoint markdown for a published `usage-badge.json` URL.
pub fn github_badge_markdown(badge_json_url: &str) -> String {
    let encoded = encode_query(badge_json_url);
    format!("[![token usage](https://img.shields.io/endpoint?url={encoded})]")
}

/// HTML paste snippet that loads the published summary (no session ids).
///
/// The card script is inlined. Gist raw URLs are `text/plain` with nosniff,
/// so a `<script src>` to `usage-card.js` would not run.
pub fn website_embed_html(summary_json_url: &str) -> String {
    format!(
        "<div class=\"token-usage-card\" data-summary-url=\"{}\"></div>\n<script>\n{}\n</script>\n",
        escape_attr(summary_json_url),
        USAGE_CARD_JS.trim()
    )
}

/// Visible card text/HTML from a public summary. Never includes session ids.
pub fn render_summary_card(summary: &UsageSummary) -> String {
    let mut line = format!(
        "token usage {} in / {} out",
        summary.input_tokens, summary.output_tokens
    );
    if let Some(cost) = summary.estimated_cost_usd {
        line.push_str(&format!(" · ~${cost:.4}"));
    }
    line
}

/// Join a publish base URL with a file name (`usage-badge.json`, …).
pub fn join_published_url(base: &str, file: &str) -> String {
    let base = base.trim_end_matches('/');
    if base.is_empty() || base == "." {
        return file.to_string();
    }
    format!("{base}/{file}")
}

/// Both paste snippets for a published directory or gist base URL.
pub fn publish_snippets(base_url: &str) -> String {
    let badge = join_published_url(base_url, "usage-badge.json");
    let summary = join_published_url(base_url, "usage-summary.json");
    format!(
        "GitHub README:\n{}\n\nWebsite:\n{}",
        github_badge_markdown(&badge),
        website_embed_html(&summary)
    )
}

/// Raw gist file prefix: `https://gist.githubusercontent.com/{owner}/{id}/raw`.
pub fn gist_raw_base(owner: &str, id: &str) -> String {
    format!("https://gist.githubusercontent.com/{owner}/{id}/raw")
}

fn encode_query(raw: &str) -> String {
    let mut out = String::new();
    for byte in raw.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn escape_attr(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
