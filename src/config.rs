use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::paths;

const CONFIG_FILENAME: &str = "config.toml";

/// User configuration for tokemon
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Default subcommand when none specified: "daily", "weekly", "monthly"
    pub default_command: String,

    /// Default output format: "table" or "json"
    pub default_format: String,

    /// Whether to show per-model breakdown by default (like --breakdown)
    pub breakdown: bool,

    /// Skip cost calculation by default
    pub no_cost: bool,

    /// Use offline pricing by default
    pub offline: bool,

    /// Default providers to show (empty = all available)
    pub providers: Vec<String>,

    /// Column visibility settings
    pub columns: ColumnConfig,

    /// Sort order: "asc" (oldest first) or "desc" (newest first)
    pub sort_order: String,
}

/// Which columns to display in table output
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ColumnConfig {
    pub date: bool,
    pub provider: bool,
    pub model: bool,
    pub input: bool,
    pub output: bool,
    pub cache_write: bool,
    pub cache_read: bool,
    pub total_tokens: bool,
    pub cost: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_command: "daily".to_string(),
            default_format: "table".to_string(),
            breakdown: true,
            no_cost: false,
            offline: false,
            providers: Vec::new(),
            columns: ColumnConfig::default(),
            sort_order: "asc".to_string(),
        }
    }
}

impl Default for ColumnConfig {
    fn default() -> Self {
        Self {
            date: true,
            provider: true,
            model: true,
            input: true,
            output: true,
            cache_write: true,
            cache_read: true,
            total_tokens: true,
            cost: true,
        }
    }
}

impl Config {
    /// Load config from ~/.config/tokemon/config.toml, falling back to defaults
    pub fn load() -> Self {
        let path = Self::config_path();
        match fs::read_to_string(&path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(config) => {
                    let config: Config = config;
                    config.validated()
                }
                Err(e) => {
                    eprintln!(
                        "[tokemon] Warning: failed to parse {}: {}; using defaults",
                        path.display(),
                        e
                    );
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    /// Validate config values, replacing invalid ones with defaults
    fn validated(mut self) -> Self {
        let defaults = Self::default();

        if !["daily", "weekly", "monthly"].contains(&self.default_command.as_str()) {
            eprintln!(
                "[tokemon] Warning: invalid default_command '{}'; using '{}'",
                self.default_command, defaults.default_command
            );
            self.default_command = defaults.default_command;
        }

        if !["table", "json"].contains(&self.default_format.as_str()) {
            eprintln!(
                "[tokemon] Warning: invalid default_format '{}'; using '{}'",
                self.default_format, defaults.default_format
            );
            self.default_format = defaults.default_format;
        }

        if !["asc", "desc"].contains(&self.sort_order.as_str()) {
            eprintln!(
                "[tokemon] Warning: invalid sort_order '{}'; using '{}'",
                self.sort_order, defaults.sort_order
            );
            self.sort_order = defaults.sort_order;
        }

        self
    }

    /// Write the default config to disk (for `tokemon init`)
    pub fn write_default() -> anyhow::Result<PathBuf> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let default = Self::default();
        let content = toml::to_string_pretty(&default)?;
        let header = "# Tokemon configuration\n\
                      # Location: ~/.config/tokemon/config.toml\n\
                      #\n\
                      # Changes here affect default behavior.\n\
                      # CLI flags always override config values.\n\n";
        fs::write(&path, format!("{}{}", header, content))?;
        Ok(path)
    }

    pub fn config_path() -> PathBuf {
        let config_dir = directories::ProjectDirs::from("", "", "tokemon")
            .map(|d| d.config_dir().to_path_buf())
            .unwrap_or_else(|| paths::home_dir().join(".config/tokemon"));
        config_dir.join(CONFIG_FILENAME)
    }
}
