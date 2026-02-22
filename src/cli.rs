use chrono::NaiveDate;
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DisplayMode {
    /// Per-model breakdown rows
    Breakdown,
    /// One row per date compact view
    Compact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SortOrder {
    /// Oldest first
    Asc,
    /// Newest first
    Desc,
}

#[derive(Parser)]
#[command(name = "tokemon", version, about = "Unified LLM token usage tracking across all providers")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Output as JSON instead of table
    #[arg(long, global = true)]
    pub json: bool,

    /// Display mode: breakdown (per-model) or compact (per-date)
    #[arg(short = 'd', long, global = true, value_enum)]
    pub display: Option<DisplayMode>,

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
    #[arg(short = 'o', long, global = true, value_enum)]
    pub order: Option<SortOrder>,
}

impl Cli {
    /// Resolve display mode from CLI flag and config default
    pub fn display_mode(&self, config: &crate::config::Config) -> DisplayMode {
        self.display.unwrap_or_else(|| {
            if config.breakdown {
                DisplayMode::Breakdown
            } else {
                DisplayMode::Compact
            }
        })
    }

    /// Whether to use descending sort order
    pub fn is_desc(&self, config: &crate::config::Config) -> bool {
        match self.order {
            Some(SortOrder::Desc) => true,
            Some(SortOrder::Asc) => false,
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
