use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Result, TokemonError};
use crate::paths;
use crate::types::Record;

pub struct CursorSource {
    base_dir: PathBuf,
}

impl CursorSource {
    pub fn new() -> Self {
        Self {
            base_dir: paths::home_dir().join(".config/tokscale/cursor-cache"),
        }
    }
}

impl super::Source for CursorSource {
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

    fn parse_file(&self, path: &Path) -> Result<Vec<Record>> {
        let content = fs::read_to_string(path).map_err(TokemonError::Io)?;

        let entries = content
            .lines()
            .skip(1) // Skip header row
            .filter_map(|line| {
                let fields: Vec<&str> = line.split(',').collect();
                if fields.len() < 5 {
                    return None;
                }

                let timestamp = crate::timestamp::parse_timestamp(fields[0].trim())?;
                let model = fields[1].trim();

                Some(Record {
                    timestamp,
                    provider: "cursor".to_string(),
                    model: if model.is_empty() { None } else { Some(model.to_string()) },
                    input_tokens: fields[2].trim().parse().unwrap_or(0),
                    output_tokens: fields[3].trim().parse().unwrap_or(0),
                    cache_read_tokens: 0,
                    cache_creation_tokens: 0,
                    thinking_tokens: 0,
                    cost_usd: fields[4].trim().parse().ok(),
                    message_id: None,
                    request_id: None,
                    session_id: None,
                })
            })
            .collect();

        Ok(entries)
    }
}
