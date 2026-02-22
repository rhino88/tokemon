use thiserror::Error;

#[derive(Error, Debug)]
pub enum TokemonError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error in {file}: {source}")]
    JsonParse {
        file: String,
        source: serde_json::Error,
    },

    #[error("SIMD JSON parse error in {file}: {message}")]
    SimdJsonParse { file: String, message: String },

    #[error("Provider '{0}' not found")]
    ProviderNotFound(String),

    #[error("No data directory found for provider '{0}' at {1}")]
    NoDataDir(String, String),

    #[error("Failed to fetch pricing data: {0}")]
    PricingFetch(String),

    #[error("Invalid date format: {0}")]
    InvalidDate(String),
}

pub type Result<T> = std::result::Result<T, TokemonError>;
