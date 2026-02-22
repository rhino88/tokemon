use chrono::Utc;
use clap::Parser;

mod aggregator;
mod cache;
mod cli;
mod config;
mod dedup;
mod error;
mod output;
mod parse_utils;
mod paths;
mod pricing;
mod provider;
mod types;

use cache::Cache;
use cli::{Cli, Commands};
use config::Config;
use provider::ProviderRegistry;
use types::Report;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = Config::load();

    let command = cli.command.as_ref().unwrap_or(&Commands::Daily);

    match command {
        Commands::Discover => cmd_discover(),
        Commands::Init => cmd_init(),
        Commands::Daily => cmd_report(&cli, &config, "daily"),
        Commands::Weekly => cmd_report(&cli, &config, "weekly"),
        Commands::Monthly => cmd_report(&cli, &config, "monthly"),
    }
}

fn cmd_discover() -> anyhow::Result<()> {
    let registry = ProviderRegistry::new();

    let info: Vec<(&str, &str, bool, String, usize)> = registry
        .all_providers()
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

    output::print_discover(&info);
    Ok(())
}

fn cmd_init() -> anyhow::Result<()> {
    let path = Config::write_default()?;
    println!("Config written to: {}", path.display());
    println!("Edit this file to customize default behavior.");
    Ok(())
}

fn cmd_report(cli: &Cli, config: &Config, period: &str) -> anyhow::Result<()> {
    let registry = ProviderRegistry::new();

    // Determine providers: CLI overrides config
    let provider_filter = if cli.providers.is_empty() {
        &config.providers
    } else {
        &cli.providers
    };

    // Parse entries, using cache for speed
    let mut entries = parse_with_cache(&registry, provider_filter)?;

    if entries.is_empty() {
        if cli.json {
            let report = Report {
                period: period.to_string(),
                generated_at: Utc::now(),
                providers_found: Vec::new(),
                summaries: Vec::new(),
                total_cost: 0.0,
                total_tokens: 0,
            };
            output::print_json(&report);
        } else {
            println!("No usage data found.");
            if provider_filter.is_empty() {
                println!("Run `tokemon discover` to see which providers are available.");
            }
        }
        return Ok(());
    }

    // Apply date filters
    entries = aggregator::filter_by_date(entries, cli.since, cli.until);

    // Apply pricing (CLI flags override config)
    let no_cost = cli.no_cost || config.no_cost;
    let offline = cli.offline || config.offline;
    if !no_cost {
        match pricing::PricingEngine::load(offline) {
            Ok(engine) => engine.apply_costs(&mut entries),
            Err(e) => {
                eprintln!("[tokemon] Warning: pricing unavailable: {}", e);
            }
        }
    }

    // Collect provider names
    let mut providers_found: Vec<String> = entries
        .iter()
        .map(|e| e.provider.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    providers_found.sort();

    // Aggregate
    let mut summaries = match period {
        "weekly" => aggregator::aggregate_weekly(&entries),
        "monthly" => aggregator::aggregate_monthly(&entries),
        _ => aggregator::aggregate_daily(&entries),
    };

    // Sort order
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
        output::print_json(&report);
    } else {
        let breakdown = cli.display_mode(config) == cli::DisplayMode::Breakdown;
        output::print_table(&report, breakdown);
    }

    Ok(())
}

/// Parse entries using cache. Strategy:
/// 1. Get all cached (file, mtime) pairs in one query
/// 2. Discover provider files and check which have changed
/// 3. Only parse changed files, store results in cache
/// 4. Load everything from cache in one bulk query
fn parse_with_cache(
    registry: &ProviderRegistry,
    filter: &[String],
) -> anyhow::Result<Vec<types::UsageEntry>> {
    let cache = match Cache::open() {
        Ok(c) => Some(c),
        Err(e) => {
            eprintln!("[tokemon] Warning: cache unavailable ({}); parsing all files", e);
            None
        }
    };

    let providers: Vec<&dyn provider::Provider> = if filter.is_empty() {
        registry.available()
    } else {
        let mut selected = Vec::new();
        for name in filter {
            match registry.get(name) {
                Some(p) => selected.push(p),
                None => {
                    return Err(error::TokemonError::ProviderNotFound(name.clone()).into())
                }
            }
        }
        selected
    };

    // Get cached file mtimes in one bulk query
    let cached_mtimes = cache
        .as_ref()
        .map(|c| c.cached_file_mtimes())
        .unwrap_or_default();

    // Find files that need (re)parsing
    let mut files_to_parse: Vec<(&dyn provider::Provider, std::path::PathBuf, i64)> = Vec::new();

    for provider in &providers {
        for file in provider.discover_files() {
            let mtime = cache::file_mtime_secs(&file).unwrap_or(0);
            let file_key = file.display().to_string();

            match cached_mtimes.get(&file_key) {
                Some(&cached_mtime) if cached_mtime == mtime => {
                    // Cache is fresh, skip
                }
                _ => {
                    // New or modified file
                    files_to_parse.push((*provider, file, mtime));
                }
            }
        }
    }

    // Parse changed files and update cache
    if !files_to_parse.is_empty() {
        if let Some(ref cache) = cache {
            let _ = cache.begin();
        }

        for (provider, file, mtime) in &files_to_parse {
            match provider.parse_file(file) {
                Ok(entries) => {
                    if let Some(ref cache) = cache {
                        if let Err(e) = cache.store_file_entries(file, *mtime, &entries) {
                            eprintln!("[tokemon] Warning: cache write failed: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[tokemon] Warning: failed to parse {}: {}", file.display(), e);
                }
            }
        }

        if let Some(ref cache) = cache {
            let _ = cache.commit();
        }
    }

    // Load all entries from cache in one bulk query
    let mut all_entries = if let Some(ref cache) = cache {
        cache.load_all_entries()?
    } else {
        // No cache — parse everything directly
        let mut entries = Vec::new();
        for provider in &providers {
            match provider.parse_all() {
                Ok(e) => entries.extend(e),
                Err(e) => eprintln!("[tokemon] Warning: {}: {}", provider.name(), e),
            }
        }
        entries
    };

    all_entries = dedup::deduplicate(all_entries);
    all_entries.sort_by_key(|e| e.timestamp);
    Ok(all_entries)
}
