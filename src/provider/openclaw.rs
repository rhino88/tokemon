use std::path::PathBuf;

use crate::paths;

use super::jsonl_provider::{GenericJsonlProvider, JsonlProviderConfig};

pub struct OpenClawConfig;

impl JsonlProviderConfig for OpenClawConfig {
    const NAME: &'static str = "openclaw";
    const DISPLAY_NAME: &'static str = "OpenClaw";
    const HAS_CACHE_TOKENS: bool = true;
    const HAS_REQUEST_IDS: bool = true;
    fn base_dir() -> PathBuf {
        paths::home_dir().join(".openclaw/sessions")
    }
}

pub type OpenClawProvider = GenericJsonlProvider<OpenClawConfig>;
