//! Static SVG chart generation for README embeds.
//!
//! GitHub READMEs strip JavaScript, so the chart is rendered server-side as
//! inline SVG and embedded via `<img src="chart.svg">`. Bars are stacked by
//! provider. The background stays transparent and text/grid colors flip with
//! `prefers-color-scheme`, so one file reads correctly on both GitHub themes.

use crate::summary::UsageSummary;

const WIDTH: i64 = 720;
const PAD_LEFT: i64 = 52;
const PAD_RIGHT: i64 = 16;
const PAD_TOP: i64 = 44;
const PLOT_H: i64 = 168;
const PLOT_W: i64 = WIDTH - PAD_LEFT - PAD_RIGHT;
const BASELINE: i64 = PAD_TOP + PLOT_H;
const LABEL_Y: i64 = BASELINE + 16;
const LEGEND_Y: i64 = BASELINE + 42;
const HEIGHT: i64 = LEGEND_Y + 12;

/// Recognisable colors for known providers. Matched as a substring of the
/// lowercased provider id, so `azure-openai` still reads as OpenAI.
const BRAND: [(&str, &str); 15] = [
    ("anthropic", "#D97757"),
    ("claude", "#D97757"),
    ("openai", "#10A37F"),
    ("codex", "#10A37F"),
    ("google", "#4285F4"),
    ("gemini", "#4285F4"),
    ("xai", "#A855F7"),
    ("grok", "#A855F7"),
    ("meta", "#0866FF"),
    ("llama", "#0866FF"),
    ("mistral", "#FF7000"),
    ("deepseek", "#4D6BFE"),
    ("qwen", "#615CED"),
    ("cohere", "#39594D"),
    ("moonshot", "#16C79A"),
];

/// Assigned in usage order to providers with no brand color, so two providers
/// never share a swatch (the old name hash collided).
const FALLBACK: [&str; 8] = [
    "#3B82F6", "#F59E0B", "#10B981", "#EF4444", "#8B5CF6", "#EC4899", "#14B8A6", "#F97316",
];

const FONT_SANS: &str = "&quot;Geist Sans&quot;, system-ui, -apple-system, BlinkMacSystemFont, &quot;Segoe UI&quot;, sans-serif";

