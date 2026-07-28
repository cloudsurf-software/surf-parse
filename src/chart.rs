//! Native chart rendering (`::chart` with inline data) — deterministic SVG.
//!
//! Eight chart types are supported, all rendered as pure, dependency-free,
//! server-side SVG: `line`, `area`, `bar`, `stacked-bar`, `scatter`, `pie`,
//! `donut`, `radar`. Each renders axes / gridlines / legend / labels as
//! appropriate, plus an optional title and accessible `<title>` / `aria-label`.
//!
//! DETERMINISM: [`render_svg`] is a pure function of `(ChartType, ChartData,
//! title)`. All geometry is computed from the data with no randomness, no time
//! and no `HashMap` iteration; floats are formatted through [`fnum`] (fixed
//! 2-decimal, trimmed) so output is byte-stable across runs on a given host —
//! consumers pin exact substrings / element counts in tests.

use crate::render_html::escape_html;
use crate::types::{ChartData, ChartType};

/// Deterministic categorical palette (stable order). Used when the renderer has
/// no theme accent to key off of. Index `i % len` per series / slice.
const PALETTE: &[&str] = &[
    "#2563eb", // blue
    "#16a34a", // green
    "#ea580c", // orange
    "#9333ea", // violet
    "#dc2626", // red
    "#0891b2", // cyan
    "#ca8a04", // amber
    "#db2777", // pink
];

const GRID: &str = "#e2e8f0";
const AXIS: &str = "#94a3b8";
const MUTED: &str = "#64748b";

// Shared cartesian canvas geometry.
const W: i64 = 680;
const H: i64 = 380;
const PAD_L: i64 = 60;
const PAD_R: i64 = 24;
const PAD_T: i64 = 44;
const PAD_B: i64 = 64;

/// Render a chart of `chart_type` from inline `data` as deterministic inline
/// SVG. `title` (the block's `title=` attribute) becomes the SVG `<title>` and
/// the on-canvas heading.
pub(crate) fn render_svg(chart_type: ChartType, data: &ChartData, title: Option<&str>) -> String {
    match chart_type {
        ChartType::Line => render_line(data, title, false),
        ChartType::Area => render_line(data, title, true),
        ChartType::Bar => render_bar(data, title, false),
        ChartType::StackedBar => render_bar(data, title, true),
        ChartType::Scatter => render_scatter(data, title),
        ChartType::Pie => render_pie(data, title, false),
        ChartType::Donut => render_pie(data, title, true),
        ChartType::Radar => render_radar(data, title),
    }
}

/// Proportional slice angles (degrees) for a pie/donut from `values`.
///
/// Negative values are clamped to zero. The angles always sum to **exactly**
/// `360.0` — the final slice absorbs any rounding remainder. With no positive
/// total the circle is split evenly.
pub(crate) fn slice_angles(values: &[f64]) -> Vec<f64> {
    let n = values.len();
    if n == 0 {
        return Vec::new();
    }
    let total: f64 = values.iter().map(|v| v.max(0.0)).sum();
    let mut out = Vec::with_capacity(n);
    let mut acc = 0.0;
    for (i, &v) in values.iter().enumerate() {
        if i == n - 1 {
            out.push(360.0 - acc);
        } else {
            let a = if total > 0.0 {
                v.max(0.0) / total * 360.0
            } else {
                360.0 / n as f64
            };
            out.push(a);
            acc += a;
        }
    }
    out
}

// ------------------------------------------------------------------
// Formatting + small numeric helpers
// ------------------------------------------------------------------

/// Deterministic, trimmed number formatting for SVG coordinates / labels.
fn fnum(v: f64) -> String {
    if !v.is_finite() {
        return "0".to_string();
    }
    let s = format!("{v:.2}");
    let t = s.trim_end_matches('0').trim_end_matches('.');
    if t.is_empty() || t == "-0" {
        "0".to_string()
    } else {
        t.to_string()
    }
}

/// Parse a possibly-formatted numeric string (used for scatter x-values).
fn num(s: &str) -> f64 {
    let cleaned: String = s
        .chars()
        .filter(|c| !matches!(c, '$' | '%' | ',' | ' ' | '_'))
        .collect();
    cleaned.parse::<f64>().unwrap_or(0.0)
}

fn color(i: usize) -> &'static str {
    PALETTE[i % PALETTE.len()]
}

// ------------------------------------------------------------------
// SVG scaffolding
// ------------------------------------------------------------------

