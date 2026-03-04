use chrono::{Datelike, NaiveDate, Utc};
use clap::Parser;

mod cache;
mod cli;
mod config;
mod cost;
mod dedup;
mod display;
mod error;
mod mcp;
mod pacemaker;
mod paths;
mod render;
mod rollup;
mod source;
mod timestamp;
mod types;

use cache::Cache;
use cli::{Cli, Commands, Frequency};
use config::Config;
use source::SourceSet;
use types::{Report, SessionReport};

const REDISCOVERY_INTERVAL_SECS: u64 = 30;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = Config::load();

    match cli.command.as_ref() {
        None => cmd_report(&cli, &config),
        Some(Commands::Discover) => cmd_discover(),
        Some(Commands::Init) => cmd_init(),
        Some(Commands::Statusline) => cmd_statusline(&cli, &config),
        Some(Commands::Budget) => cmd_budget(&cli, &config),
        Some(Commands::Sessions { top }) => cmd_sessions(&cli, &config, *top),
        Some(Commands::Prune { before }) => cmd_prune(*before),
        Some(Commands::Mcp) => mcp::run(&cli, &config),
    }
}

fn cmd_discover() -> anyhow::Result<()> {
    let registry = SourceSet::new();

    let info: Vec<(&str, &str, bool, String, usize)> = registry
        .all()
        .iter()
        .map(|p| {
            let available = p.is_available();
            let data_dir = p.data_dir().display().to_string();
            let file_count = if available {
                p.discover_files().len()
            } else {
                0
            };
            (p.name(), p.display_name(), available, data_dir, file_count)
        })
        .collect();

    render::print_discover(&info);
    Ok(())
}

fn cmd_init() -> anyhow::Result<()> {
    let path = Config::write_default()?;
    println!("Config written to: {}", path.display());
    println!("Edit this file to customize default behavior.");
    Ok(())
}

// --- Shared helpers for command handlers ---

fn resolve_providers<'a>(cli: &'a Cli, config: &'a Config) -> &'a [String] {
    if cli.providers.is_empty() {
        &config.providers
    } else {
        &cli.providers
    }
}

pub(crate) fn load_and_price(
    cli: &Cli,
    config: &Config,
    force_offline: bool,
) -> anyhow::Result<Vec<types::Record>> {
    let registry = SourceSet::new();
    let filter = resolve_providers(cli, config);
    let force_refresh = cli.refresh || config.refresh;
    let force_reparse = cli.reparse || config.reparse;
    let mut entries = parse_with_cache(
        &registry,
        filter,
        force_refresh,
        force_reparse,
        cli.since,
        cli.until,
    )?;

    if !(cli.no_cost || config.no_cost) {
        let offline = force_offline || cli.offline || config.offline;
        match cost::PricingEngine::load(offline) {
            Ok(engine) => engine.apply_costs(&mut entries),
            Err(e) => {
                if !force_offline {
                    eprintln!("[tokemon] Warning: pricing unavailable: {}", e);
                }
            }
        }
    }

    Ok(entries)
}

// --- Command handlers ---

fn cmd_report(cli: &Cli, config: &Config) -> anyhow::Result<()> {
    let freq = cli.frequency;
    let period = match freq {
        Frequency::Daily => "daily",
        Frequency::Weekly => "weekly",
        Frequency::Monthly => "monthly",
    };
    let mut entries = load_and_price(cli, config, false)?;

    if entries.is_empty() {
        let empty_report = Report {
            period: period.to_string(),
            generated_at: Utc::now(),
            providers_found: Vec::new(),
            summaries: Vec::new(),
            total_cost: 0.0,
            total_tokens: 0,
        };
        if cli.json {
            render::print_json(&empty_report);
        } else if cli.csv {
            let breakdown = cli.display_mode(config) == cli::DisplayMode::Breakdown;
            if breakdown {
                render::print_csv_breakdown(&empty_report);
            } else {
                render::print_csv_compact(&empty_report);
            }
        } else {
            println!("No usage data found.");
            if resolve_providers(cli, config).is_empty() {
                println!("Run `tokemon discover` to see which providers are available.");
            }
        }
        return Ok(());
    }

    entries = rollup::filter_by_date(entries, cli.since, cli.until);

    let mut providers_found: Vec<String> = entries
        .iter()
        .map(|e| e.provider.to_string())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    providers_found.sort_unstable();

    let mut summaries = match freq {
        Frequency::Weekly => rollup::aggregate_weekly(&entries),
        Frequency::Monthly => rollup::aggregate_monthly(&entries),
        Frequency::Daily => rollup::aggregate_daily(&entries),
    };

    if cli.is_desc(config) {
        summaries.reverse();
    }

    let total_cost: f64 = summaries.iter().map(|s| s.total_cost).sum();
    let total_tokens: u64 = entries.iter().map(|e| e.total_tokens()).sum();

    let report = Report {
        period: period.to_string(),
        generated_at: Utc::now(),
        providers_found,
        summaries,
        total_cost,
        total_tokens,
    };

    if cli.json {
        render::print_json(&report);
    } else if cli.csv {
        let breakdown = cli.display_mode(config) == cli::DisplayMode::Breakdown;
        if breakdown {
            render::print_csv_breakdown(&report);
        } else {
            render::print_csv_compact(&report);
        }
    } else {
        let breakdown = cli.display_mode(config) == cli::DisplayMode::Breakdown;
        render::print_table(&report, breakdown, &config.columns);
    }

    Ok(())
}

