use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::Block;
use ratatui::Frame;

use crate::tui::app::{App, FullscreenView};
use crate::tui::theme;
use crate::tui::views::{help, settings};
use crate::tui::widgets::{header, heatmap, spike_chart, status_bar, summary_cards, usage_table};

/// Render the complete dashboard view.
///
/// Default layout — the spike chart and heatmap are the core visuals and
/// are always rendered when there's space. The usage table fills whatever
/// is left over.
///
/// ```text
/// ┌────────────── header (1 line) ──────────────┐
/// ├──────────── summary cards (5–7 lines) ──────┤
/// ├─────────── spike chart (6–10 lines) ────────┤
/// ├─────────── heatmap (10–12 lines) ───────────┤
/// ├────────── usage table (flexible) ───────────┤
/// ├────────────── status bar (1 line) ──────────┤
/// └─────────────────────────────────────────────┘
/// ```
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Fill the entire background
    let bg = Block::default().style(theme::text());
    frame.render_widget(bg, area);

    match app.fullscreen {
        FullscreenView::Heatmap | FullscreenView::SpikeChart => {
            let layout = Layout::vertical([
                Constraint::Length(1), // header
                Constraint::Min(10),   // chart
                Constraint::Length(1), // status bar
            ])
            .split(area);

            header::render(frame, layout[0], app);
            match app.fullscreen {
                FullscreenView::Heatmap => {
                    heatmap::render(frame, layout[1], &app.heatmap_data);
                }
                FullscreenView::SpikeChart => {
                    spike_chart::render(
                        frame,
                        layout[1],
                        &app.spike_chart_data,
                        app.spike_chart_age_secs(),
                    );
                }
                FullscreenView::None => unreachable!(),
            }
            status_bar::render(frame, layout[2], app);
        }
        FullscreenView::None => {
            render_default(frame, area, app);
        }
    }

    // Overlays (rendered on top of everything)
    if app.show_help {
        help::render(frame);
    }
    if app.show_settings {
        settings::render(frame, app);
    }
}

/// Render the default (non-fullscreen) dashboard. Spike chart and heatmap
/// are the core visuals — they're always rendered if there's room, and
/// the usage table takes whatever vertical space is left.
fn render_default(frame: &mut ratatui::Frame, area: ratatui::layout::Rect, app: &App) {
    // Fixed chrome: header + status bar.
    const CHROME: u16 = 2;

    // Decide which sections get space, working from the most important
    // (spike chart + heatmap) outwards. Each decision reserves space from
    // a running `remaining` budget.
    let total = area.height;
    let mut remaining = total.saturating_sub(CHROME);

    // Spike chart: core visual. Give it 10 rows if we can afford it,
    // degrade to 8 / 6 on tighter terminals, skip only if we can't fit
    // its minimum (5 rows).
    let spike_height: u16 = if remaining >= 10 {
        10
    } else if remaining >= 8 {
        8
    } else if remaining >= 6 {
        6
    } else if remaining >= 5 {
        5
    } else {
        0
    };
    remaining = remaining.saturating_sub(spike_height);

    // Heatmap: core visual. Needs at least 10 rows and 60 cols — otherwise
    // it renders a "too small" placeholder, so skip it entirely below that.
    let heatmap_height: u16 = if remaining >= 12 && area.width >= 60 {
        12
    } else if remaining >= 10 && area.width >= 60 {
        10
    } else {
        0
    };
    remaining = remaining.saturating_sub(heatmap_height);

    // Summary cards: optional. Show 7-line variant on very tall terminals,
    // 5-line on medium, skip on tight.
    let card_height: u16 = if remaining >= 7 + 5 {
        // Only show cards if the usage table still gets a usable slice.
        7
    } else if remaining >= 5 + 5 {
        5
    } else {
        0
    };
    remaining = remaining.saturating_sub(card_height);

    // Usage table: flexible, fills whatever is left (if at least 3 rows).
    let show_table = remaining >= 3;

    // Assemble constraints in render order.
    let mut constraints: Vec<Constraint> = Vec::with_capacity(6);
    constraints.push(Constraint::Length(1)); // header
    if card_height > 0 {
        constraints.push(Constraint::Length(card_height));
    }
    if spike_height > 0 {
        constraints.push(Constraint::Length(spike_height));
    }
    if heatmap_height > 0 {
        constraints.push(Constraint::Length(heatmap_height));
    }
    if show_table {
        constraints.push(Constraint::Min(3));
    }
    constraints.push(Constraint::Length(1)); // status bar

    let layout = Layout::vertical(constraints).split(area);
    let mut idx = 0;

    header::render(frame, layout[idx], app);
    idx += 1;

    if card_height > 0 {
        summary_cards::render(frame, layout[idx], app);
        idx += 1;
    }

    if spike_height > 0 {
        spike_chart::render(
            frame,
            layout[idx],
            &app.spike_chart_data,
            app.spike_chart_age_secs(),
        );
        idx += 1;
    }

    if heatmap_height > 0 {
        heatmap::render(frame, layout[idx], &app.heatmap_data);
        idx += 1;
    }

    if show_table {
        usage_table::render(frame, layout[idx], app);
        idx += 1;
    }

    status_bar::render(frame, layout[idx], app);
}
