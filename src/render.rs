use std::io::IsTerminal;

use tabled::builder::Builder;
use tabled::settings::object::Columns;
use tabled::settings::{Alignment, Modify, Style};
use tabled::{Table, Tabled};

use crate::types::Report;

// ---------------------------------------------------------------------------
// Color helpers
// ---------------------------------------------------------------------------

/// Whether to emit ANSI color codes. Respects NO_COLOR and non-TTY pipes.
#[must_use]
fn use_color() -> bool {
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

fn ansi(code: &str, s: &str, color: bool) -> String {
    if color {
        format!("\x1b[{}m{}\x1b[0m", code, s)
    } else {
        s.to_string()
    }
}

fn bold(s: &str, c: bool) -> String { ansi("1", s, c) }
fn dim(s: &str, c: bool) -> String { ansi("2", s, c) }
fn cyan_bold(s: &str, c: bool) -> String { ansi("1;36", s, c) }
fn green(s: &str, c: bool) -> String { ansi("32", s, c) }
fn yellow(s: &str, c: bool) -> String { ansi("33", s, c) }
fn red(s: &str, c: bool) -> String { ansi("31", s, c) }

/// Format a cost value with color coding.
fn format_cost_styled(cost: f64, color: bool) -> String {
    let s = format_cost(cost);
    if !color { return s; }
    if cost == 0.0 {
        dim(&s, true)
    } else if cost < 1.0 {
        green(&s, true)
    } else if cost < 10.0 {
        yellow(&s, true)
    } else {
        red(&s, true)
    }
}

/// Format a token count with dim styling for zeros.
fn format_tokens_styled(n: u64, color: bool) -> String {
    let s = format_tokens(n);
    if color && n == 0 {
        dim(&s, true)
    } else {
        s
    }
}

/// Apply bold to every element in a row.
fn bold_row(row: &mut [String], color: bool) {
    if !color { return; }
    for cell in row.iter_mut() {
        if !cell.is_empty() {
            *cell = bold(cell, true);
        }
    }
}

/// Style each element of the header row.
fn style_header(header: &mut [String], color: bool) {
    if !color { return; }
    for cell in header.iter_mut() {
        *cell = cyan_bold(cell, true);
    }
}

// ---------------------------------------------------------------------------
// Responsive columns
// ---------------------------------------------------------------------------

/// Determine which optional columns are visible based on terminal width.
/// Returns (show_input, show_output, show_cache_write, show_cache_read).
/// Total Tokens and Cost are always shown.
#[must_use]
fn visible_columns(extra_fixed_cols: usize) -> (bool, bool, bool, bool) {
    let width = terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(120);

    let fixed_overhead = extra_fixed_cols * 15 + 2 * 14 + 10;
    let remaining = width.saturating_sub(fixed_overhead);

    let show_cache_write = remaining >= 4 * 14;
    let show_cache_read = remaining >= 3 * 14;
    let show_input = remaining >= 2 * 14;
    let show_output = show_input;

    (show_input, show_output, show_cache_write, show_cache_read)
}

// ---------------------------------------------------------------------------
// Table printing
// ---------------------------------------------------------------------------

#[derive(Tabled)]
struct DiscoverRow {
    #[tabled(rename = "Provider")]
    provider: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Data Dir")]
    data_dir: String,
    #[tabled(rename = "Files")]
    files: String,
}

pub fn print_table(report: &Report, breakdown: bool) {
    if report.summaries.is_empty() {
        println!("No usage data found.");
        return;
    }

    if breakdown {
        print_breakdown_table(report);
    } else {
        print_compact_table(report);
    }
}

fn print_breakdown_table(report: &Report) {
    let color = use_color();
    let (show_in, show_out, show_cw, show_cr) = visible_columns(2);

    let mut header: Vec<String> = vec!["Date".into(), "Model".into()];
    if show_in { header.push("Input".into()); }
    if show_out { header.push("Output".into()); }
    if show_cw { header.push("Cache Write".into()); }
    if show_cr { header.push("Cache Read".into()); }
    header.push("Total Tokens".into());
    header.push("Cost".into());
    style_header(&mut header, color);

    let first_numeric_col = 2;

    let mut builder = Builder::default();
    builder.push_record(header);

    for summary in &report.summaries {
        // Date summary row — bold
        let total = summary.total_input
            + summary.total_output
            + summary.total_cache_creation()
            + summary.total_cache_read()
            + summary.total_thinking;

        let mut row: Vec<String> = vec![summary.label.clone(), String::new()];
        if show_in { row.push(format_tokens_styled(summary.total_input, color)); }
        if show_out { row.push(format_tokens_styled(summary.total_output, color)); }
        if show_cw { row.push(format_tokens_styled(summary.total_cache_creation(), color)); }
        if show_cr { row.push(format_tokens_styled(summary.total_cache_read(), color)); }
        row.push(format_tokens_styled(total, color));
        row.push(format_cost_styled(summary.total_cost, color));
        bold_row(&mut row, color);
        builder.push_record(row);

        // Model sub-rows
        for model in &summary.models {
            let model_total = model.input_tokens
                + model.output_tokens
                + model.cache_read_tokens
                + model.cache_creation_tokens
                + model.thinking_tokens;

            let mut row: Vec<String> =
                vec![String::new(), format!("  {}", shorten_model(&model.model))];
            if show_in { row.push(format_tokens_styled(model.input_tokens, color)); }
            if show_out { row.push(format_tokens_styled(model.output_tokens, color)); }
            if show_cw { row.push(format_tokens_styled(model.cache_creation_tokens, color)); }
            if show_cr { row.push(format_tokens_styled(model.cache_read_tokens, color)); }
            row.push(format_tokens_styled(model_total, color));
            row.push(format_cost_styled(model.cost_usd, color));
            builder.push_record(row);
        }
    }

    // Grand totals
    let (gi, go, gcw, gcr, gt) = grand_totals(report);
    let mut row: Vec<String> = vec!["TOTAL".into(), String::new()];
    if show_in { row.push(format_tokens(gi)); }
    if show_out { row.push(format_tokens(go)); }
    if show_cw { row.push(format_tokens(gcw)); }
    if show_cr { row.push(format_tokens(gcr)); }
    row.push(format_tokens(gt));
    row.push(format_cost(report.total_cost));
    bold_row(&mut row, color);
    builder.push_record(row);

    let table = builder
        .build()
        .with(Style::rounded())
        .with(Modify::new(Columns::new(first_numeric_col..)).with(Alignment::right()))
        .to_string();
    println!("{}", table);
}

fn print_compact_table(report: &Report) {
    let color = use_color();
    let (show_in, show_out, show_cw, show_cr) = visible_columns(1);

    let mut header: Vec<String> = vec!["Date".into()];
    if show_in { header.push("Input".into()); }
    if show_out { header.push("Output".into()); }
    if show_cw { header.push("Cache Write".into()); }
    if show_cr { header.push("Cache Read".into()); }
    header.push("Total Tokens".into());
    header.push("Cost".into());
    style_header(&mut header, color);

    let first_numeric_col = 1;

    let mut builder = Builder::default();
    builder.push_record(header);

    for summary in &report.summaries {
        let total = summary.total_input
            + summary.total_output
            + summary.total_cache
            + summary.total_thinking;

        let mut row: Vec<String> = vec![summary.label.clone()];
        if show_in { row.push(format_tokens_styled(summary.total_input, color)); }
        if show_out { row.push(format_tokens_styled(summary.total_output, color)); }
        if show_cw { row.push(format_tokens_styled(summary.total_cache_creation(), color)); }
        if show_cr { row.push(format_tokens_styled(summary.total_cache_read(), color)); }
        row.push(format_tokens_styled(total, color));
        row.push(format_cost_styled(summary.total_cost, color));
        builder.push_record(row);
    }

    // Grand totals
    let (gi, go, gcw, gcr, gt) = grand_totals(report);
    let mut row: Vec<String> = vec!["TOTAL".into()];
    if show_in { row.push(format_tokens(gi)); }
    if show_out { row.push(format_tokens(go)); }
    if show_cw { row.push(format_tokens(gcw)); }
    if show_cr { row.push(format_tokens(gcr)); }
    row.push(format_tokens(gt));
    row.push(format_cost(report.total_cost));
    bold_row(&mut row, color);
    builder.push_record(row);

    let table = builder
        .build()
        .with(Style::rounded())
        .with(Modify::new(Columns::new(first_numeric_col..)).with(Alignment::right()))
        .to_string();
    println!("{}", table);
}

fn grand_totals(report: &Report) -> (u64, u64, u64, u64, u64) {
    let gi: u64 = report.summaries.iter().map(|s| s.total_input).sum();
    let go: u64 = report.summaries.iter().map(|s| s.total_output).sum();
    let gcw: u64 = report.summaries.iter().map(|s| s.total_cache_creation()).sum();
    let gcr: u64 = report.summaries.iter().map(|s| s.total_cache_read()).sum();
    let gth: u64 = report.summaries.iter().map(|s| s.total_thinking).sum();
    (gi, go, gcw, gcr, gi + go + gcw + gcr + gth)
}

pub fn print_statusline(total_cost: f64, total_tokens: u64, provider_count: usize, period_label: &str) {
    let provider_str = if provider_count == 1 {
        "1 provider".to_string()
    } else {
        format!("{} providers", provider_count)
    };

    println!(
        "{} | {} | {} | {}",
        format_cost(total_cost),
        format_tokens_short(total_tokens),
        provider_str,
        period_label
    );
}

pub fn print_budget(daily: Option<(f64, f64)>, weekly: Option<(f64, f64)>, monthly: Option<(f64, f64)>) {
    let lines = [
        ("Daily", daily),
        ("Weekly", weekly),
        ("Monthly", monthly),
    ];

    let mut any = false;
    for (label, budget) in &lines {
        if let Some((spent, limit)) = budget {
            any = true;
            let pct = if *limit > 0.0 { spent / limit * 100.0 } else { 0.0 };
            let bar = progress_bar(pct, 10);
            let status = if pct > 100.0 {
                "OVER"
            } else if pct > 90.0 {
                "!!"
            } else if pct > 60.0 {
                "!"
            } else {
                "ok"
            };
            println!(
                "{:<8} ${:>8.2} / ${:<8.2} [{}] {:>5.1}%  {}",
                format!("{}:", label),
                spent,
                limit,
                bar,
                pct,
                status
            );
        }
    }

    if !any {
        println!("No budgets configured. Set [budget] in ~/.config/tokemon/config.toml");
    }
}

fn progress_bar(pct: f64, width: usize) -> String {
    let filled = ((pct / 100.0) * width as f64).min(width as f64) as usize;
    let empty = width.saturating_sub(filled);
    format!("{}{}", "#".repeat(filled), "-".repeat(empty))
}

#[must_use]
pub fn format_tokens_short(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1e9)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1e6)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1e3)
    } else {
        n.to_string()
    }
}

