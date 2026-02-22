use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Result, TokemonError};
use crate::parse_utils;
use crate::paths;
use crate::types::UsageEntry;

pub struct CodexProvider {
    base_dir: PathBuf,
}

impl CodexProvider {
    pub fn new() -> Self {
        Self {
            base_dir: paths::home_dir().join(".codex/sessions"),
        }
    }
}

#[derive(Deserialize)]
struct CodexLine {
    #[serde(rename = "type")]
    line_type: Option<String>,
    timestamp: Option<String>,
    payload: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct TokenUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cached_input_tokens: Option<u64>,
}

impl super::Provider for CodexProvider {
    fn name(&self) -> &str {
        "codex"
    }

    fn display_name(&self) -> &str {
        "Codex CLI"
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

        let session_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());

        // State machine: track current model from turn_context lines
        let mut current_model: Option<String> = None;

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => continue,
            };

            if line.trim().is_empty() {
                continue;
            }

            let parsed: CodexLine = match serde_json::from_str(&line) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let line_type = match &parsed.line_type {
                Some(t) => t.as_str(),
                None => continue,
            };

            match line_type {
                "turn_context" => {
                    // Extract model from payload
                    if let Some(payload) = &parsed.payload {
                        if let Some(model) = payload.get("model").and_then(|m| m.as_str()) {
                            current_model = Some(model.to_string());
                        }
                    }
                }
                "event_msg" => {
                    let payload = match &parsed.payload {
                        Some(p) => p,
                        None => continue,
                    };

                    // Check if this is a token_count event
                    let payload_type = payload.get("type").and_then(|t| t.as_str());
                    if payload_type != Some("token_count") {
                        continue;
                    }

                    let info = match payload.get("info") {
                        Some(i) => i,
                        None => continue,
                    };

                    // Try last_token_usage first, then total_token_usage
                    let usage_val = info
                        .get("last_token_usage")
                        .or_else(|| info.get("total_token_usage"));

                    let usage: TokenUsage = match usage_val {
                        Some(v) => match serde_json::from_value(v.clone()) {
                            Ok(u) => u,
                            Err(_) => continue,
                        },
                        None => continue,
                    };

                    let timestamp = match parsed.timestamp.as_deref().and_then(parse_utils::parse_timestamp) {
                        Some(dt) => dt,
                        None => continue,
                    };

                    let raw_input = usage.input_tokens.unwrap_or(0);
                    let cached = usage.cached_input_tokens.unwrap_or(0);
                    // Codex input_tokens includes cached tokens
                    let actual_input = if cached > raw_input {
                        eprintln!(
                            "[tokemon] Warning: cached tokens ({}) > input tokens ({}) in {}",
                            cached, raw_input, path.display()
                        );
                        0
                    } else {
                        raw_input - cached
                    };

                    entries.push(UsageEntry {
                        timestamp,
                        provider: "codex".to_string(),
                        model: current_model.clone(),
                        input_tokens: actual_input,
                        output_tokens: usage.output_tokens.unwrap_or(0),
                        cache_read_tokens: cached,
                        cache_creation_tokens: 0,
                        thinking_tokens: 0,
                        cost_usd: None,
                        message_id: None,
                        request_id: None,
                        session_id: session_id.clone(),
                    });
                }
                _ => {}
            }
        }

        Ok(entries)
    }
}