fn svg_open(w: i64, h: i64, aria: &str, title: Option<&str>) -> String {
    let title_text = title.unwrap_or(aria);
    format!(
        "<svg class=\"surfdoc-chart-svg\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w} {h}\" width=\"{w}\" height=\"{h}\" role=\"img\" aria-label=\"{}\" font-family=\"system-ui, sans-serif\" font-size=\"12\"><title>{}</title>",
        escape_html(aria),
        escape_html(title_text),
    )
}

fn push_title(svg: &mut String, title: Option<&str>) {
    if let Some(t) = title {
        svg.push_str(&format!(
            "<text class=\"surfdoc-chart-title\" x=\"{}\" y=\"24\" text-anchor=\"middle\" font-size=\"15\" font-weight=\"bold\" fill=\"currentColor\">{}</text>",
            W / 2,
            escape_html(t),
        ));
    }
}

/// Horizontal legend centred on the canvas at vertical `y`.
fn push_legend(svg: &mut String, names: &[String], y: i64) {
    let item_w = |n: &str| 18 + n.chars().count() as i64 * 7 + 18;
    let total: i64 = names.iter().map(|n| item_w(n)).sum();
    let mut x = (W - total) / 2;
    for (i, n) in names.iter().enumerate() {
        svg.push_str(&format!(
            "<rect class=\"surfdoc-chart-legend-swatch\" x=\"{x}\" y=\"{}\" width=\"12\" height=\"12\" rx=\"2\" fill=\"{}\"/>",
            y - 10,
            color(i),
        ));
        svg.push_str(&format!(
            "<text class=\"surfdoc-chart-legend-label\" x=\"{}\" y=\"{y}\" font-size=\"11\" fill=\"currentColor\">{}</text>",
            x + 16,
            escape_html(n),
        ));
        x += item_w(n);
    }
}

/// Vertical legend (used by pie/donut/radar) anchored at `(x, y_start)`.
fn push_legend_vertical(svg: &mut String, names: &[String], x: i64, y_start: i64) {
    for (i, n) in names.iter().enumerate() {
        let y = y_start + i as i64 * 22;
        svg.push_str(&format!(
            "<rect class=\"surfdoc-chart-legend-swatch\" x=\"{x}\" y=\"{}\" width=\"12\" height=\"12\" rx=\"2\" fill=\"{}\"/>",
            y,
            color(i),
        ));
        svg.push_str(&format!(
            "<text class=\"surfdoc-chart-legend-label\" x=\"{}\" y=\"{}\" font-size=\"11\" fill=\"currentColor\">{}</text>",
            x + 18,
            y + 11,
            escape_html(n),
        ));
    }
}

/// "Nice" axis bounds + step yielding roughly `target` intervals.
fn nice_ticks(min: f64, max: f64, target: i64) -> (f64, f64, f64) {
    let range = if (max - min).abs() < 1e-9 {
        max.abs().max(1.0)
    } else {
        max - min
    };
    let raw = range / target.max(1) as f64;
    let mag = 10f64.powf(raw.log10().floor());
    let norm = raw / mag;
    let nice = if norm < 1.5 {
        1.0
    } else if norm < 3.0 {
        2.0
    } else if norm < 7.0 {
        5.0
    } else {
        10.0
    };
    let step = nice * mag;
    let lo = (min / step).floor() * step;
    let hi = (max / step).ceil() * step;
    (lo, hi, if step.abs() < 1e-12 { 1.0 } else { step })
}

/// Draw the y gridlines + tick labels and the L-shaped axis. Returns nothing;
/// caller maps values with the same `(lo, hi)` it passed in.
#[allow(clippy::too_many_arguments)]
fn push_y_axis(svg: &mut String, x0: f64, y0: f64, plot_w: f64, plot_h: f64, lo: f64, hi: f64, step: f64) {
    let y_bottom = y0 + plot_h;
    let span = (hi - lo).max(1e-9);
    let count = ((hi - lo) / step).round().max(1.0) as i64;
    for i in 0..=count {
        let t = lo + i as f64 * step;
        let y = y_bottom - (t - lo) / span * plot_h;
        svg.push_str(&format!(
            "<line class=\"surfdoc-chart-grid\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{GRID}\" stroke-width=\"1\"/>",
            fnum(x0),
            fnum(y),
            fnum(x0 + plot_w),
            fnum(y),
        ));
        svg.push_str(&format!(
            "<text class=\"surfdoc-chart-ytick\" x=\"{}\" y=\"{}\" text-anchor=\"end\" font-size=\"10\" fill=\"{MUTED}\">{}</text>",
            fnum(x0 - 8.0),
            fnum(y + 3.0),
            fnum(t),
        ));
    }
    svg.push_str(&format!(
        "<line class=\"surfdoc-chart-axis\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{AXIS}\" stroke-width=\"1\"/>",
        fnum(x0),
        fnum(y0),
        fnum(x0),
        fnum(y_bottom),
    ));
    svg.push_str(&format!(
        "<line class=\"surfdoc-chart-axis\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{AXIS}\" stroke-width=\"1\"/>",
        fnum(x0),
        fnum(y_bottom),
        fnum(x0 + plot_w),
        fnum(y_bottom),
    ));
}

