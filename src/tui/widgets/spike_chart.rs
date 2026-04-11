use std::collections::HashMap;

use chrono::{DateTime, Timelike, Utc};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::display;
use crate::tui::theme;
use crate::types::Record;

// ── Data ─────────────────────────────────────────────────────────────────

/// Per-provider token breakdown within a bucket.
#[derive(Debug, Clone)]
pub struct ProviderSlice {
    pub provider: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// One time bucket for the spike chart.
#[derive(Debug, Clone)]
pub struct SpikeChartBucket {
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// Provider slices sorted by total tokens descending (largest first).
    pub providers: Vec<ProviderSlice>,
}

/// Result of building spike chart data, including the most recent record timestamp.
pub struct SpikeChartResult {
    pub buckets: Vec<SpikeChartBucket>,
    /// Timestamp of the most recent record, if any.
    pub most_recent: Option<DateTime<Utc>>,
}

/// Build spike chart data from records, bucketed into `bucket_secs`-second intervals for today.
#[must_use]
pub fn build_spike_data(records: &[Record], bucket_secs: u32) -> SpikeChartResult {
    let bucket_secs = bucket_secs.max(1);
    let now = Utc::now();
    let today = now.date_naive();
    let total_seconds = now.hour() * 3600 + now.minute() * 60 + now.second();
    let current_slot = (total_seconds / bucket_secs) as usize;
    let num_slots = current_slot + 1;

    let mut input_data = vec![0u64; num_slots];
    let mut output_data = vec![0u64; num_slots];
    let mut provider_input: Vec<HashMap<&str, u64>> = vec![HashMap::new(); num_slots];
    let mut provider_output: Vec<HashMap<&str, u64>> = vec![HashMap::new(); num_slots];
    let mut most_recent: Option<&Record> = None;

    for record in records {
        if record.timestamp.date_naive() != today {
            continue;
        }
        if most_recent.is_none_or(|r| record.timestamp > r.timestamp) {
            most_recent = Some(record);
        }
        let rs = record.timestamp.hour() * 3600
            + record.timestamp.minute() * 60
            + record.timestamp.second();
        let slot = (rs / bucket_secs) as usize;
        if slot >= num_slots {
            continue;
        }
        // Cost-weighted input: weight each token class by its relative cost
        // so the visual height reflects spending, not raw token count.
        //   input_tokens       → 1.0x  (base input price)
        //   cache_read_tokens  → 0.1x  (cache hit discount)
        //   cache_creation     → 1.25x (cache write premium)
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let weighted_input = record.input_tokens
            + (record.cache_read_tokens as f64 * 0.1) as u64
            + (record.cache_creation_tokens as f64 * 1.25) as u64;
        let output = record.output_tokens + record.thinking_tokens;

        input_data[slot] += weighted_input;
        output_data[slot] += output;

        let provider = display::infer_api_provider(record.model.as_deref().unwrap_or(""));
        *provider_input[slot].entry(provider).or_default() += weighted_input;
        *provider_output[slot].entry(provider).or_default() += output;
    }

    let buckets = (0..num_slots)
        .map(|i| {
            let mut slices: Vec<ProviderSlice> = provider_input[i]
                .keys()
                .map(|&p| ProviderSlice {
                    provider: p.to_string(),
                    input_tokens: provider_input[i].get(p).copied().unwrap_or(0),
                    output_tokens: provider_output[i].get(p).copied().unwrap_or(0),
                })
                .collect();
            slices.sort_by(|a, b| {
                let ta = a.input_tokens + a.output_tokens;
                let tb = b.input_tokens + b.output_tokens;
                tb.cmp(&ta)
            });
            SpikeChartBucket {
                input_tokens: input_data[i],
                output_tokens: output_data[i],
                providers: slices,
            }
        })
        .collect();

    SpikeChartResult {
        buckets,
        most_recent: most_recent.map(|r| r.timestamp),
    }
}

// ── Braille helpers ──────────────────────────────────────────────────────

const BRAILLE_OFFSET: u32 = 0x2800;

/// Dot-position bits for a 2-column x 4-row braille cell.
/// `DOT_BITS[col][row]` gives the bit to OR into the character.
const DOT_BITS: [[u8; 4]; 2] = [
    [0x01, 0x02, 0x04, 0x40], // col 0: bits 0,1,2,6
    [0x08, 0x10, 0x20, 0x80], // col 1: bits 3,4,5,7
];

#[must_use]
fn braille_char(dots: u8) -> char {
    char::from_u32(BRAILLE_OFFSET + u32::from(dots)).unwrap_or(' ')
}

// ── Rendering ────────────────────────────────────────────────────────────

const MIN_WIDTH: u16 = 20;
const MIN_HEIGHT: u16 = 5;
const LABEL_COL: u16 = 4;
/// Minimum token scale — spikes need at least this many tokens to reach full height.
const MIN_SCALE: u64 = 50_000;

/// Render the spike chart into the given area.
///
/// `age_secs` is the number of seconds since the most recent record.  When
/// recent (< 5 min), a brightness "heat" glow is applied to the trailing
/// columns of actual spike data — bright when fresh, fading as the session
/// goes stale.
#[allow(clippy::too_many_lines)]
pub fn render(
    frame: &mut Frame,
    area: Rect,
    data: &[SpikeChartBucket],
    age_secs: f64,
) {
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        let msg = Line::from(Span::styled(
            "Terminal too small for spike chart",
            theme::text_dim(),
        ));
        frame.render_widget(msg, area);
        return;
    }

