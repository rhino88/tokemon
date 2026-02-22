use std::collections::BTreeSet;

use tabled::settings::object::Columns;
use tabled::settings::{Alignment, Modify, Style};
use tabled::{Table, Tabled};

use crate::config::ColumnConfig;
use crate::types::Report;

/// Breakdown mode: one row per model per date
#[derive(Tabled)]
struct BreakdownRow {
    #[tabled(rename = "Date")]
    date: String,
    #[tabled(rename = "Provider")]
    provider: String,
    #[tabled(rename = "Model")]
    model: String,
    #[tabled(rename = "Input")]
    input: String,
    #[tabled(rename = "Output")]
    output: String,
    #[tabled(rename = "Cache Write")]
    cache_write: String,
    #[tabled(rename = "Cache Read")]
    cache_read: String,
    #[tabled(rename = "Total Tokens")]
    total_tokens: String,
    #[tabled(rename = "Cost")]
    cost: String,
}

/// Compact mode: one row per date 
#[derive(Tabled)]
struct CompactRow {
    #[tabled(rename = "Date")]
    date: String,
    #[tabled(rename = "Models")]
    models: String,
    #[tabled(rename = "Input")]
    input: String,
    #[tabled(rename = "Output")]
    output: String,
    #[tabled(rename = "Cache Write")]
    cache_write: String,
    #[tabled(rename = "Cache Read")]
    cache_read: String,
    #[tabled(rename = "Total Tokens")]
    total_tokens: String,
    #[tabled(rename = "Cost")]
    cost: String,
}

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

pub fn print_table(report: &Report, breakdown: bool, _columns: &ColumnConfig) {
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
    let mut rows: Vec<BreakdownRow> = Vec::new();

    for summary in &report.summaries {
        let date_label = &summary.label;
        for model in &summary.models {
            let total = model.input_tokens
                + model.output_tokens
                + model.cache_read_tokens
                + model.cache_creation_tokens
                + model.thinking_tokens;
            rows.push(BreakdownRow {
                date: date_label.clone(),
                provider: model.provider.clone(),
                model: shorten_model(&model.model),
                input: format_tokens(model.input_tokens),
                output: format_tokens(model.output_tokens),
                cache_write: format_tokens(model.cache_creation_tokens),
                cache_read: format_tokens(model.cache_read_tokens),
                total_tokens: format_tokens(total),
                cost: format_cost(model.cost_usd),
            });
        }
    }

    // Grand totals
    let (gi, go, gcw, gcr, gt) = grand_totals(report);
    rows.push(BreakdownRow {
        date: "TOTAL".to_string(),
        provider: String::new(),
        model: String::new(),
        input: format_tokens(gi),
        output: format_tokens(go),
        cache_write: format_tokens(gcw),
        cache_read: format_tokens(gcr),
        total_tokens: format_tokens(gt),
        cost: format_cost(report.total_cost),
    });

    let table = Table::new(&rows)
        .with(Style::rounded())
        .with(Modify::new(Columns::new(3..)).with(Alignment::right()))
        .to_string();
    println!("{}", table);
}

fn print_compact_table(report: &Report) {
    let mut rows: Vec<CompactRow> = Vec::new();

    for summary in &report.summaries {
        // Collect unique model short names
        let model_names: BTreeSet<String> = summary
            .models
            .iter()
            .map(|m| shorten_model(&m.model))
            .collect();
        let models_str = model_names
            .iter()
            .map(|m| format!("- {}", m))
            .collect::<Vec<_>>()
            .join("\n");

        let total = summary.total_input
            + summary.total_output
            + summary.total_cache
            + summary.total_thinking;

        rows.push(CompactRow {
            date: summary.label.clone(),
            models: models_str,
            input: format_tokens(summary.total_input),
            output: format_tokens(summary.total_output),
            cache_write: format_tokens(
                summary
                    .models
                    .iter()
                    .map(|m| m.cache_creation_tokens)
                    .sum(),
            ),
            cache_read: format_tokens(
                summary
                    .models
                    .iter()
                    .map(|m| m.cache_read_tokens)
                    .sum(),
            ),
            total_tokens: format_tokens(total),
            cost: format_cost(summary.total_cost),
        });
    }

    // Grand totals
    let (gi, go, gcw, gcr, gt) = grand_totals(report);
    rows.push(CompactRow {
        date: "TOTAL".to_string(),
        models: String::new(),
        input: format_tokens(gi),
        output: format_tokens(go),
        cache_write: format_tokens(gcw),
        cache_read: format_tokens(gcr),
        total_tokens: format_tokens(gt),
        cost: format_cost(report.total_cost),
    });

    let table = Table::new(&rows)
        .with(Style::rounded())
        .with(Modify::new(Columns::new(2..)).with(Alignment::right()))
        .to_string();
    println!("{}", table);
}

fn grand_totals(report: &Report) -> (u64, u64, u64, u64, u64) {
    let gi: u64 = report.summaries.iter().map(|s| s.total_input).sum();
    let go: u64 = report.summaries.iter().map(|s| s.total_output).sum();
    let gcw: u64 = report
        .summaries
        .iter()
        .flat_map(|s| s.models.iter())
        .map(|m| m.cache_creation_tokens)
        .sum();
    let gcr: u64 = report
        .summaries
        .iter()
        .flat_map(|s| s.models.iter())
        .map(|m| m.cache_read_tokens)
        .sum();
    let gt = gi
        + go
        + gcw
        + gcr
        + report
            .summaries
            .iter()
            .map(|s| s.total_thinking)
            .sum::<u64>();
    (gi, go, gcw, gcr, gt)
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
}
