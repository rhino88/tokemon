use std::path::PathBuf;

use crate::paths;

use super::jsonl_source::{JsonlSource, JsonlSourceConfig};

pub struct DroidConfig;

impl JsonlSourceConfig for DroidConfig {
    const NAME: &'static str = "droid";
    const DISPLAY_NAME: &'static str = "Droid";
    fn base_dir() -> PathBuf {
        paths::home_dir().join(".droid/sessions")
    }
}

pub type DroidSource = JsonlSource<DroidConfig>;