    let has_data = data
        .iter()
        .any(|b| b.input_tokens > 0 || b.output_tokens > 0);
    if !has_data {
        let msg = Line::from(Span::styled("No token data for today", theme::text_dim()));
        frame.render_widget(msg, area);
        return;
    }

    let chart_w = (area.width.saturating_sub(LABEL_COL)) as usize;
    let chart_h = area.height as usize;
    let pixel_w = chart_w * 2;
    let pixel_h = chart_h * 4;
    let baseline = pixel_h / 2;

    let visible = if data.len() > pixel_w {
        &data[data.len() - pixel_w..]
    } else {
        data
    };

    let max_out = visible
        .iter()
        .map(|b| b.output_tokens)
        .max()
        .unwrap_or(0)
        .max(MIN_SCALE);
    let max_in = visible
        .iter()
        .map(|b| b.input_tokens)
        .max()
        .unwrap_or(0)
        .max(MIN_SCALE);
    let half_above = baseline;
    let half_below = pixel_h - baseline;

    let mut grid = build_pixel_grid(
        visible, pixel_w, pixel_h, baseline, max_out, max_in, half_above, half_below,
    );

    paint_heat(&mut grid, pixel_w, pixel_h, age_secs);

    render_braille(
        frame, area, &grid, chart_w, chart_h, pixel_w, pixel_h, baseline,
    );
}

/// Three-band spike gradient — `t` runs 0 (base) → 1 (tip).
///
/// The bright color is "earned": it only appears at the very tip of a
/// nearly-full spike. Combined with `paint_segment`'s `local_t * fill_ratio`,
/// this means only spikes that fill ≥95% of their available half-height
/// reach the bright band at all, and only the topmost pixel of a
/// `fill_ratio ≈ 1.0` spike burns pure white.
///
///   [0.00, 0.70]  dim → mid    — most of the spike builds up the provider color
///   [0.70, 0.95]  hold at mid  — saturated provider-color band
///   [0.95, 1.00]  mid → bright — only the tip of a near-max spike
fn spike_gradient(dim: Color, mid: Color, bright: Color, t: f64) -> Color {
    if t < 0.70 {
        theme::lerp_color(dim, mid, t / 0.70)
    } else if t < 0.95 {
        mid
    } else {
        theme::lerp_color(mid, bright, (t - 0.95) / 0.05)
    }
}

