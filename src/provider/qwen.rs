use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Result, TokemonError};
use crate::parse_utils;
use crate::paths;
use crate::types::UsageEntry;

pub struct QwenProvider {
    base_dir: PathBuf,
}

impl QwenProvider {
    pub fn new() -> Self {
        Self {
            base_dir: paths::home_dir().join(".qwen"),
        }
    }
}

// Reuses Gemini-compatible format
#[derive(Deserialize)]
struct QwenSession {
    messages: Option<Vec<QwenMessage>>,
}

#[derive(Deserialize)]
struct QwenMessage {
    #[serde(rename = "type")]
    msg_type: Option<String>,
    model: Option<String>,
    timestamp: Option<String>,
    tokens: Option<QwenTokens>,
}

#[derive(Deserialize)]
struct QwenTokens {
    input: Option<u64>,
    output: Option<u64>,
    cached: Option<u64>,
    thoughts: Option<u64>,
}

impl super::Provider for QwenProvider {
    fn name(&self) -> &str {
        "qwen"
    }

    fn display_name(&self) -> &str {
        "Qwen Code"
    }

    fn data_dir(&self) -> PathBuf {
        self.base_dir.clone()
    }

    fn discover_files(&self) -> Vec<PathBuf> {
        let pattern = self.base_dir.join("tmp/**/session.json").display().to_string();
        glob::glob(&pattern)
            .map(|paths| paths.filter_map(|p| p.ok()).collect())
            .unwrap_or_default()
    }

    fn parse_file(&self, path: &Path) -> Result<Vec<UsageEntry>> {
        let content = fs::read_to_string(path).map_err(TokemonError::Io)?;
        let session: QwenSession =
            serde_json::from_str(&content).map_err(|e| TokemonError::JsonParse {
                file: path.display().to_string(),
                source: e,
            })?;

        let messages = match session.messages {
            Some(m) => m,
            None => return Ok(Vec::new()),
        };

        let session_id = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());

        let mut entries = Vec::new();

        for msg in messages {
            let msg_type = msg.msg_type.as_deref().unwrap_or("");
            if !msg_type.eq_ignore_ascii_case("assistant")
                && !msg_type.eq_ignore_ascii_case("model")
            {
                continue;
            }

            let tokens = match msg.tokens {
                Some(t) => t,
                None => continue,
            };

            let timestamp = match msg.timestamp.as_deref().and_then(parse_utils::parse_timestamp) {
                Some(dt) => dt,
                None => continue,
            };

            entries.push(UsageEntry {
                timestamp,
                provider: "qwen".to_string(),
                model: msg.model,
                input_tokens: tokens.input.unwrap_or(0),
                output_tokens: tokens.output.unwrap_or(0),
                cache_read_tokens: tokens.cached.unwrap_or(0),
                cache_creation_tokens: 0,
                thinking_tokens: tokens.thoughts.unwrap_or(0),
                cost_usd: None,
                message_id: None,
                request_id: None,
                session_id: session_id.clone(),
            });
        }

        Ok(entries)
    }
}
