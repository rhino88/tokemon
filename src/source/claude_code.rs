use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Result, TokemonError};
use crate::timestamp;
use crate::paths;
use crate::types::Record;

pub struct ClaudeCodeSource {
    base_dir: PathBuf,
}

impl ClaudeCodeSource {
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

impl super::Source for ClaudeCodeSource {
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

    fn parse_file(&self, path: &Path) -> Result<Vec<Record>> {
        let file = fs::File::open(path).map_err(TokemonError::Io)?;
        let reader = BufReader::new(file);
        let session_id = timestamp::extract_session_id(path);

        let entries = reader
            .lines()
            .filter_map(|line| line.ok())
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| serde_json::from_str::<ClaudeLine>(&line).ok())
            .filter(|parsed| parsed.line_type.as_deref() == Some("assistant"))
            .filter_map(|parsed| {
                let message = parsed.message?;
                if message.model.as_deref() == Some("<synthetic>") {
                    return None;
                }
                let usage = message.usage?;
                let timestamp = parsed
                    .timestamp
                    .as_deref()
                    .and_then(timestamp::parse_timestamp)?;

                Some(Record {
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
                })
            })
            .collect();

        Ok(entries)
    }
}
