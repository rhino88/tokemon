use chrono::NaiveDate;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "tokemon", version, about = "Unified LLM token usage tracking across all providers")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Output as JSON instead of table
    #[arg(long, global = true)]
    pub json: bool,

    /// Show per-model breakdown (default; use --no-breakdown for compact)
    #[arg(short = 'b', long, global = true)]
    pub breakdown: bool,

    /// Compact mode: one row per date (no per-model breakdown)
    #[arg(long = "no-breakdown", global = true)]
    pub no_breakdown: bool,

    /// Filter by provider (repeatable: -p claude-code -p codex)
    #[arg(short = 'p', long = "provider", global = true)]
    pub providers: Vec<String>,

    /// Show usage since this date (YYYY-MM-DD)
    #[arg(long, global = true)]
    pub since: Option<NaiveDate>,

    /// Show usage until this date (YYYY-MM-DD)
    #[arg(long, global = true)]
    pub until: Option<NaiveDate>,

    /// Skip cost calculation (faster, shows tokens only)
    #[arg(long, global = true)]
    pub no_cost: bool,

    /// Don't fetch remote pricing data (use cached/offline)
    #[arg(long, global = true)]
    pub offline: bool,

    /// Sort order: asc (oldest first) or desc (newest first)
    #[arg(short = 'o', long, global = true)]
    pub order: Option<String>,
}

impl Cli {
    /// Whether to show per-model breakdown, considering config and CLI flags
    pub fn should_breakdown(&self, config: &crate::config::Config) -> bool {
        if self.no_breakdown {
            return false;
        }
        if self.breakdown {
            return true;
        }
        config.breakdown
    }

    /// Whether to use descending sort order
    pub fn is_desc(&self, config: &crate::config::Config) -> bool {
        match &self.order {
            Some(o) => o == "desc",
            None => config.sort_order == "desc",
        }
    }
}

#[derive(Subcommand)]
pub enum Commands {
    /// Show daily usage breakdown (default when no subcommand given)
    Daily,
    /// Show weekly usage summary
    Weekly,
    /// Show monthly usage summary
    Monthly,
    /// List auto-detected providers on this machine
    Discover,
    /// Generate default config file at ~/.config/tokemon/config.toml
    Init,
}