fn cmd_statusline(cli: &Cli, config: &Config) -> anyhow::Result<()> {
    let entries = load_and_price(cli, config, true)?;
    let freq = cli.frequency;

    let today = Utc::now().date_naive();
    let since = match freq {
        Frequency::Daily => today,
        Frequency::Weekly => {
            today - chrono::Duration::days(today.weekday().num_days_from_monday() as i64)
        }
        Frequency::Monthly => {
            chrono::NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today)
        }
    };
    let period_label = match freq {
        Frequency::Daily => "today",
        Frequency::Weekly => "this week",
        Frequency::Monthly => "this month",
    };

    let mut providers_seen = std::collections::HashSet::new();
    let (total_cost, total_tokens) = entries
        .iter()
        .filter(|e| e.timestamp.date_naive() >= since)
        .fold((0.0f64, 0u64), |(cost, tokens), e| {
            providers_seen.insert(&*e.provider);
            (cost + e.cost_usd.unwrap_or(0.0), tokens + e.total_tokens())
        });
    let provider_count = providers_seen.len();

    // Append budget info if configured
    let budget_str = if config.budget.daily.is_some()
        || config.budget.weekly.is_some()
        || config.budget.monthly.is_some()
    {
        let (daily, weekly, monthly) = pacemaker::evaluate(&entries, &config.budget);
        match freq {
            Frequency::Daily => daily.map(|(s, l)| format_budget_short(s, l)),
            Frequency::Weekly => weekly.map(|(s, l)| format_budget_short(s, l)),
            Frequency::Monthly => monthly.map(|(s, l)| format_budget_short(s, l)),
        }
    } else {
        None
    };

    match budget_str {
        Some(bs) => println!(
            "${:.2} | {} | {} | {} | {}",
            total_cost,
            render::format_tokens_short(total_tokens),
            format_provider_count(provider_count),
            period_label,
            bs
        ),
        None => render::print_statusline(total_cost, total_tokens, provider_count, period_label),
    }

    Ok(())
}

fn cmd_budget(cli: &Cli, config: &Config) -> anyhow::Result<()> {
    let entries = load_and_price(cli, config, false)?;
    let (daily, weekly, monthly) = pacemaker::evaluate(&entries, &config.budget);
    render::print_budget(daily, weekly, monthly);
    Ok(())
}

fn cmd_sessions(cli: &Cli, config: &Config, top: usize) -> anyhow::Result<()> {
    let entries = load_and_price(cli, config, false)?;

    if entries.is_empty() {
        let empty_report = SessionReport {
            generated_at: Utc::now(),
            sessions: Vec::new(),
            total_cost: 0.0,
            total_tokens: 0,
        };
        if cli.json {
            render::print_sessions_json(&empty_report);
        } else if cli.csv {
            render::print_csv_sessions(&empty_report);
        } else {
            println!("No usage data found.");
        }
        return Ok(());
    }

    let entries = rollup::filter_by_date(entries, cli.since, cli.until);
    let mut sessions = rollup::aggregate_by_session(&entries);
    sessions.truncate(top);

    let total_cost: f64 = sessions.iter().map(|s| s.cost).sum();
    let total_tokens: u64 = sessions.iter().map(|s| s.total_tokens).sum();

    let report = SessionReport {
        generated_at: Utc::now(),
        sessions,
        total_cost,
        total_tokens,
    };

    if cli.json {
        render::print_sessions_json(&report);
    } else if cli.csv {
        render::print_csv_sessions(&report);
    } else {
        render::print_sessions_table(&report);
    }

    Ok(())
}

fn cmd_prune(before: NaiveDate) -> anyhow::Result<()> {
    let cache = Cache::open()?;
    let deleted = cache.prune_before(before)?;
    println!(
        "Pruned {} preserved entries with timestamps before {}.",
        deleted, before
    );
    Ok(())
}

// --- Formatting helpers ---

fn format_budget_short(spent: f64, limit: f64) -> String {
    let pct = if limit > 0.0 {
        spent / limit * 100.0
    } else {
        0.0
    };
    if pct > 100.0 {
        format!("OVER ${:.0} limit", limit)
    } else {
        format!("{:.0}%", pct)
    }
}

fn format_provider_count(count: usize) -> String {
    if count == 1 {
        "1 provider".to_string()
    } else {
        format!("{} providers", count)
    }
}

