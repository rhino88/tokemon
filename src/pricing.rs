use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use serde::Deserialize;

use crate::error::{Result, TokemonError};
use crate::paths;
use crate::types::UsageEntry;

const PRICING_URL: &str =
    "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";
const CACHE_TTL_SECS: u64 = 3600; // 1 hour

#[derive(Debug, Clone, Deserialize)]
pub struct ModelPricing {
    pub input_cost_per_token: Option<f64>,
    pub output_cost_per_token: Option<f64>,
    #[serde(alias = "cache_read_input_token_cost")]
    pub cache_read_cost: Option<f64>,
    #[serde(alias = "cache_creation_input_token_cost")]
    pub cache_creation_cost: Option<f64>,
}

pub struct PricingEngine {
    models: HashMap<String, ModelPricing>,
}

impl PricingEngine {
    pub fn load(offline: bool) -> Result<Self> {
        let cache_path = Self::cache_path();

        // Check if cache is fresh
        if let Some(data) = Self::read_cache(&cache_path) {
            return Self::parse_pricing(&data);
        }

        if offline {
            eprintln!("[tokemon] Warning: no cached pricing data and --offline specified; costs will be $0.00");
            return Ok(Self {
                models: HashMap::new(),
            });
        }

        // Fetch from remote
        match Self::fetch_remote() {
            Ok(data) => {
                // Save to cache
                if let Some(parent) = cache_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let _ = fs::write(&cache_path, &data);
                Self::parse_pricing(&data)
            }
            Err(e) => {
                eprintln!("[tokemon] Warning: failed to fetch pricing: {}; costs will be $0.00", e);
                Ok(Self {
                    models: HashMap::new(),
                })
            }
        }
    }

    pub fn calculate_cost(&self, entry: &UsageEntry) -> f64 {
        // If entry already has a cost, use it
        if let Some(cost) = entry.cost_usd {
            if cost > 0.0 {
                return cost;
            }
        }

        let model = match &entry.model {
            Some(m) if !m.is_empty() => m.as_str(),
            _ => return 0.0,
        };

        let pricing = match self.find_pricing(model) {
            Some(p) => p,
            None => return 0.0,
        };

        let mut cost = 0.0;
        cost += entry.input_tokens as f64 * pricing.input_cost_per_token.unwrap_or(0.0);
        cost += entry.output_tokens as f64 * pricing.output_cost_per_token.unwrap_or(0.0);
        cost += entry.cache_read_tokens as f64 * pricing.cache_read_cost.unwrap_or(0.0);
        cost += entry.cache_creation_tokens as f64 * pricing.cache_creation_cost.unwrap_or(0.0);
        // thinking_tokens charged at output rate
        cost += entry.thinking_tokens as f64 * pricing.output_cost_per_token.unwrap_or(0.0);
        cost
    }

    /// Apply costs to all entries in-place
    pub fn apply_costs(&self, entries: &mut [UsageEntry]) {
        for entry in entries.iter_mut() {
            let cost = self.calculate_cost(entry);
            entry.cost_usd = Some(cost);
        }
    }

    /// Three-level model matching
    fn find_pricing(&self, model: &str) -> Option<&ModelPricing> {
        // 1. Exact match
        if let Some(p) = self.models.get(model) {
            return Some(p);
        }

        // 2. Normalized match (strip date suffix, lowercase)
        let normalized = normalize_model_name(model);
        if let Some(p) = self.models.get(&normalized) {
            return Some(p);
        }

        // 3. Try with common provider prefixes
        let prefixed_variants = [
            format!("anthropic/{}", model),
            format!("anthropic/{}", normalized),
            format!("openai/{}", model),
            format!("openai/{}", normalized),
            format!("google/{}", model),
            format!("google/{}", normalized),
            format!("vertex_ai/{}", model),
            format!("vertex_ai/{}", normalized),
        ];
        for variant in &prefixed_variants {
            if let Some(p) = self.models.get(variant.as_str()) {
                return Some(p);
            }
        }

        // 4. Prefix match - longest match wins, requires word boundary
        let mut best_match: Option<&ModelPricing> = None;
        let mut best_len: usize = 0;

        for (key, pricing) in &self.models {
            let plain_key = key.split('/').last().unwrap_or(key);
            let norm_key = normalize_model_name(plain_key);

            // Only match if our model starts with the pricing key
            // AND the match ends at a word boundary (delimiter or end of string)
            if normalized.starts_with(&norm_key) && norm_key.len() > best_len {
                let at_boundary = normalized.len() == norm_key.len()
                    || matches!(
                        normalized.as_bytes().get(norm_key.len()),
                        Some(b'-' | b'_' | b'.')
                    );
                if at_boundary {
                    best_match = Some(pricing);
                    best_len = norm_key.len();
                }
            }
        }

        best_match
    }

    fn cache_path() -> PathBuf {
        paths::cache_dir().join("pricing.json")
    }

    fn read_cache(path: &PathBuf) -> Option<String> {
        let metadata = fs::metadata(path).ok()?;
        let modified = metadata.modified().ok()?;
        let age = SystemTime::now().duration_since(modified).ok()?;
        if age.as_secs() > CACHE_TTL_SECS {
            return None;
        }
        fs::read_to_string(path).ok()
    }

    fn fetch_remote() -> Result<String> {
        let resp = reqwest::blocking::get(PRICING_URL)
            .map_err(|e| TokemonError::PricingFetch(e.to_string()))?;
        let text = resp
            .text()
            .map_err(|e| TokemonError::PricingFetch(e.to_string()))?;
        Ok(text)
    }

    fn parse_pricing(data: &str) -> Result<Self> {
        let models: HashMap<String, ModelPricing> = serde_json::from_str(data).map_err(|e| {
            TokemonError::PricingFetch(format!("failed to parse pricing JSON: {}", e))
        })?;
        Ok(Self { models })
    }
}

fn normalize_model_name(model: &str) -> String {
    let s = model.to_lowercase();
    // Strip date suffixes like -20250805, -20241022
    let re_date = regex_strip_date(&s);
    re_date.replace('.', "-")
}

/// Strip trailing date patterns like -YYYYMMDD
fn regex_strip_date(s: &str) -> String {
    // Match patterns like -20250805, -20241022 at end of string
    if s.len() >= 9 {
        let last_9 = &s[s.len() - 9..];
        if last_9.starts_with('-')
            && last_9[1..].chars().all(|c| c.is_ascii_digit())
            && last_9[1..].len() == 8
        {
            return s[..s.len() - 9].to_string();
        }
    }
    s.to_string()
}
