use std::path::PathBuf;

use crate::paths;

use super::jsonl_provider::{GenericJsonlProvider, JsonlProviderConfig};

pub struct PiAgentConfig;

impl JsonlProviderConfig for PiAgentConfig {
    const NAME: &'static str = "pi-agent";
    const DISPLAY_NAME: &'static str = "Pi Agent";
    fn base_dir() -> PathBuf {
        paths::home_dir().join(".pi-agent/sessions")
    }
}

pub type PiAgentProvider = GenericJsonlProvider<PiAgentConfig>;
