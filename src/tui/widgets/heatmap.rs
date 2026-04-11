use std::collections::HashMap;

use chrono::{Datelike, Duration, NaiveDate, Utc};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::display;
use crate::tui::theme;
use crate::types::PeriodSummary;

// ── Data ─────────────────────────────────────────────────────────────────

/// Per-day data for the contribution heatmap.
#[derive(Debug, Clone)]
pub struct HeatmapDay {
    pub date: NaiveDate,
    pub total_cost: f64,
    pub dominant_provider: String,
}

/// Build heatmap data from daily period summaries.
#[must_use]
pub fn build_heatmap_data(daily_summaries: &[PeriodSummary]) -> Vec<HeatmapDay> {
    daily_summaries
        .iter()
        .map(|ps| {
            // Sum cost by API provider to find the dominant one.
            let mut provider_cost: HashMap<&str, f64> = HashMap::new();
            for mu in &ps.models {
                let provider = display::infer_api_provider(mu.effective_raw_model());
                *provider_cost.entry(provider).or_default() += mu.cost_usd;
            }
            let dominant = provider_cost
                .into_iter()
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .map_or("", |(p, _)| p);
            HeatmapDay {
                date: ps.date,
                total_cost: ps.total_cost,
                dominant_provider: dominant.to_string(),
            }
        })
        .collect()
}

// ── Rendering ────────────────────────────────────────────────────────────

/// Minimum terminal width needed to render the heatmap.
const MIN_WIDTH: u16 = 60;

/// Label column width (for "Mon", "Wed", "Fri" + padding).
const LABEL_COL: u16 = 5;

/// Number of intensity buckets (excluding zero / empty).
const INTENSITY_LEVELS: usize = 4;