/// Collect series names for legend rendering.
fn series_names(data: &ChartData) -> Vec<String> {
    data.series.iter().map(|s| s.name.clone()).collect()
}

// ------------------------------------------------------------------
// Line / Area
// ------------------------------------------------------------------

fn render_line(data: &ChartData, title: Option<&str>, fill: bool) -> String {
    let aria = if fill { "Area chart" } else { "Line chart" };
    let x0 = PAD_L as f64;
    let y0 = PAD_T as f64;
    let plot_w = (W - PAD_L - PAD_R) as f64;
    let plot_h = (H - PAD_T - PAD_B) as f64;
    let y_bottom = y0 + plot_h;
    let n = data.categories.len();

    let mut svg = svg_open(W, H, aria, title);
    push_title(&mut svg, title);
    if n == 0 {
        svg.push_str("</svg>");
        return svg;
    }

    let mut dmin = 0.0f64;
    let mut dmax = 0.0f64;
    for s in &data.series {
        for &v in &s.values {
            dmin = dmin.min(v);
            dmax = dmax.max(v);
        }
    }
    if dmax <= dmin {
        dmax = dmin + 1.0;
    }
    let (lo, hi, step) = nice_ticks(dmin, dmax, 5);
    let span = (hi - lo).max(1e-9);
    push_y_axis(&mut svg, x0, y0, plot_w, plot_h, lo, hi, step);

    let x_at = |i: usize| -> f64 {
        if n == 1 {
            x0 + plot_w / 2.0
        } else {
            x0 + i as f64 / (n - 1) as f64 * plot_w
        }
    };
    let y_at = |v: f64| -> f64 { y_bottom - (v - lo) / span * plot_h };

    // x category labels
    for (i, cat) in data.categories.iter().enumerate() {
        svg.push_str(&format!(
            "<text class=\"surfdoc-chart-xtick\" x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"10\" fill=\"{MUTED}\">{}</text>",
            fnum(x_at(i)),
            fnum(y_bottom + 16.0),
            escape_html(cat),
        ));
    }

    for (si, s) in data.series.iter().enumerate() {
        let col = color(si);
        let pts: Vec<(f64, f64)> = s
            .values
            .iter()
            .enumerate()
            .map(|(i, &v)| (x_at(i), y_at(v)))
            .collect();
        let pts_str = pts
            .iter()
            .map(|(x, y)| format!("{},{}", fnum(*x), fnum(*y)))
            .collect::<Vec<_>>()
            .join(" ");
        if fill && !pts.is_empty() {
            let first_x = pts[0].0;
            let last_x = pts[pts.len() - 1].0;
            svg.push_str(&format!(
                "<polygon class=\"surfdoc-chart-area\" points=\"{} {},{} {},{}\" fill=\"{col}\" fill-opacity=\"0.15\" stroke=\"none\"/>",
                pts_str,
                fnum(last_x),
                fnum(y_bottom),
                fnum(first_x),
                fnum(y_bottom),
            ));
        }
        svg.push_str(&format!(
            "<polyline class=\"surfdoc-chart-line\" points=\"{pts_str}\" fill=\"none\" stroke=\"{col}\" stroke-width=\"2\" stroke-linejoin=\"round\"/>",
        ));
        for (x, y) in &pts {
            svg.push_str(&format!(
                "<circle class=\"surfdoc-chart-dot\" cx=\"{}\" cy=\"{}\" r=\"2.5\" fill=\"{col}\"/>",
                fnum(*x),
                fnum(*y),
            ));
        }
    }

    if data.series.len() > 1 {
        push_legend(&mut svg, &series_names(data), H - 14);
    }
    svg.push_str("</svg>");
    svg
}

