use std::path::PathBuf;

use crate::paths;

use super::jsonl_source::{JsonlSource, JsonlSourceConfig};

pub struct PiAgentConfig;

impl JsonlSourceConfig for PiAgentConfig {
    const NAME: &'static str = "pi-agent";
    const DISPLAY_NAME: &'static str = "Pi Agent";
    fn base_dir() -> PathBuf {
        paths::home_dir().join(".pi-agent/sessions")
    }
}

pub type PiAgentSource = JsonlSource<PiAgentConfig>;
