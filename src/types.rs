use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

/// A single usage entry from any provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageEntry {
    pub timestamp: DateTime<Utc>,
    pub provider: String,
    pub model: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub thinking_tokens: u64,
    pub cost_usd: Option<f64>,
    pub message_id: Option<String>,
    pub request_id: Option<String>,
    pub session_id: Option<String>,
}

impl UsageEntry {
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens
            + self.output_tokens
            + self.cache_read_tokens
            + self.cache_creation_tokens
            + self.thinking_tokens
    }

    /// Generate dedup hash from message_id and request_id.
    /// Returns None if neither field is present (entry kept unconditionally).
    pub fn dedup_key(&self) -> Option<String> {
        match (&self.message_id, &self.request_id) {
            (Some(msg), Some(req)) => Some(format!("{}:{}", msg, req)),
            (Some(msg), None) => {
                let model = self.model.as_deref().unwrap_or("unknown");
                Some(format!(
                    "{}:{}:{}:{}",
                    msg, model, self.input_tokens, self.output_tokens
                ))
            }
            _ => None,
        }
    }
}

/// Aggregated usage for a single model within a time period
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelUsage {
    pub model: String,
    pub provider: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub thinking_tokens: u64,
    pub cost_usd: f64,
    pub request_count: u64,
}

/// Summary for a time period (day, week, or month)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailySummary {
    pub date: NaiveDate,
    pub label: String,
    pub models: Vec<ModelUsage>,
    pub total_input: u64,
    pub total_output: u64,
    pub total_cache: u64,
    pub total_thinking: u64,
    pub total_cost: f64,
    pub total_requests: u64,
}

/// Full report structure (serializable to JSON)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub period: String,
    pub generated_at: DateTime<Utc>,
    pub providers_found: Vec<String>,
    pub summaries: Vec<DailySummary>,
    pub total_cost: f64,
    pub total_tokens: u64,
}