// ------------------------------------------------------------------
// Bar / Stacked bar
// ------------------------------------------------------------------

fn render_bar(data: &ChartData, title: Option<&str>, stacked: bool) -> String {
    let aria = if stacked { "Stacked bar chart" } else { "Bar chart" };
    let x0 = PAD_L as f64;
    let y0 = PAD_T as f64;
    let plot_w = (W - PAD_L - PAD_R) as f64;
    let plot_h = (H - PAD_T - PAD_B) as f64;
    let y_bottom = y0 + plot_h;
    let n = data.categories.len();
    let m = data.series.len();

    let mut svg = svg_open(W, H, aria, title);
    push_title(&mut svg, title);
    if n == 0 || m == 0 {
        svg.push_str("</svg>");
        return svg;
    }

    let dmax = if stacked {
        (0..n)
            .map(|i| {
                data.series
                    .iter()
                    .map(|s| s.values.get(i).copied().unwrap_or(0.0).max(0.0))
                    .sum::<f64>()
            })
            .fold(0.0f64, f64::max)
    } else {
        data.series
            .iter()
            .flat_map(|s| s.values.iter().copied())
            .fold(0.0f64, f64::max)
    };
    let dmax = if dmax <= 0.0 { 1.0 } else { dmax };
    let (lo, hi, step) = nice_ticks(0.0, dmax, 5);
    let span = (hi - lo).max(1e-9);
    push_y_axis(&mut svg, x0, y0, plot_w, plot_h, lo, hi, step);

    let slot = plot_w / n as f64;
    let h_of = |v: f64| -> f64 { v.max(0.0) / span * plot_h };

    for (i, cat) in data.categories.iter().enumerate() {
        let slot_left = x0 + i as f64 * slot;
        // x label
        svg.push_str(&format!(
            "<text class=\"surfdoc-chart-xtick\" x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"10\" fill=\"{MUTED}\">{}</text>",
            fnum(slot_left + slot / 2.0),
            fnum(y_bottom + 16.0),
            escape_html(cat),
        ));

        if stacked {
            let bw = slot * 0.6;
            let bx = slot_left + (slot - bw) / 2.0;
            let mut top = y_bottom;
            for (si, s) in data.series.iter().enumerate() {
                let v = s.values.get(i).copied().unwrap_or(0.0).max(0.0);
                let bh = h_of(v);
                top -= bh;
                svg.push_str(&format!(
                    "<rect class=\"surfdoc-chart-bar\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
                    fnum(bx),
                    fnum(top),
                    fnum(bw),
                    fnum(bh),
                    color(si),
                ));
                if bh >= 14.0 {
                    svg.push_str(&format!(
                        "<text class=\"surfdoc-chart-value\" x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"9\" fill=\"#ffffff\">{}</text>",
                        fnum(bx + bw / 2.0),
                        fnum(top + bh / 2.0 + 3.0),
                        fnum(v),
                    ));
                }
            }
        } else {
            let group_w = slot * 0.7;
            let bw = group_w / m as f64;
            let group_left = slot_left + (slot - group_w) / 2.0;
            for (si, s) in data.series.iter().enumerate() {
                let v = s.values.get(i).copied().unwrap_or(0.0).max(0.0);
                let bh = h_of(v);
                let bx = group_left + si as f64 * bw;
                let by = y_bottom - bh;
                svg.push_str(&format!(
                    "<rect class=\"surfdoc-chart-bar\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
                    fnum(bx),
                    fnum(by),
                    fnum(bw.max(1.0)),
                    fnum(bh),
                    color(si),
                ));
                svg.push_str(&format!(
                    "<text class=\"surfdoc-chart-value\" x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"9\" fill=\"{MUTED}\">{}</text>",
                    fnum(bx + bw / 2.0),
                    fnum(by - 3.0),
                    fnum(v),
                ));
            }
        }
    }

    if m > 1 {
        push_legend(&mut svg, &series_names(data), H - 14);
    }
    svg.push_str("</svg>");
    svg
}

// ------------------------------------------------------------------
// Scatter
// ------------------------------------------------------------------

