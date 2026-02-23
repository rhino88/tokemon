use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Result, TokemonError};
use crate::timestamp;
use crate::paths;
use crate::types::Record;

pub struct OpenCodeSource {
    dirs: Vec<PathBuf>,
}

impl OpenCodeSource {
    pub fn new() -> Self {
        let home = paths::home_dir();
        Self {
            dirs: vec![
                home.join(".local/share/opencode/storage/message"),
                home.join(".opencode/message"),
            ],
        }
    }
}

#[derive(Deserialize)]
struct OpenCodeMessage {
    model: Option<String>,
    role: Option<String>,
    timestamp: Option<String>,
    usage: Option<OpenCodeUsage>,
}

#[derive(Deserialize)]
struct OpenCodeUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    cache_creation_tokens: Option<u64>,
}

impl super::Source for OpenCodeSource {
    fn name(&self) -> &str {
        "opencode"
    }

    fn display_name(&self) -> &str {
        "OpenCode"
    }

    fn data_dir(&self) -> PathBuf {
        self.dirs.first().cloned().unwrap_or_default()
    }

    fn discover_files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for dir in &self.dirs {
            let pattern = dir.join("**/msg_*.json").display().to_string();
            if let Ok(paths) = glob::glob(&pattern) {
                files.extend(paths.filter_map(|p| p.ok()));
            }
        }
        files
    }

    fn parse_file(&self, path: &Path) -> Result<Vec<Record>> {
        let content = fs::read_to_string(path).map_err(TokemonError::Io)?;
        let msg: OpenCodeMessage =
            serde_json::from_str(&content).map_err(|e| TokemonError::JsonParse {
                file: path.display().to_string(),
                source: e,
            })?;

        // Only process assistant/model messages
        let role = msg.role.as_deref().unwrap_or("");
        if role != "assistant" && role != "model" {
            return Ok(Vec::new());
        }

        let usage = match msg.usage {
            Some(u) => u,
            None => return Ok(Vec::new()),
        };

        let timestamp = match msg.timestamp.as_deref().and_then(timestamp::parse_timestamp) {
            Some(dt) => dt,
            None => return Ok(Vec::new()),
        };

        let session_id = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .map(String::from);

        Ok(vec![Record {
            timestamp,
            provider: "opencode".to_string(),
            model: msg.model,
            input_tokens: usage.input_tokens.unwrap_or(0),
            output_tokens: usage.output_tokens.unwrap_or(0),
            cache_read_tokens: usage.cache_read_tokens.unwrap_or(0),
            cache_creation_tokens: usage.cache_creation_tokens.unwrap_or(0),
            thinking_tokens: 0,
            cost_usd: None,
            message_id: None,
            request_id: None,
            session_id,
        }])
    }
}
