use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate};

use crate::types::{DailySummary, ModelUsage, Record};

/// Group entries by date, then by model within each date
pub fn aggregate_daily(entries: &[Record]) -> Vec<DailySummary> {
    let grouped = group_by_date(entries, |e| {
        let date = e.timestamp.date_naive();
        (date, date.format("%Y-%m-%d").to_string())
    });
    build_summaries(grouped)
}

/// Group entries by ISO week
pub fn aggregate_weekly(entries: &[Record]) -> Vec<DailySummary> {
    let grouped = group_by_date(entries, |e| {
        let date = e.timestamp.date_naive();
        let iso = date.iso_week();
        let year = iso.year();
        let week = iso.week();
        // Use Monday of the ISO week as the representative date
        let monday = NaiveDate::from_isoywd_opt(year, week, chrono::Weekday::Mon)
            .unwrap_or(date);
        let sunday = monday + chrono::Duration::days(6);
        let label = format!(
            "{}-W{:02} ({} - {})",
            year,
            week,
            monday.format("%b %d"),
            sunday.format("%b %d")
        );
        (monday, label)
    });
    build_summaries(grouped)
}

/// Group entries by month
pub fn aggregate_monthly(entries: &[Record]) -> Vec<DailySummary> {
    let grouped = group_by_date(entries, |e| {
        let date = e.timestamp.date_naive();
        let first = NaiveDate::from_ymd_opt(date.year(), date.month(), 1).unwrap_or(date);
        let label = date.format("%B %Y").to_string();
        (first, label)
    });
    build_summaries(grouped)
}

/// Apply date range filter
pub fn filter_by_date(
    entries: Vec<Record>,
    since: Option<NaiveDate>,
    until: Option<NaiveDate>,
) -> Vec<Record> {
    entries
        .into_iter()
        .filter(|e| {
            let date = e.timestamp.date_naive();
            since.map_or(true, |s| date >= s) && until.map_or(true, |u| date <= u)
        })
        .collect()
}

fn group_by_date<F>(
    entries: &[Record],
    key_fn: F,
) -> BTreeMap<NaiveDate, (String, Vec<&Record>)>
where
    F: Fn(&Record) -> (NaiveDate, String),
{
    let mut grouped: BTreeMap<NaiveDate, (String, Vec<&Record>)> = BTreeMap::new();
    for entry in entries {
        let (date, label) = key_fn(entry);
        grouped
            .entry(date)
            .or_insert_with(|| (label, Vec::new()))
            .1
            .push(entry);
    }
    grouped
}

fn build_summaries(
    grouped: BTreeMap<NaiveDate, (String, Vec<&Record>)>,
) -> Vec<DailySummary> {
    let mut summaries = Vec::new();

    for (date, (label, entries)) in grouped {
        let mut model_map: BTreeMap<String, ModelUsage> = BTreeMap::new();

        for entry in &entries {
            let model_name = entry.model.as_deref().unwrap_or("unknown").to_string();
            let key = format!("{}:{}", entry.provider, model_name);

            let mu = model_map.entry(key).or_insert_with(|| ModelUsage {
                model: model_name.clone(),
                provider: entry.provider.clone(),
                ..Default::default()
            });

            mu.input_tokens += entry.input_tokens;
            mu.output_tokens += entry.output_tokens;
            mu.cache_read_tokens += entry.cache_read_tokens;
            mu.cache_creation_tokens += entry.cache_creation_tokens;
            mu.thinking_tokens += entry.thinking_tokens;
            mu.cost_usd += entry.cost_usd.unwrap_or(0.0);
            mu.request_count += 1;
        }

        let models: Vec<ModelUsage> = model_map.into_values().collect();

        let total_input: u64 = models.iter().map(|m| m.input_tokens).sum();
        let total_output: u64 = models.iter().map(|m| m.output_tokens).sum();
        let total_cache: u64 = models
            .iter()
            .map(|m| m.cache_read_tokens + m.cache_creation_tokens)
            .sum();
        let total_thinking: u64 = models.iter().map(|m| m.thinking_tokens).sum();
        let total_cost: f64 = models.iter().map(|m| m.cost_usd).sum();
        let total_requests: u64 = models.iter().map(|m| m.request_count).sum();

        summaries.push(DailySummary {
            date,
            label,
            models,
            total_input,
            total_output,
            total_cache,
            total_thinking,
            total_cost,
            total_requests,
        });
    }

    summaries
}