fn render_scatter(data: &ChartData, title: Option<&str>) -> String {
    let x0 = PAD_L as f64;
    let y0 = PAD_T as f64;
    let plot_w = (W - PAD_L - PAD_R) as f64;
    let plot_h = (H - PAD_T - PAD_B) as f64;
    let y_bottom = y0 + plot_h;

    let mut svg = svg_open(W, H, "Scatter chart", title);
    push_title(&mut svg, title);

    let xs: Vec<f64> = data.categories.iter().map(|c| num(c)).collect();
    if xs.is_empty() {
        svg.push_str("</svg>");
        return svg;
    }

    let xmin = xs.iter().copied().fold(f64::INFINITY, f64::min);
    let xmax = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let mut ymin = f64::INFINITY;
    let mut ymax = f64::NEG_INFINITY;
    for s in &data.series {
        for &v in &s.values {
            ymin = ymin.min(v);
            ymax = ymax.max(v);
        }
    }
    if !ymin.is_finite() {
        ymin = 0.0;
        ymax = 1.0;
    }
    let (xlo, xhi, xstep) = nice_ticks(xmin.min(xmax), xmax.max(xmin), 5);
    let (ylo, yhi, ystep) = nice_ticks(ymin.min(ymax), ymax.max(ymin), 5);
    let xspan = (xhi - xlo).max(1e-9);
    let yspan = (yhi - ylo).max(1e-9);
    push_y_axis(&mut svg, x0, y0, plot_w, plot_h, ylo, yhi, ystep);

    // x ticks
    let xcount = ((xhi - xlo) / xstep).round().max(1.0) as i64;
    for i in 0..=xcount {
        let t = xlo + i as f64 * xstep;
        let x = x0 + (t - xlo) / xspan * plot_w;
        svg.push_str(&format!(
            "<text class=\"surfdoc-chart-xtick\" x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"10\" fill=\"{MUTED}\">{}</text>",
            fnum(x),
            fnum(y_bottom + 16.0),
            fnum(t),
        ));
    }

    let x_at = |v: f64| x0 + (v - xlo) / xspan * plot_w;
    let y_at = |v: f64| y_bottom - (v - ylo) / yspan * plot_h;

    for (si, s) in data.series.iter().enumerate() {
        let col = color(si);
        for (i, &v) in s.values.iter().enumerate() {
            let x = x_at(*xs.get(i).unwrap_or(&0.0));
            let y = y_at(v);
            svg.push_str(&format!(
                "<circle class=\"surfdoc-chart-point\" cx=\"{}\" cy=\"{}\" r=\"4\" fill=\"{col}\" fill-opacity=\"0.75\"/>",
                fnum(x),
                fnum(y),
            ));
        }
    }

    if data.series.len() > 1 {
        push_legend(&mut svg, &series_names(data), H - 14);
    }
    svg.push_str("</svg>");
    svg
}

// ------------------------------------------------------------------
// Pie / Donut
// ------------------------------------------------------------------

fn render_pie(data: &ChartData, title: Option<&str>, donut: bool) -> String {
    let aria = if donut { "Donut chart" } else { "Pie chart" };
    let cx = 220.0f64;
    let cy = 210.0f64;
    let r = 130.0f64;
    let inner = if donut { r * 0.55 } else { 0.0 };

    let mut svg = svg_open(W, H, aria, title);
    push_title(&mut svg, title);

    let values: Vec<f64> = data
        .series
        .first()
        .map(|s| s.values.iter().map(|v| v.max(0.0)).collect())
        .unwrap_or_default();
    if values.is_empty() {
        svg.push_str("</svg>");
        return svg;
    }
    let angles = slice_angles(&values);

    let mut acc = -90.0f64; // start at 12 o'clock
    for (i, &ang) in angles.iter().enumerate() {
        let a0 = acc.to_radians();
        let a1 = (acc + ang).to_radians();
        let large = if ang > 180.0 { 1 } else { 0 };
        let (px0, py0) = (cx + r * a0.cos(), cy + r * a0.sin());
        let (px1, py1) = (cx + r * a1.cos(), cy + r * a1.sin());
        let path = if donut {
            let (ix0, iy0) = (cx + inner * a0.cos(), cy + inner * a0.sin());
            let (ix1, iy1) = (cx + inner * a1.cos(), cy + inner * a1.sin());
            format!(
                "M {} {} A {r} {r} 0 {large} 1 {} {} L {} {} A {inner} {inner} 0 {large} 0 {} {} Z",
                fnum(px0),
                fnum(py0),
                fnum(px1),
                fnum(py1),
                fnum(ix1),
                fnum(iy1),
                fnum(ix0),
                fnum(iy0),
            )
        } else {
            format!(
                "M {} {} L {} {} A {r} {r} 0 {large} 1 {} {} Z",
                fnum(cx),
                fnum(cy),
                fnum(px0),
                fnum(py0),
                fnum(px1),
                fnum(py1),
            )
        };
        svg.push_str(&format!(
            "<path class=\"surfdoc-chart-slice\" d=\"{path}\" fill=\"{}\" stroke=\"#ffffff\" stroke-width=\"1\"/>",
            color(i),
        ));
        // percentage label at the slice mid-angle
        let pct = ang / 360.0 * 100.0;
        if pct >= 4.0 {
            let mid = (acc + ang / 2.0).to_radians();
            let lr = if donut { (r + inner) / 2.0 } else { r * 0.62 };
            let lx = cx + lr * mid.cos();
            let ly = cy + lr * mid.sin();
            svg.push_str(&format!(
                "<text class=\"surfdoc-chart-pct\" x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"11\" fill=\"#ffffff\">{}%</text>",
                fnum(lx),
                fnum(ly + 3.0),
                format!("{:.0}", pct),
            ));
        }
        acc += ang;
    }

    push_legend_vertical(&mut svg, &data.categories, 430, 80);
    svg.push_str("</svg>");
    svg
}

