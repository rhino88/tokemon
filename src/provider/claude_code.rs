use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Result, TokemonError};
use crate::parse_utils;
use crate::paths;
use crate::types::UsageEntry;

pub struct ClaudeCodeProvider {
    base_dir: PathBuf,
}

impl ClaudeCodeProvider {
    pub fn new() -> Self {
        Self {
            base_dir: paths::home_dir().join(".claude/projects"),
        }
    }
}

#[derive(Deserialize)]
struct ClaudeLine {
    #[serde(rename = "type")]
    line_type: Option<String>,
    timestamp: Option<String>,
    #[serde(rename = "requestId")]
    request_id: Option<String>,
    message: Option<ClaudeMessage>,
}

#[derive(Deserialize)]
struct ClaudeMessage {
    model: Option<String>,
    id: Option<String>,
    usage: Option<ClaudeUsage>,
}

#[derive(Deserialize)]
struct ClaudeUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
}

impl super::Provider for ClaudeCodeProvider {
    fn name(&self) -> &str {
        "claude-code"
    }

    fn display_name(&self) -> &str {
        "Claude Code"
    }

    fn data_dir(&self) -> PathBuf {
        self.base_dir.clone()
    }

    fn discover_files(&self) -> Vec<PathBuf> {
        let pattern = self.base_dir.join("**/*.jsonl").display().to_string();
        glob::glob(&pattern)
            .map(|paths| paths.filter_map(|p| p.ok()).collect())
            .unwrap_or_default()
    }

    fn parse_file(&self, path: &Path) -> Result<Vec<UsageEntry>> {
        let file = fs::File::open(path).map_err(TokemonError::Io)?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();

        // Extract session_id from filename
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

            // Use serde_json for robust parsing (simd-json needs mutable bytes)
            let parsed: ClaudeLine = match serde_json::from_str(&line) {
                Ok(p) => p,
                Err(_) => continue,
            };

            // Only process assistant messages
            match parsed.line_type.as_deref() {
                Some("assistant") => {}
                _ => continue,
            }

            let message = match parsed.message {
                Some(m) => m,
                None => continue,
            };

            let usage = match message.usage {
                Some(u) => u,
                None => continue,
            };

            // Skip synthetic model entries
            if message.model.as_deref() == Some("<synthetic>") {
                continue;
            }

            let timestamp = match parsed.timestamp.as_deref().and_then(parse_utils::parse_timestamp) {
                Some(dt) => dt,
                None => continue,
            };

            entries.push(UsageEntry {
                timestamp,
                provider: "claude-code".to_string(),
                model: message.model,
                input_tokens: usage.input_tokens.unwrap_or(0),
                output_tokens: usage.output_tokens.unwrap_or(0),
                cache_read_tokens: usage.cache_read_input_tokens.unwrap_or(0),
                cache_creation_tokens: usage.cache_creation_input_tokens.unwrap_or(0),
                thinking_tokens: 0,
                cost_usd: None,
                message_id: message.id,
                request_id: parsed.request_id,
                session_id: session_id.clone(),
            });
        }

        Ok(entries)
    }
}