pub fn print_json(report: &Report) {
    match serde_json::to_string_pretty(report) {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("[tokemon] Error serializing report: {}", e),
    }
}

pub fn print_discover(providers: &[(&str, &str, bool, String, usize)]) {
    let rows: Vec<DiscoverRow> = providers
        .iter()
        .map(|(name, display, available, data_dir, file_count)| DiscoverRow {
            provider: format!("{} ({})", display, name),
            status: if *available {
                "Found".to_string()
            } else {
                "None".to_string()
            },
            data_dir: data_dir.clone(),
            files: format_tokens(*file_count as u64),
        })
        .collect();

    let table = Table::new(&rows).with(Style::rounded()).to_string();
    println!("{}", table);
}

fn format_tokens(n: u64) -> String {
    if n == 0 {
        return "0".to_string();
    }
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

fn format_cost(cost: f64) -> String {
    if cost == 0.0 {
        return "$0.00".to_string();
    }
    if cost < 0.01 {
        format!("${:.4}", cost)
    } else {
        format!("${:.2}", cost)
    }
}

fn shorten_model(model: &str) -> String {
    let s = model.to_string();

    if let Some(rest) = s.strip_prefix("claude-") {
        let without_date = strip_date_suffix(rest);
        return without_date.to_string();
    }

    strip_date_suffix(&s).to_string()
}

fn strip_date_suffix(s: &str) -> &str {
    if s.len() >= 9 {
        let last_9 = &s[s.len() - 9..];
        if last_9.starts_with('-')
            && last_9[1..].len() == 8
            && last_9[1..].chars().all(|c| c.is_ascii_digit())
        {
            return &s[..s.len() - 9];
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_tokens() {
        assert_eq!(format_tokens(0), "0");
        assert_eq!(format_tokens(123), "123");
        assert_eq!(format_tokens(1234), "1,234");
        assert_eq!(format_tokens(1234567), "1,234,567");
    }

    #[test]
    fn test_shorten_model() {
        assert_eq!(shorten_model("claude-opus-4-1-20250805"), "opus-4-1");
        assert_eq!(shorten_model("claude-sonnet-4-20250514"), "sonnet-4");
        assert_eq!(shorten_model("claude-opus-4-5-20251101"), "opus-4-5");
        assert_eq!(shorten_model("gpt-5-codex"), "gpt-5-codex");
        assert_eq!(shorten_model("gemini-2.5-flash"), "gemini-2.5-flash");
    }

    #[test]
    fn test_format_cost() {
        assert_eq!(format_cost(0.0), "$0.00");
        assert_eq!(format_cost(1.50), "$1.50");
        assert_eq!(format_cost(0.005), "$0.0050");
    }

    #[test]
    fn test_visible_columns() {
        let _ = visible_columns(2);
    }

    #[test]
    fn test_use_color_does_not_panic() {
        // Just ensure it runs without panicking in test context
        let _ = use_color();
    }

    #[test]
    fn test_format_cost_styled_no_color() {
        // Without color, should return plain string
        assert_eq!(format_cost_styled(0.0, false), "$0.00");
        assert_eq!(format_cost_styled(1.50, false), "$1.50");
    }

    #[test]
    fn test_format_tokens_styled_no_color() {
        assert_eq!(format_tokens_styled(0, false), "0");
        assert_eq!(format_tokens_styled(1234, false), "1,234");
    }
}
