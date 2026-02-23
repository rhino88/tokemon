use std::path::PathBuf;

use crate::paths;

use super::jsonl_source::{JsonlSource, JsonlSourceConfig};

pub struct KimiConfig;

impl JsonlSourceConfig for KimiConfig {
    const NAME: &'static str = "kimi";
    const DISPLAY_NAME: &'static str = "Kimi";
    fn base_dir() -> PathBuf {
        paths::home_dir().join(".kimi/sessions")
    }
}

pub type KimiSource = JsonlSource<KimiConfig>;