// ------------------------------------------------------------------
// Radar
// ------------------------------------------------------------------

fn render_radar(data: &ChartData, title: Option<&str>) -> String {
    let cx = 230.0f64;
    let cy = 210.0f64;
    let r = 130.0f64;
    let k = data.categories.len();

    let mut svg = svg_open(W, H, "Radar chart", title);
    push_title(&mut svg, title);
    if k == 0 {
        svg.push_str("</svg>");
        return svg;
    }

    let dmax = data
        .series
        .iter()
        .flat_map(|s| s.values.iter().copied())
        .fold(0.0f64, f64::max);
    let dmax = if dmax <= 0.0 { 1.0 } else { dmax };

    let axis_angle = |a: usize| -> f64 { (-90.0 + a as f64 * 360.0 / k as f64).to_radians() };
    let point = |a: usize, frac: f64| -> (f64, f64) {
        let ang = axis_angle(a);
        (cx + r * frac * ang.cos(), cy + r * frac * ang.sin())
    };

    // concentric grid rings (4 levels) as polygons
    for level in 1..=4 {
        let frac = level as f64 / 4.0;
        let ring: Vec<String> = (0..k)
            .map(|a| {
                let (x, y) = point(a, frac);
                format!("{},{}", fnum(x), fnum(y))
            })
            .collect();
        svg.push_str(&format!(
            "<polygon class=\"surfdoc-chart-grid\" points=\"{}\" fill=\"none\" stroke=\"{GRID}\" stroke-width=\"1\"/>",
            ring.join(" "),
        ));
    }

    // spokes + axis labels
    for a in 0..k {
        let (x, y) = point(a, 1.0);
        svg.push_str(&format!(
            "<line class=\"surfdoc-chart-axis\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{AXIS}\" stroke-width=\"1\"/>",
            fnum(cx),
            fnum(cy),
            fnum(x),
            fnum(y),
        ));
        let (lx, ly) = point(a, 1.12);
        let anchor = if lx > cx + 1.0 {
            "start"
        } else if lx < cx - 1.0 {
            "end"
        } else {
            "middle"
        };
        svg.push_str(&format!(
            "<text class=\"surfdoc-chart-xtick\" x=\"{}\" y=\"{}\" text-anchor=\"{anchor}\" font-size=\"10\" fill=\"{MUTED}\">{}</text>",
            fnum(lx),
            fnum(ly + 3.0),
            escape_html(&data.categories[a]),
        ));
    }

    for (si, s) in data.series.iter().enumerate() {
        let col = color(si);
        let poly: Vec<String> = (0..k)
            .map(|a| {
                let v = s.values.get(a).copied().unwrap_or(0.0).max(0.0);
                let (x, y) = point(a, v / dmax);
                format!("{},{}", fnum(x), fnum(y))
            })
            .collect();
        svg.push_str(&format!(
            "<polygon class=\"surfdoc-chart-radar\" points=\"{}\" fill=\"{col}\" fill-opacity=\"0.15\" stroke=\"{col}\" stroke-width=\"2\"/>",
            poly.join(" "),
        ));
    }

    push_legend_vertical(&mut svg, &series_names(data), 430, 80);
    svg.push_str("</svg>");
    svg
}

