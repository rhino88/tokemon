use chrono::Utc;
use clap::Parser;

mod aggregator;
mod cli;
mod config;
mod dedup;
mod error;
mod parse_utils;
mod output;
mod paths;
mod pricing;
mod provider;
mod types;

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

    // Parse entries from providers
    let mut entries = registry.all_entries(provider_filter)?;

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