/// Render the contribution heatmap into the given area.
pub fn render(frame: &mut Frame, area: Rect, heatmap_data: &[HeatmapDay]) {
    if area.width < MIN_WIDTH || area.height < 10 {
        let msg = Line::from(Span::styled(
            "Terminal too small for heatmap",
            theme::text_dim(),
        ));
        frame.render_widget(msg, area);
        return;
    }

    let today = Utc::now().date_naive();
    // Start 52 full weeks ago on Monday, plus partial current week.
    let start = monday_of_week(today) - Duration::weeks(52);
    let num_weeks = weeks_between(start, today) + 1;

    // Build a lookup map for O(1) date access.
    let day_map: HashMap<NaiveDate, &HeatmapDay> =
        heatmap_data.iter().map(|d| (d.date, d)).collect();

    // Compute intensity thresholds using log scale.
    let max_cost = heatmap_data
        .iter()
        .map(|d| d.total_cost)
        .fold(0.0_f64, f64::max);
    let thresholds = log_thresholds(max_cost);

    // Determine how many week columns we can fit.
    // Each week is 2 chars wide (dot/block + space).
    let available_cols = area.width.saturating_sub(LABEL_COL);
    let cell_width: u16 = 2;
    let max_visible_weeks = (available_cols / cell_width) as usize;
    let visible_weeks = num_weeks.min(max_visible_weeks);
    // Adjust start if we can't fit all weeks.
    let display_start = if num_weeks > visible_weeks {
        start + Duration::weeks((num_weeks - visible_weeks) as i64)
    } else {
        start
    };

    let mut y = area.y;

    // ── Month labels row ─────────────────────────────────────────────
    if y < area.y + area.height {
        let month_line = build_month_labels(display_start, visible_weeks, cell_width);
        let padded = Line::from(
            std::iter::once(Span::styled(" ".repeat(LABEL_COL as usize), theme::text()))
                .chain(month_line)
                .collect::<Vec<_>>(),
        );
        frame.render_widget(padded, Rect::new(area.x, y, area.width, 1));
        y += 1;
    }

    // ── Grid rows (one row per day of week) ──────────────────────────
    // Labels only on Mon (0), Wed (2), Fri (4); others blank.
    let day_labels = ["Mon", "", "Wed", "", "Fri", "", ""];

    for (day_idx, label) in day_labels.iter().enumerate() {
        if y >= area.y + area.height {
            break;
        }

        let mut spans: Vec<Span> = Vec::with_capacity(visible_weeks + 1);

        // Day-of-week label (or blank).
        spans.push(Span::styled(
            format!("{label:<width$}", width = LABEL_COL as usize),
            Style::default().fg(theme::FG).bold(),
        ));

        // One cell per week.
        for week in 0..visible_weeks {
            let date =
                display_start + Duration::weeks(week as i64) + Duration::days(day_idx as i64);

            if date > today {
                // Future: invisible.
                spans.push(Span::styled("  ", Style::default()));
            } else if let Some(day) = day_map.get(&date) {
                let level = intensity_level(day.total_cost, &thresholds);
                let color = theme::provider_color(&day.dominant_provider);
                let dimmed = dim_color(color, level);
                spans.push(Span::styled(
                    "\u{2588}\u{2588}", // solid block ██
                    Style::default().fg(dimmed),
                ));
            } else {
                // Empty day: dim dot.
                spans.push(Span::styled(
                    "\u{2022} ", // bullet •
                    Style::default().fg(theme::BORDER),
                ));
            }
        }

        let line = Line::from(spans);
        frame.render_widget(line, Rect::new(area.x, y, area.width, 1));
        y += 1;
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────

/// Return the Monday of the ISO week containing `date`.
fn monday_of_week(date: NaiveDate) -> NaiveDate {
    let wd = date.weekday().num_days_from_monday(); // Mon=0 .. Sun=6
    date - Duration::days(i64::from(wd))
}

/// Number of weeks between two Mondays.
fn weeks_between(start: NaiveDate, end: NaiveDate) -> usize {
    let days = (end - start).num_days().max(0);
    #[allow(clippy::cast_sign_loss)]
    let w = (days / 7) as usize;
    w
}

/// Compute log-scale thresholds for 4 intensity levels.
fn log_thresholds(max: f64) -> [f64; INTENSITY_LEVELS] {
    if max <= 0.0 {
        return [0.0; INTENSITY_LEVELS];
    }
    // Exponential spacing: each level is ~3x the previous.
    // level 1: > 0, level 2: > max/27, level 3: > max/9, level 4: > max/3
    [0.001, max / 27.0, max / 9.0, max / 3.0]
}

/// Map a cost value to an intensity level (1-4). Returns 0 if cost is zero.
fn intensity_level(cost: f64, thresholds: &[f64; INTENSITY_LEVELS]) -> usize {
    if cost <= 0.0 {
        return 0;
    }
    for (i, &t) in thresholds.iter().enumerate().rev() {
        if cost >= t {
            return i + 1;
        }
    }
    1
}

/// Dim a colour to the given intensity level (1=dimmest, 4=brightest).
fn dim_color(full: ratatui::style::Color, level: usize) -> ratatui::style::Color {
    // Interpolate from SURFACE towards the full colour.
    let t = match level {
        0 => 0.0,
        1 => 0.25,
        2 => 0.50,
        3 => 0.75,
        _ => 1.0,
    };
    theme::lerp_color(theme::SURFACE, full, t)
}

/// Build month label spans positioned above the grid columns.
fn build_month_labels(start: NaiveDate, num_weeks: usize, cell_width: u16) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut last_month: Option<u32> = None;
    let mut col: usize = 0;

    let months = [
        "", "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    for week in 0..num_weeks {
        let week_start = start + Duration::weeks(week as i64);
        let m = week_start.month();

        if last_month != Some(m) {
            last_month = Some(m);
            let label = months[m as usize];
            // Pad from current position to this column.
            let target_col = week * cell_width as usize;
            if target_col > col {
                spans.push(Span::styled(" ".repeat(target_col - col), theme::text()));
                col = target_col;
            }
            spans.push(Span::styled(label.to_string(), theme::text_dim()));
            col += label.len();
        }
    }

    spans
}