// ------------------------------------------------------------------
// Tests
// ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ChartSeries;

    fn data() -> ChartData {
        ChartData {
            categories: vec!["Jan".into(), "Feb".into(), "Mar".into(), "Apr".into()],
            series: vec![
                ChartSeries {
                    name: "Product A".into(),
                    values: vec![120.0, 150.0, 170.0, 140.0],
                },
                ChartSeries {
                    name: "Product B".into(),
                    values: vec![80.0, 95.0, 110.0, 130.0],
                },
            ],
        }
    }

    fn count(hay: &str, needle: &str) -> usize {
        hay.matches(needle).count()
    }

    const ALL: &[ChartType] = &[
        ChartType::Line,
        ChartType::Area,
        ChartType::Bar,
        ChartType::StackedBar,
        ChartType::Scatter,
        ChartType::Pie,
        ChartType::Donut,
        ChartType::Radar,
    ];

    #[test]
    fn every_type_is_deterministic() {
        let d = data();
        for &t in ALL {
            let a = render_svg(t, &d, Some("Title"));
            let b = render_svg(t, &d, Some("Title"));
            assert_eq!(a, b, "non-deterministic SVG for {t:?}");
            assert!(a.starts_with("<svg"), "no svg root for {t:?}");
            assert!(a.ends_with("</svg>"), "unterminated svg for {t:?}");
            assert!(a.contains("role=\"img\""), "no aria role for {t:?}");
            assert!(a.contains("<title>"), "no title element for {t:?}");
        }
    }

    #[test]
    fn bar_count_matches_data() {
        let d = data();
        // grouped: n categories * m series = 4 * 2 = 8
        assert_eq!(count(&render_svg(ChartType::Bar, &d, None), "surfdoc-chart-bar"), 8);
        // stacked: same number of segments
        assert_eq!(
            count(&render_svg(ChartType::StackedBar, &d, None), "surfdoc-chart-bar"),
            8
        );
    }

    #[test]
    fn slice_count_matches_categories() {
        let d = data();
        // first series has 4 values -> 4 slices
        assert_eq!(count(&render_svg(ChartType::Pie, &d, None), "surfdoc-chart-slice"), 4);
        assert_eq!(count(&render_svg(ChartType::Donut, &d, None), "surfdoc-chart-slice"), 4);
    }

    #[test]
    fn scatter_point_count_matches_data() {
        let d = data();
        // 2 series * 4 points = 8
        assert_eq!(count(&render_svg(ChartType::Scatter, &d, None), "surfdoc-chart-point"), 8);
    }

    #[test]
    fn radar_polygon_per_series() {
        let d = data();
        assert_eq!(count(&render_svg(ChartType::Radar, &d, None), "surfdoc-chart-radar"), 2);
    }

    #[test]
    fn line_has_polyline_per_series() {
        let d = data();
        assert_eq!(count(&render_svg(ChartType::Line, &d, None), "surfdoc-chart-line"), 2);
        // area adds a filled polygon per series
        assert_eq!(count(&render_svg(ChartType::Area, &d, None), "surfdoc-chart-area"), 2);
    }

    #[test]
    fn slice_angles_sum_to_360() {
        for vals in [
            vec![1.0, 1.0, 1.0],
            vec![120.0, 150.0, 170.0, 140.0],
            vec![0.0, 0.0],
            vec![5.0],
            vec![3.0, 0.0, 7.0, 1.0, 9.0],
        ] {
            let a = slice_angles(&vals);
            assert_eq!(a.len(), vals.len());
            let sum: f64 = a.iter().sum();
            assert!((sum - 360.0).abs() < 1e-6, "angles {a:?} sum {sum}");
        }
        assert!(slice_angles(&[]).is_empty());
    }

    #[test]
    fn title_is_rendered_and_escaped() {
        let d = data();
        let svg = render_svg(ChartType::Line, &d, Some("R&D <growth>"));
        assert!(svg.contains("surfdoc-chart-title"));
        assert!(svg.contains("R&amp;D &lt;growth&gt;"));
    }

    #[test]
    fn multi_series_has_legend() {
        let d = data();
        let svg = render_svg(ChartType::Bar, &d, None);
        assert!(svg.contains("surfdoc-chart-legend-label"));
        assert!(svg.contains("Product A"));
        assert!(svg.contains("Product B"));
    }
}
