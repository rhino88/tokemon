use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Result, TokemonError};
use crate::timestamp;
use crate::paths;
use crate::types::Record;

pub struct GeminiSource {
    base_dir: PathBuf,
}

impl GeminiSource {
    pub fn new() -> Self {
        Self {
            base_dir: paths::home_dir().join(".gemini"),
        }
    }
}

#[derive(Deserialize)]
struct GeminiSession {
    messages: Option<Vec<GeminiMessage>>,
}

#[derive(Deserialize)]
struct GeminiMessage {
    #[serde(rename = "type")]
    msg_type: Option<String>,
    model: Option<String>,
    timestamp: Option<String>,
    tokens: Option<GeminiTokens>,
}

#[derive(Deserialize)]
struct GeminiTokens {
    input: Option<u64>,
    output: Option<u64>,
    cached: Option<u64>,
    thoughts: Option<u64>,
}

impl super::Source for GeminiSource {
    fn name(&self) -> &str {
        "gemini"
    }

    fn display_name(&self) -> &str {
        "Gemini CLI"
    }

    fn data_dir(&self) -> PathBuf {
        self.base_dir.clone()
    }

    fn discover_files(&self) -> Vec<PathBuf> {
        // Check both patterns
        let patterns = [
            self.base_dir.join("tmp/**/chats/session-*.json").display().to_string(),
            self.base_dir.join("tmp/**/session.json").display().to_string(),
        ];

        let mut files = Vec::new();
        for pattern in &patterns {
            if let Ok(paths) = glob::glob(pattern) {
                files.extend(paths.filter_map(|p| p.ok()));
            }
        }
        files
    }

    fn parse_file(&self, path: &Path) -> Result<Vec<Record>> {
        let content = fs::read_to_string(path).map_err(TokemonError::Io)?;
        let session: GeminiSession = serde_json::from_str(&content).map_err(|e| {
            TokemonError::JsonParse {
                file: path.display().to_string(),
                source: e,
            }
        })?;

        let Some(messages) = session.messages else {
            return Ok(Vec::new());
        };

        let session_id = timestamp::extract_session_id(path);

        let entries = messages
            .into_iter()
            .filter(|msg| {
                let t = msg.msg_type.as_deref().unwrap_or("");
                t.eq_ignore_ascii_case("gemini")
                    || t.eq_ignore_ascii_case("model")
                    || t.eq_ignore_ascii_case("assistant")
            })
            .filter_map(|msg| {
                let tokens = msg.tokens?;
                let timestamp = msg
                    .timestamp
                    .as_deref()
                    .and_then(timestamp::parse_timestamp)?;

                Some(Record {
                    timestamp,
                    provider: "gemini".to_string(),
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