/// Theme-aware styles. GitHub renders the file as an image, so the browser
/// applies these rules and the media query follows the reader's theme.
fn style_block() -> String {
    format!(
        r#"<style>
    text {{ font-family: {FONT_SANS}; }}
    .t-title {{ font-size: 14px; font-weight: 600; fill: #1f2328; }}
    .t-sub {{ font-size: 12px; fill: #59636e; }}
    .t-tick {{ font-size: 10px; fill: #818b98; }}
    .t-grid {{ stroke: #d1d9e0; stroke-width: 1; shape-rendering: crispEdges; }}
    .t-axis {{ stroke: #b7bfc7; stroke-width: 1; shape-rendering: crispEdges; }}
    @media (prefers-color-scheme: dark) {{
      .t-title {{ fill: #f0f6fc; }}
      .t-sub {{ fill: #9198a1; }}
      .t-tick {{ fill: #7d8590; }}
      .t-grid {{ stroke: #2a313c; }}
      .t-axis {{ stroke: #3d444d; }}
    }}
  </style>"#
    )
}

/// Render a daily stacked-provider token bar chart as an SVG string.
pub fn chart_svg(summary: &UsageSummary) -> String {
    let days = &summary.days;
    let style = style_block();
    let header = header_markup(summary);

    if days.is_empty() {
        return format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{PAD_TOP}" viewBox="0 0 {WIDTH} {PAD_TOP}" role="img" aria-label="toktally token usage">
  {style}
{header}
</svg>"##
        );
    }

    let order = provider_order(summary);
    let colors = assign_colors(&order);

    let peak = days
        .iter()
        .map(|d| d.input_tokens)
        .max()
        .unwrap_or(1)
        .max(1);
    let (step, ticks) = nice_scale(peak, 4);
    let max = (step * ticks) as f64;

    let mut grid = String::new();
    for i in 0..=ticks {
        let y = BASELINE as f64 - (PLOT_H as f64 * i as f64) / ticks as f64;
        grid.push_str(&format!(
            "<line class=\"t-grid\" x1=\"{PAD_LEFT}\" y1=\"{y:.1}\" x2=\"{}\" y2=\"{y:.1}\"/>",
            WIDTH - PAD_RIGHT
        ));
        grid.push_str(&format!(
            "<text class=\"t-tick\" x=\"{}\" y=\"{:.1}\" text-anchor=\"end\">{}</text>",
            PAD_LEFT - 8,
            y + 3.0,
            compact(step * i)
        ));
    }

    let slot = PLOT_W as f64 / days.len() as f64;
    let bar_w = (slot * 0.64).clamp(2.0, 22.0);
    // Keep ~54px between date labels, anchored to the newest day.
    let per_label = (PLOT_W / 54).max(1) as usize;
    let stride = days.len().div_ceil(per_label).max(1);

    let mut bars = String::new();
    let mut labels = String::new();
    for (i, day) in days.iter().enumerate() {
        let cx = PAD_LEFT as f64 + i as f64 * slot + slot / 2.0;
        let mut y = BASELINE as f64;
        for name in &order {
            let Some(seg) = day.providers.iter().find(|p| &p.provider == name) else {
                continue;
            };
            let h = (seg.input_tokens as f64 / max) * PLOT_H as f64;
            if h < 0.5 {
                continue;
            }
            y -= h;
            bars.push_str(&format!(
                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"1.5\" fill=\"{}\"/>",
                cx - bar_w / 2.0,
                y,
                bar_w,
                h,
                colors[name]
            ));
        }
        if (days.len() - 1 - i).is_multiple_of(stride) {
            labels.push_str(&format!(
                "<text class=\"t-tick\" x=\"{cx:.1}\" y=\"{LABEL_Y}\" text-anchor=\"middle\">{}</text>",
                escape_text(&day_label(day.day))
            ));
        }
    }

    let legend = legend_markup(&order, &colors);

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}" viewBox="0 0 {WIDTH} {HEIGHT}" role="img" aria-label="toktally daily input tokens by provider">
  {style}
{header}
  <g>{grid}</g>
  <g>{bars}</g>
  <g>{labels}</g>
  <line class="t-axis" x1="{PAD_LEFT}" y1="{BASELINE}" x2="{}" y2="{BASELINE}"/>
  <g>{legend}</g>
</svg>"##,
        WIDTH - PAD_RIGHT
    )
}

fn header_markup(summary: &UsageSummary) -> String {
    let mut sub = format!(
        "{} in · {} out",
        compact(summary.input_tokens),
        compact(summary.output_tokens)
    );
    if let Some(cost) = summary.estimated_cost_usd {
        sub.push_str(&format!(" · ~{}", money(cost)));
    }
    format!(
        "  <text class=\"t-title\" x=\"{PAD_LEFT}\" y=\"20\">toktally</text>\n  \
         <text class=\"t-sub\" x=\"{}\" y=\"20\" text-anchor=\"end\">{}</text>",
        WIDTH - PAD_RIGHT,
        escape_text(&sub)
    )
}

fn legend_markup(order: &[String], colors: &std::collections::HashMap<String, String>) -> String {
    let mut out = String::new();
    let mut x = PAD_LEFT;
    for name in order.iter().take(8) {
        // 11px sans averages ~6.2px per character; enough to avoid overlap.
        let width = 14 + (name.chars().count() as f64 * 6.2).ceil() as i64 + 18;
        if x + width > WIDTH - PAD_RIGHT {
            break;
        }
        out.push_str(&format!(
            "<rect x=\"{x}\" y=\"{}\" width=\"9\" height=\"9\" rx=\"2\" fill=\"{}\"/>",
            LEGEND_Y - 8,
            colors[name]
        ));
        out.push_str(&format!(
            "<text class=\"t-tick\" x=\"{}\" y=\"{LEGEND_Y}\">{}</text>",
            x + 14,
            escape_text(name)
        ));
        x += width;
    }
    out
}

/// Providers present in the summary, most-used first.
fn provider_order(summary: &UsageSummary) -> Vec<String> {
    let mut totals: Vec<(String, u64)> = Vec::new();
    for day in &summary.days {
        for p in &day.providers {
            match totals.iter_mut().find(|(name, _)| name == &p.provider) {
                Some(entry) => entry.1 += p.input_tokens,
                None => totals.push((p.provider.clone(), p.input_tokens)),
            }
        }
    }
    totals.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    totals.into_iter().map(|(name, _)| name).collect()
}

/// Brand color when we recognise the provider, otherwise the next unused
/// fallback. Deterministic for a given provider set and collision-free.
fn assign_colors(order: &[String]) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let mut used = std::collections::HashSet::new();
    let mut next = 0usize;
    for (index, name) in order.iter().enumerate() {
        let key = name.to_ascii_lowercase();
        let brand = BRAND
            .iter()
            .find(|(needle, _)| key.contains(needle))
            .map(|(_, color)| (*color).to_string())
            .filter(|color| !used.contains(color));
        let color = brand.unwrap_or_else(|| {
            while next < FALLBACK.len() && used.contains(FALLBACK[next]) {
                next += 1;
            }
            if next < FALLBACK.len() {
                let color = FALLBACK[next].to_string();
                next += 1;
                color
            } else {
                let hue = (index as f64 * 137.508) % 360.0;
                format!("hsl({hue:.1},65%,55%)")
            }
        });
        used.insert(color.clone());
        map.insert(name.clone(), color);
    }
    map
}

/// Pick a `1/2/5 x 10^n` gridline interval so every tick label is round.
/// Returns the interval and how many of them cover `peak`.
fn nice_scale(peak: u64, target: u64) -> (u64, u64) {
    if peak == 0 {
        return (1, 1);
    }
    let raw = peak as f64 / target.max(1) as f64;
    let exp = 10f64.powf(raw.log10().floor());
    let f = raw / exp;
    let mult = if f <= 1.0 {
        1.0
    } else if f <= 2.0 {
        2.0
    } else if f <= 5.0 {
        5.0
    } else {
        10.0
    };
    let step = ((mult * exp) as u64).max(1);
    (step, peak.div_ceil(step).max(1))
}

/// Compact a token count to `12.3M` / `456k`, dropping a trailing `.0`.
fn compact(n: u64) -> String {
    fn trim(v: f64, suffix: &str) -> String {
        let s = format!("{v:.1}");
        format!("{}{suffix}", s.strip_suffix(".0").unwrap_or(&s))
    }
    if n >= 1_000_000_000 {
        trim(n as f64 / 1_000_000_000.0, "B")
    } else if n >= 1_000_000 {
        trim(n as f64 / 1_000_000.0, "M")
    } else if n >= 1_000 {
        trim(n as f64 / 1_000.0, "k")
    } else {
        n.to_string()
    }
}

/// Format an estimated cost with cents, or four places below a dollar.
fn money(v: f64) -> String {
    if v >= 1.0 {
        format!("${v:.2}")
    } else {
        format!("${v:.4}")
    }
}

/// Format a UTC day-start Unix timestamp as `MMM d`.
fn day_label(day_start: u64) -> String {
    let days = day_start / 86400;
    let (_y, m, d) = civil_from_days(days as i64);
    let month = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ][(m - 1) as usize];
    format!("{month} {d}")
}

/// Convert days since epoch to (year, month, day) in the civil calendar.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Escape text for safe SVG inclusion.
fn escape_text(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