/// Paint a vertical segment of the spike in a single provider's color.
///
/// `fill_ratio` (0..1) is how much of the available half-height this spike
/// actually fills.  The gradient's peak brightness is capped to this ratio
/// so that short spikes stay in the dim-to-mid range instead of jumping to white.
#[allow(clippy::too_many_arguments)]
fn paint_segment(
    grid: &mut [Vec<Option<Color>>],
    cx: usize,
    pixel_w: usize,
    pixel_h: usize,
    start_dy: usize,
    end_dy: usize,
    total_h: usize,
    color: Color,
    dim_color: Color,
    bright: Color,
    go_up: bool,
    baseline: usize,
    fill_ratio: f64,
) {
    let max_t = fill_ratio.clamp(0.0, 1.0);
    for dy in start_dy..end_dy {
        let py = if go_up {
            baseline.saturating_sub(1 + dy)
        } else {
            baseline + dy
        };
        if py >= pixel_h {
            continue;
        }
        let local_t = if total_h <= 1 {
            1.0
        } else {
            (dy as f64) / (total_h as f64 - 1.0)
        };
        let c = spike_gradient(dim_color, color, bright, local_t * max_t);
        if cx < pixel_w {
            grid[py][cx] = Some(c);
        }
    }
}

/// Plot spike data into a pixel grid of `Option<Color>` values.
/// Each spike is split into colored segments by provider proportion.
#[allow(clippy::too_many_arguments)]
fn build_pixel_grid(
    visible: &[SpikeChartBucket],
    pixel_w: usize,
    pixel_h: usize,
    baseline: usize,
    max_out: u64,
    max_in: u64,
    half_above: usize,
    half_below: usize,
) -> Vec<Vec<Option<Color>>> {
    let mut grid: Vec<Vec<Option<Color>>> = vec![vec![None; pixel_w]; pixel_h];
    let col_offset = pixel_w.saturating_sub(visible.len());
    let bright = Color::Rgb(255, 255, 255);

    for (i, bucket) in visible.iter().enumerate() {
        let cx = col_offset + i;

        // Paint both directions: output above baseline, input below.
        for &(tokens, max_tokens, half_h, go_up) in &[
            (bucket.output_tokens, max_out, half_above, true),
            (bucket.input_tokens, max_in, half_below, false),
        ] {
            if tokens == 0 {
                continue;
            }
            let ratio = tokens as f64 / max_tokens as f64;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let total_h = (ratio * half_h as f64).ceil() as usize;
            let total_h = total_h.max(1).min(half_h);

            let mut dy_cursor = 0usize;
            for slice in &bucket.providers {
                let slice_tokens = if go_up {
                    slice.output_tokens
                } else {
                    slice.input_tokens
                };
                if slice_tokens == 0 || dy_cursor >= total_h {
                    continue;
                }
                let frac = slice_tokens as f64 / tokens as f64;
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let seg_h = (frac * total_h as f64).round().max(1.0) as usize;
                let end_dy = (dy_cursor + seg_h).min(total_h);

                let color = theme::provider_color(&slice.provider);
                let dim_color = theme::lerp_color(color, theme::BORDER, 0.75);
                paint_segment(
                    &mut grid, cx, pixel_w, pixel_h, dy_cursor, end_dy, total_h, color, dim_color,
                    bright, go_up, baseline, ratio,
                );
                dy_cursor = end_dy;
            }
        }
    }

    grid
}

