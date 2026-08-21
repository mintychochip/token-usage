//! Static SVG chart generation for README embeds.
//!
//! GitHub READMEs strip JavaScript, so the chart is rendered server-side as
//! inline SVG and embedded via `<img src="chart.svg">`. Bars are stacked by
//! provider, colored from the mintychochip.dev palette.

use crate::summary::UsageSummary;

const WIDTH: u32 = 720;
const HEIGHT: u32 = 280;
const PAD_LEFT: u32 = 56;
const PAD_RIGHT: u32 = 16;
const PAD_TOP: u32 = 30;
const PAD_BOTTOM: u32 = 40;
const PLOT_W: u32 = WIDTH - PAD_LEFT - PAD_RIGHT;
const PLOT_H: u32 = HEIGHT - PAD_TOP - PAD_BOTTOM;

/// mintychochip.dev chart palette, assigned to providers in order of total usage.
const PALETTE: [&str; 8] = [
    "#1E61B8", "#2DC2D2", "#25A3B1", "#983D16", "#921C33", "#7C2489", "#184E95", "#1E848F",
];

const FONT_SANS: &str = "&quot;Geist Sans&quot;, system-ui, -apple-system, BlinkMacSystemFont, &quot;Segoe UI&quot;, sans-serif";
const BG: &str = "#121212";
const GRID: &str = "#2a2a2a";
const TEXT: &str = "#ededed";
const MUTED: &str = "#737373";

/// Render a daily stacked-provider token bar chart as an SVG string.
pub fn chart_svg(summary: &UsageSummary) -> String {
    let days = &summary.days;
    let max_tokens = days
        .iter()
        .map(|d| d.input_tokens)
        .max()
        .unwrap_or(1)
        .max(1);
    let n = days.len().max(1);
    let slot = PLOT_W / n as u32;
    let bar_w = (slot as f32 * 0.62).max(2.0) as u32;

    // Assign palette colors to providers by total usage across all days.
    let mut provider_order: Vec<String> = Vec::new();
    for day in days {
        for p in &day.providers {
            if !provider_order.contains(&p.provider) {
                provider_order.push(p.provider.clone());
            }
        }
    }
    // Order providers by total input tokens descending.
    let mut totals: Vec<(&str, u64)> = provider_order
        .iter()
        .map(|name| {
            let total: u64 = days
                .iter()
                .flat_map(|d| &d.providers)
                .filter(|p| &p.provider == name)
                .map(|p| p.input_tokens)
                .sum();
            (name.as_str(), total)
        })
        .collect();
    totals.sort_by_key(|b| std::cmp::Reverse(b.1));

    let mut bars = String::new();
    let mut labels = String::new();
    let mut legend = String::new();
    for (i, day) in days.iter().enumerate() {
        let x = PAD_LEFT + (i as u32) * slot;
        let cx = x + slot / 2;
        // Stack provider segments bottom-up.
        let mut y = PAD_TOP + PLOT_H;
        for (name, _) in &totals {
            let name: &str = name;
            let Some(seg) = day.providers.iter().find(|p| p.provider == name) else {
                continue;
            };
            let h = ((seg.input_tokens as f64 / max_tokens as f64) * PLOT_H as f64) as u32;
            if h == 0 {
                continue;
            }
            let color = palette_color(name);
            let top = y - h;
            bars.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"1\" fill=\"{}\"/>",
                cx - bar_w / 2,
                top,
                bar_w,
                h,
                color
            ));
            y = top;
        }
        if n <= 31 || i % 5 == 0 {
            labels.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"11\" fill=\"{}\" font-family=\"{}\">{}</text>",
                cx,
                PAD_TOP + PLOT_H + 18,
                MUTED,
                FONT_SANS,
                day_label(day.day)
            ));
        }
    }

    // Legend: top 8 providers with their colors.
    for (idx, (name, _)) in totals.iter().take(8).enumerate() {
        let lx = PAD_LEFT + (idx as u32 % 4) * 170;
        let ly = HEIGHT - 10 - (idx as u32 / 4) * 18;
        let color = palette_color(name);
        legend.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"10\" height=\"10\" rx=\"2\" fill=\"{}\"/>",
            lx,
            ly - 9,
            color
        ));
        legend.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" font-size=\"11\" fill=\"{}\" font-family=\"{}\">{}</text>",
            lx + 14,
            ly,
            MUTED,
            FONT_SANS,
            escape_text(name)
        ));
    }

    let total = summary.input_tokens;
    let total_label = compact(total);
    let baseline = PAD_TOP + PLOT_H;
    let right = WIDTH - PAD_RIGHT;
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}" viewBox="0 0 {WIDTH} {HEIGHT}" role="img" aria-label="toktally token usage">
  <rect width="100%" height="100%" fill="{BG}"/>
  <text x="{PAD_LEFT}" y="20" font-size="14" font-weight="600" fill="{TEXT}" font-family="{FONT_SANS}">toktally · {total_label} tokens in</text>
  <g>{bars}</g>
  <g>{labels}</g>
  <line x1="{PAD_LEFT}" y1="{PAD_TOP}" x2="{PAD_LEFT}" y2="{baseline}" stroke="{GRID}"/>
  <line x1="{PAD_LEFT}" y1="{baseline}" x2="{right}" y2="{baseline}" stroke="{GRID}"/>
  <g>{legend}</g>
</svg>"##
    )
}

/// Pick a palette color for a provider, stable by name.
fn palette_color(provider: &str) -> &'static str {
    // Stable hash so a provider always maps to the same color across renders.
    let mut h: u32 = 2166136261;
    for b in provider.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    PALETTE[(h as usize) % PALETTE.len()]
}

/// Compact a token count to `12.3M` / `456k`.
fn compact(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
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
