use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Result, TokemonError};
use crate::timestamp;
use crate::paths;
use crate::types::Record;

pub struct QwenSource {
    base_dir: PathBuf,
}

impl QwenSource {
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

impl super::Source for QwenSource {
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

    fn parse_file(&self, path: &Path) -> Result<Vec<Record>> {
        let content = fs::read_to_string(path).map_err(TokemonError::Io)?;
        let session: QwenSession =
            serde_json::from_str(&content).map_err(|e| TokemonError::JsonParse {
                file: path.display().to_string(),
                source: e,
            })?;

        let Some(messages) = session.messages else {
            return Ok(Vec::new());
        };

        // Qwen uses parent dir as session ID (file is always session.json)
        let session_id = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .map(String::from);

        let entries = messages
            .into_iter()
            .filter(|msg| {
                let t = msg.msg_type.as_deref().unwrap_or("");
                t.eq_ignore_ascii_case("assistant") || t.eq_ignore_ascii_case("model")
            })
            .filter_map(|msg| {
                let tokens = msg.tokens?;
                let timestamp = msg
                    .timestamp
                    .as_deref()
                    .and_then(timestamp::parse_timestamp)?;

                Some(Record {
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
                })
            })
            .collect();

        Ok(entries)
    }
}
