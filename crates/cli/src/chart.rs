//! Static SVG chart generation for README embeds.
//!
//! GitHub READMEs strip JavaScript, so the chart is rendered server-side as
//! inline SVG and embedded via `<img src="chart.svg">`.

use crate::summary::UsageSummary;

const WIDTH: u32 = 720;
const HEIGHT: u32 = 260;
const PAD_LEFT: u32 = 56;
const PAD_RIGHT: u32 = 16;
const PAD_TOP: u32 = 24;
const PAD_BOTTOM: u32 = 36;
const PLOT_W: u32 = WIDTH - PAD_LEFT - PAD_RIGHT;
const PLOT_H: u32 = HEIGHT - PAD_TOP - PAD_BOTTOM;

/// Render a daily token bar chart as an SVG string.
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

    let mut bars = String::new();
    let mut labels = String::new();
    for (i, day) in days.iter().enumerate() {
        let x = PAD_LEFT + (i as u32) * slot;
        let h = ((day.input_tokens as f64 / max_tokens as f64) * PLOT_H as f64) as u32;
        let y = PAD_TOP + PLOT_H - h;
        let cx = x + slot / 2;
        bars.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"2\" fill=\"#4f8ef7\"/>",
            cx - bar_w / 2,
            y,
            bar_w,
            h
        ));
        if n <= 31 || i % 5 == 0 {
            labels.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"11\" fill=\"#8b949e\">{}</text>",
                cx,
                PAD_TOP + PLOT_H + 18,
                day_label(day.day)
            ));
        }
    }

    let total = summary.input_tokens;
    let total_label = compact(total);
    let baseline = PAD_TOP + PLOT_H;
    let right = WIDTH - PAD_RIGHT;
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}" viewBox="0 0 {WIDTH} {HEIGHT}" role="img" aria-label="toktally token usage">
  <rect width="100%" height="100%" fill="#0d1117"/>
  <text x="{PAD_LEFT}" y="18" font-size="14" font-weight="600" fill="#e6edf3">toktally · {total_label} tokens in</text>
  <g>{bars}</g>
  <g>{labels}</g>
  <line x1="{PAD_LEFT}" y1="{PAD_TOP}" x2="{PAD_LEFT}" y2="{baseline}" stroke="#30363d"/>
  <line x1="{PAD_LEFT}" y1="{baseline}" x2="{right}" y2="{baseline}" stroke="#30363d"/>
</svg>"##
    )
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
