use std::path::PathBuf;

use crate::paths;

use super::jsonl_source::{JsonlSource, JsonlSourceConfig};

pub struct AmpConfig;

impl JsonlSourceConfig for AmpConfig {
    const NAME: &'static str = "amp";
    const DISPLAY_NAME: &'static str = "Amp";
    const HAS_CACHE_TOKENS: bool = true;
    const HAS_REQUEST_IDS: bool = true;
    fn base_dir() -> PathBuf {
        paths::home_dir().join(".local/share/amp/threads")
    }
}

pub type AmpSource = JsonlSource<AmpConfig>;
