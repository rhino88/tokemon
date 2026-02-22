use std::fs;
use std::io::{BufRead, BufReader};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use chrono::DateTime;
use serde::Deserialize;

use crate::error::{Result, TokemonError};
use crate::types::UsageEntry;

/// Configuration trait for generic JSONL providers.
///
/// Implement this with a zero-sized type to define a new provider:
/// ```ignore
/// pub struct KimiConfig;
/// impl JsonlProviderConfig for KimiConfig {
///     const NAME: &'static str = "kimi";
///     const DISPLAY_NAME: &'static str = "Kimi";
///     fn base_dir() -> PathBuf { paths::home_dir().join(".kimi/sessions") }
/// }
/// pub type KimiProvider = GenericJsonlProvider<KimiConfig>;
/// ```
pub trait JsonlProviderConfig: Send + Sync + 'static {
    const NAME: &'static str;
    const DISPLAY_NAME: &'static str;
    const HAS_CACHE_TOKENS: bool = false;
    const HAS_REQUEST_IDS: bool = false;
    fn base_dir() -> PathBuf;
}

/// Generic JSONL provider that handles the common pattern of:
/// - Reading `**/*.jsonl` files from a base directory
/// - Parsing lines with `type`, `timestamp`, `model`, `usage` fields
/// - Filtering for "assistant" or "response" type lines
/// - Extracting token counts from usage data
pub struct GenericJsonlProvider<C: JsonlProviderConfig> {
    base_dir: PathBuf,
    _config: PhantomData<C>,
}

impl<C: JsonlProviderConfig> GenericJsonlProvider<C> {
    pub fn new() -> Self {
        Self {
            base_dir: C::base_dir(),
            _config: PhantomData,
        }
    }
}

/// Unified deserialization struct for all JSONL providers.
/// Fields not present in the JSON are deserialized as None.
#[derive(Deserialize)]
struct JsonlLine {
    #[serde(rename = "type")]
    line_type: Option<String>,
    timestamp: Option<String>,
    model: Option<String>,
    usage: Option<JsonlUsage>,
    #[serde(rename = "requestId")]
    request_id: Option<String>,
    #[serde(rename = "messageId")]
    message_id: Option<String>,
}

#[derive(Deserialize)]
struct JsonlUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    cache_creation_tokens: Option<u64>,
}

impl<C: JsonlProviderConfig> super::Provider for GenericJsonlProvider<C> {
    fn name(&self) -> &str {
        C::NAME
    }

    fn display_name(&self) -> &str {
        C::DISPLAY_NAME
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

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => continue,
            };

            if line.trim().is_empty() {
                continue;
            }

            let parsed: JsonlLine = match serde_json::from_str(&line) {
                Ok(p) => p,
                Err(_) => continue,
            };

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
                provider: C::NAME.to_string(),
                model: parsed.model,
                input_tokens: usage.input_tokens.unwrap_or(0),
                output_tokens: usage.output_tokens.unwrap_or(0),
                cache_read_tokens: if C::HAS_CACHE_TOKENS {
                    usage.cache_read_tokens.unwrap_or(0)
                } else {
                    0
                },
                cache_creation_tokens: if C::HAS_CACHE_TOKENS {
                    usage.cache_creation_tokens.unwrap_or(0)
                } else {
                    0
                },
                thinking_tokens: 0,
                cost_usd: None,
                message_id: if C::HAS_REQUEST_IDS {
                    parsed.message_id
                } else {
                    None
                },
                request_id: if C::HAS_REQUEST_IDS {
                    parsed.request_id
                } else {
                    None
                },
                session_id: session_id.clone(),
            });
        }

        Ok(entries)
    }
}