/// Encode the pixel grid into braille characters and render them.
#[allow(clippy::too_many_arguments)]
fn render_braille(
    frame: &mut Frame,
    area: Rect,
    grid: &[Vec<Option<Color>>],
    chart_w: usize,
    chart_h: usize,
    pixel_w: usize,
    pixel_h: usize,
    baseline: usize,
) {
    let mid_row = chart_h / 2;
    // Vertically center OUT above the baseline and IN below it.
    let out_label_row = mid_row / 2;
    let in_label_row = (mid_row + chart_h) / 2;

    for row in 0..chart_h {
        let row_top = row * 4;
        let mut spans: Vec<Span> = Vec::with_capacity(chart_w + 1);

        let label = if row == out_label_row {
            "OUT "
        } else if row == in_label_row {
            " IN "
        } else {
            "    "
        };
        spans.push(Span::styled(label, theme::text_dim()));

        for col in 0..chart_w {
            let col_left = col * 2;
            let mut dots: u8 = 0;
            let mut cell_color: Option<Color> = None;
            let mut has_dots = false;

            for (dc, col_bits) in DOT_BITS.iter().enumerate() {
                for (dr, &bit) in col_bits.iter().enumerate() {
                    let py = row_top + dr;
                    let px = col_left + dc;
                    if py < pixel_h && px < pixel_w {
                        if let Some(c) = grid[py][px] {
                            dots |= bit;
                            cell_color = Some(c);
                            has_dots = true;
                        }
                    }
                }
            }

            if has_dots {
                let color = cell_color.unwrap_or(theme::BORDER);
                spans.push(Span::styled(
                    braille_char(dots).to_string(),
                    Style::default().fg(color),
                ));
            } else {
                let baseline_in_cell = baseline >= row_top && baseline < row_top + 4;
                if baseline_in_cell {
                    let br = baseline - row_top;
                    let bdots = DOT_BITS[0][br] | DOT_BITS[1][br];
                    spans.push(Span::styled(
                        braille_char(bdots).to_string(),
                        Style::default().fg(theme::BORDER),
                    ));
                } else {
                    spans.push(Span::raw(" "));
                }
            }
        }

        let line = Line::from(spans);
        frame.render_widget(line, Rect::new(area.x, area.y + row as u16, area.width, 1));
    }
}

/// Brighten trailing pixel columns that contain spike data to show recency.
///
/// The glow covers the rightmost columns (up to `HEAT_COLS`) and fades both
/// with distance from the trailing edge and with `age_secs`.  Only pixels
/// that already have color (i.e. actual spike data) are affected — empty
/// cells stay empty.
fn paint_heat(
    grid: &mut [Vec<Option<Color>>],
    pixel_w: usize,
    pixel_h: usize,
    age_secs: f64,
) {
    const MAX_AGE: f64 = 300.0; // 5 minutes — fully cooled
    const HEAT_COLS: usize = 6; // trailing pixel columns to glow

    if age_secs >= MAX_AGE {
        return;
    }

    // Overall heat intensity: 1.0 when fresh → 0.0 at MAX_AGE.
    // Use a square-root curve so it stays warm longer before fading fast.
    let age_frac = (age_secs / MAX_AGE).clamp(0.0, 1.0);
    let heat = 1.0 - age_frac.sqrt();

    let bright = Color::Rgb(255, 255, 255);

    // Find the rightmost pixel column that has any data.
    let trailing_col = (0..pixel_w)
        .rev()
        .find(|&cx| (0..pixel_h).any(|py| grid[py][cx].is_some()));

    let Some(trailing) = trailing_col else {
        return;
    };

    let start_col = trailing.saturating_sub(HEAT_COLS - 1);

    for cx in start_col..=trailing {
        // Spatial falloff: 1.0 at trailing edge → 0.0 at start_col.
        let dist = trailing - cx;
        let spatial = 1.0 - (dist as f64 / HEAT_COLS as f64);

        let boost = heat * spatial * 0.5; // cap at 50% blend toward white

        for row in &mut grid[..pixel_h] {
            if let Some(color) = row[cx] {
                row[cx] = Some(theme::lerp_color(color, bright, boost));
            }
        }
    }
}

