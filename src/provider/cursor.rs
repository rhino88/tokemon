use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Result, TokemonError};
use crate::paths;
use crate::types::UsageEntry;

pub struct CursorProvider {
    base_dir: PathBuf,
}

impl CursorProvider {
    pub fn new() -> Self {
        Self {
            base_dir: paths::home_dir().join(".config/tokscale/cursor-cache"),
        }
    }
}

impl super::Provider for CursorProvider {
    fn name(&self) -> &str {
        "cursor"
    }

    fn display_name(&self) -> &str {
        "Cursor"
    }

    fn data_dir(&self) -> PathBuf {
        self.base_dir.clone()
    }

    fn discover_files(&self) -> Vec<PathBuf> {
        let pattern = self.base_dir.join("usage*.csv").display().to_string();
        glob::glob(&pattern)
            .map(|paths| paths.filter_map(|p| p.ok()).collect())
            .unwrap_or_default()
    }

    fn parse_file(&self, path: &Path) -> Result<Vec<UsageEntry>> {
        let content = fs::read_to_string(path).map_err(TokemonError::Io)?;
        let mut entries = Vec::new();

        for (i, line) in content.lines().enumerate() {
            // Skip header row
            if i == 0 {
                continue;
            }

            let fields: Vec<&str> = line.split(',').collect();
            // Expected CSV format: timestamp, model, input_tokens, output_tokens, cost
            if fields.len() < 5 {
                continue;
            }

            let timestamp = match crate::parse_utils::parse_timestamp(fields[0].trim()) {
                Some(dt) => dt,
                None => continue,
            };

            let model = fields[1].trim().to_string();
            let input_tokens = fields[2].trim().parse::<u64>().unwrap_or(0);
            let output_tokens = fields[3].trim().parse::<u64>().unwrap_or(0);
            let cost = fields[4].trim().parse::<f64>().ok();

            entries.push(UsageEntry {
                timestamp,
                provider: "cursor".to_string(),
                model: if model.is_empty() {
                    None
                } else {
                    Some(model)
                },
                input_tokens,
                output_tokens,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
                thinking_tokens: 0,
                cost_usd: cost,
                message_id: None,
                request_id: None,
                session_id: None,
            });
        }

        Ok(entries)
    }
}
