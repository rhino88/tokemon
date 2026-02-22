use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use chrono::DateTime;
use serde::Deserialize;

use crate::error::{Result, TokemonError};
use crate::paths;
use crate::types::UsageEntry;

pub struct AmpProvider {
    base_dir: PathBuf,
}

impl AmpProvider {
    pub fn new() -> Self {
        Self {
            base_dir: paths::home_dir().join(".local/share/amp/threads"),
        }
    }
}

#[derive(Deserialize)]
struct AmpLine {
    #[serde(rename = "type")]
    line_type: Option<String>,
    timestamp: Option<String>,
    model: Option<String>,
    usage: Option<AmpUsage>,
    #[serde(rename = "requestId")]
    request_id: Option<String>,
    #[serde(rename = "messageId")]
    message_id: Option<String>,
}

#[derive(Deserialize)]
struct AmpUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    cache_creation_tokens: Option<u64>,
}

impl super::Provider for AmpProvider {
    fn name(&self) -> &str {
        "amp"
    }

    fn display_name(&self) -> &str {
        "Amp"
    }

    fn data_dir(&self) -> PathBuf {
        self.base_dir.clone()
    }

    fn discover_files(&self) -> Vec<PathBuf> {
        let pattern = format!("{}/**/*.jsonl", self.base_dir.display());
        glob::glob(&pattern)
            .map(|paths| paths.filter_map(|p| p.ok()).collect())
            .unwrap_or_default()
    }

    fn parse_file(&self, path: &Path) -> Result<Vec<UsageEntry>> {
        let file = fs::File::open(path).map_err(TokemonError::Io)?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();

        let session_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => continue,
            };

            if line.trim().is_empty() {
                continue;
            }

            let parsed: AmpLine = match serde_json::from_str(&line) {
                Ok(p) => p,
                Err(_) => continue,
            };

            // Only process assistant/response messages with usage data
            let line_type = parsed.line_type.as_deref().unwrap_or("");
            if line_type != "assistant" && line_type != "response" {
                continue;
            }

            let usage = match parsed.usage {
                Some(u) => u,
                None => continue,
            };

            let timestamp = match &parsed.timestamp {
                Some(ts) => match DateTime::parse_from_rfc3339(ts) {
                    Ok(dt) => dt.to_utc(),
                    Err(_) => continue,
                },
                None => continue,
            };

            entries.push(UsageEntry {
                timestamp,
                provider: "amp".to_string(),
                model: parsed.model,
                input_tokens: usage.input_tokens.unwrap_or(0),
                output_tokens: usage.output_tokens.unwrap_or(0),
                cache_read_tokens: usage.cache_read_tokens.unwrap_or(0),
                cache_creation_tokens: usage.cache_creation_tokens.unwrap_or(0),
                thinking_tokens: 0,
                cost_usd: None,
                message_id: parsed.message_id,
                request_id: parsed.request_id,
                session_id: session_id.clone(),
            });
        }

        Ok(entries)
    }
}
