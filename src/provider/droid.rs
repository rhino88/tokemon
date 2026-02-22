use std::path::PathBuf;

use crate::paths;

use super::jsonl_provider::{GenericJsonlProvider, JsonlProviderConfig};

pub struct DroidConfig;

impl JsonlProviderConfig for DroidConfig {
    const NAME: &'static str = "droid";
    const DISPLAY_NAME: &'static str = "Droid";
    fn base_dir() -> PathBuf {
        paths::home_dir().join(".droid/sessions")
    }
}

pub type DroidProvider = GenericJsonlProvider<DroidConfig>;
