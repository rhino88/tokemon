use std::fs;
use std::path::{Path, PathBuf};

use chrono::DateTime;
use serde::Deserialize;

use crate::error::{Result, TokemonError};
use crate::paths;
use crate::types::UsageEntry;

pub struct GeminiProvider {
    base_dir: PathBuf,
}

impl GeminiProvider {
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

impl super::Provider for GeminiProvider {
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
            format!("{}/**/chats/session-*.json", self.base_dir.join("tmp").display()),
            format!("{}/**/session.json", self.base_dir.join("tmp").display()),
        ];

        let mut files = Vec::new();
        for pattern in &patterns {
            if let Ok(paths) = glob::glob(pattern) {
                files.extend(paths.filter_map(|p| p.ok()));
            }
        }
        files
    }

    fn parse_file(&self, path: &Path) -> Result<Vec<UsageEntry>> {
        let content = fs::read_to_string(path).map_err(TokemonError::Io)?;
        let session: GeminiSession = serde_json::from_str(&content).map_err(|e| {
            TokemonError::JsonParse {
                file: path.display().to_string(),
                source: e,
            }
        })?;

        let messages = match session.messages {
            Some(m) => m,
            None => return Ok(Vec::new()),
        };

        let session_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());

        let mut entries = Vec::new();

        for msg in messages {
            // Only process Gemini AI response messages
            let msg_type = msg.msg_type.as_deref().unwrap_or("");
            if !msg_type.eq_ignore_ascii_case("gemini")
                && !msg_type.eq_ignore_ascii_case("model")
                && !msg_type.eq_ignore_ascii_case("assistant")
            {
                continue;
            }

            let tokens = match msg.tokens {
                Some(t) => t,
                None => continue,
            };

            let timestamp = match &msg.timestamp {
                Some(ts) => match DateTime::parse_from_rfc3339(ts) {
                    Ok(dt) => dt.to_utc(),
                    Err(_) => {
                        // Try parsing as milliseconds timestamp
                        if let Ok(ms) = ts.parse::<i64>() {
                            match DateTime::from_timestamp_millis(ms) {
                                Some(dt) => dt,
                                None => continue,
                            }
                        } else {
                            continue
                        }
                    }
                },
                None => continue,
            };

            entries.push(UsageEntry {
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
            });
        }

        Ok(entries)
    }
}
