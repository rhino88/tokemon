use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Sparkline};
use ratatui::Frame;

use crate::tui::app::{App, Scope};
use crate::tui::theme;

/// Render the four summary cards: Today, This Week, This Month, All Time.
///
/// Each card shows:
/// - Label (highlighted if it matches the active scope)
/// - Cost (large, bold)
/// - Token count (secondary)
/// - Sparkline (trend)
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    // Split into 4 equal columns
    let [c1, c2, c3, c4] = Layout::horizontal([
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
    ])
    .areas(area);

    let scoped = [
        (Scope::Today, c1),
        (Scope::Week, c2),
        (Scope::Month, c3),
        (Scope::AllTime, c4),
    ];
    for (i, &(scope, card_area)) in scoped.iter().enumerate() {
        render_card(frame, card_area, &app.cards[i], scope == app.scope);
    }
}

fn render_card(frame: &mut Frame, area: Rect, card: &crate::tui::app::CardData, active: bool) {
    // Card block with border
    let border_style = if active {
        theme::border().fg(theme::ACCENT)
    } else {
        theme::border()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(theme::card());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 2 || inner.width < 8 {
        return;
    }

    // Layout within card: label, cost, tokens, sparkline
    let constraints = if inner.height >= 5 {
        vec![
            Constraint::Length(1), // label
            Constraint::Length(1), // cost
            Constraint::Length(1), // tokens
            Constraint::Min(1),    // sparkline
        ]
    } else if inner.height >= 3 {
        vec![
            Constraint::Length(1), // label
            Constraint::Length(1), // cost
            Constraint::Length(1), // tokens
        ]
    } else {
        vec![
            Constraint::Length(1), // label
            Constraint::Length(1), // cost
        ]
    };

    let card_areas = Layout::vertical(constraints).split(inner);

    // Label with trend indicator
    let label_style = if active {
        theme::card_label().add_modifier(Modifier::UNDERLINED)
    } else {
        theme::card_label()
    };
    let trend_color = match card.trend.cmp(&0) {
        std::cmp::Ordering::Greater => theme::GREEN,
        std::cmp::Ordering::Less => theme::RED,
        std::cmp::Ordering::Equal => theme::DIM,
    };
    let label = Line::from(vec![
        Span::styled(card.label, label_style),
        Span::styled(
            format!(" {}", card.trend_symbol()),
            ratatui::style::Style::default()
                .fg(trend_color)
                .bg(theme::SURFACE),
        ),
    ]);
    frame.render_widget(label, card_areas[0]);

    // Cost
    let cost_line = Line::from(Span::styled(card.cost_str(), theme::card_value()));
    frame.render_widget(cost_line, card_areas[1]);

    // Tokens (if space)
    if card_areas.len() >= 3 {
        let tokens_line = Line::from(Span::styled(card.tokens_str(), theme::card_secondary()));
        frame.render_widget(tokens_line, card_areas[2]);
    }

    // Sparkline (if space)
    if card_areas.len() >= 4 && !card.sparkline.is_empty() {
        let sparkline = Sparkline::default().data(&card.sparkline).style(
            ratatui::style::Style::default()
                .fg(if active {
                    theme::ACCENT
                } else {
                    theme::ACCENT_DIM
                })
                .bg(theme::SURFACE),
        );
        frame.render_widget(sparkline, card_areas[3]);
    }
}