// --- Cache-aware parsing ---

/// Parse entries using cache. Strategy:
/// 1. Get all cached (file, mtime) pairs in one query
/// 2. Discover provider files and check which have changed
/// 3. Only parse changed files, store results in cache
/// 4. Load everything from cache in one bulk query
fn parse_with_cache(
    registry: &SourceSet,
    filter: &[String],
    force_refresh: bool,
    force_reparse: bool,
    since: Option<NaiveDate>,
    until: Option<NaiveDate>,
) -> anyhow::Result<Vec<types::Record>> {
    let cache = Cache::open()
        .map_err(|e| {
            eprintln!(
                "[tokemon] Warning: cache unavailable ({}); parsing all files",
                e
            );
            e
        })
        .ok();

    let providers = resolve_source_refs(registry, filter)?;

    let Some(cache) = cache else {
        return parse_all_directly(&providers);
    };

    let has_filters = since.is_some() || until.is_some() || !filter.is_empty();

    // If cache is fresh and no --refresh/--reparse flag, skip discovery entirely
    if !force_refresh && !force_reparse && !cache.should_rediscover(REDISCOVERY_INTERVAL_SECS) {
        let mut entries = if has_filters {
            cache.load_entries_filtered(since, until, filter)?
        } else {
            cache.load_all_entries()?
        };
        entries = dedup::deduplicate(entries);
        entries.sort_by_key(|e| e.timestamp);
        return Ok(entries);
    }

    // When --reparse, ignore cached mtimes so every file gets re-parsed
    let cached_mtimes = if force_reparse {
        std::collections::HashMap::new()
    } else {
        cache.cached_file_mtimes()
    };

    // Discover all files and collect their paths for preservation tracking
    let all_discovered: Vec<(&dyn source::Source, std::path::PathBuf)> = providers
        .iter()
        .flat_map(|provider| {
            provider
                .discover_files()
                .into_iter()
                .map(move |file| (*provider, file))
        })
        .collect();

    let discovered_files: std::collections::HashSet<String> = all_discovered
        .iter()
        .map(|(_, file)| file.display().to_string())
        .collect();

    // Mark entries from deleted files as preserved (only when discovering all providers,
    // otherwise we'd incorrectly mark entries from non-filtered providers)
    if filter.is_empty() {
        cache.mark_preserved(&discovered_files);
    }

    // Find files that need (re)parsing
    let files_to_parse: Vec<_> = all_discovered
        .into_iter()
        .filter_map(|(provider, file)| {
            let mtime = cache::file_mtime_secs(&file).unwrap_or(0);
            let file_key = file.display().to_string();
            if cached_mtimes.get(&file_key) == Some(&mtime) {
                None
            } else {
                Some((provider, file, mtime))
            }
        })
        .collect();

    // Parse changed files in parallel, then insert into cache serially
    if !files_to_parse.is_empty() {
        use rayon::prelude::*;

        let results: Vec<_> = files_to_parse
            .par_iter()
            .map(|(provider, file, mtime)| {
                let parsed = provider.parse_file(file);
                (file, *mtime, parsed)
            })
            .collect();

        if let Err(e) = cache.begin() {
            eprintln!("[tokemon] Warning: cache transaction failed: {}", e);
        }

        for (file, mtime, parsed) in &results {
            match parsed {
                Ok(entries) => {
                    if let Err(e) = cache.store_file_entries(file, *mtime, entries) {
                        eprintln!("[tokemon] Warning: cache write failed: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[tokemon] Warning: failed to parse {}: {}",
                        file.display(),
                        e
                    );
                }
            }
        }

        if let Err(e) = cache.commit() {
            eprintln!("[tokemon] Warning: cache commit failed: {}", e);
        }
    }

    cache.set_last_discovery();

    let mut entries = if has_filters {
        cache.load_entries_filtered(since, until, filter)?
    } else {
        cache.load_all_entries()?
    };
    entries = dedup::deduplicate(entries);
    entries.sort_by_key(|e| e.timestamp);
    Ok(entries)
}

fn resolve_source_refs<'a>(
    registry: &'a SourceSet,
    filter: &[String],
) -> anyhow::Result<Vec<&'a dyn source::Source>> {
    if filter.is_empty() {
        return Ok(registry.available());
    }

    filter
        .iter()
        .map(|name| {
            registry
                .get(name)
                .ok_or_else(|| error::TokemonError::ProviderNotFound(name.clone()).into())
        })
        .collect()
}

fn parse_all_directly(providers: &[&dyn source::Source]) -> anyhow::Result<Vec<types::Record>> {
    let mut entries = Vec::new();
    for provider in providers {
        match provider.parse_all() {
            Ok(e) => entries.extend(e),
            Err(e) => eprintln!("[tokemon] Warning: {}: {}", provider.name(), e),
        }
    }
    entries = dedup::deduplicate(entries);
    entries.sort_by_key(|e| e.timestamp);
    Ok(entries)
}
