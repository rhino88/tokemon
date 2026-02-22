use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::paths;
use crate::types::UsageEntry;

pub struct PiebaldProvider {
    db_path: PathBuf,
}

impl PiebaldProvider {
    pub fn new() -> Self {
        Self {
            db_path: if cfg!(target_os = "macos") {
                paths::home_dir().join("Library/Application Support/piebald/app.db")
            } else {
                paths::home_dir().join(".local/share/piebald/app.db")
            },
        }
    }
}

impl super::Provider for PiebaldProvider {
    fn name(&self) -> &str {
        "piebald"
    }

    fn display_name(&self) -> &str {
        "Piebald"
    }

    fn data_dir(&self) -> PathBuf {
        self.db_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default()
    }

    fn discover_files(&self) -> Vec<PathBuf> {
        if self.db_path.exists() {
            vec![self.db_path.clone()]
        } else {
            Vec::new()
        }
    }

    fn parse_file(&self, _path: &Path) -> Result<Vec<UsageEntry>> {
        // SQLite parsing deferred to keep PoC dependencies minimal.
        // Would need rusqlite dependency.
        eprintln!("[tokemon] Note: Piebald SQLite parsing not yet implemented (needs rusqlite)");
        Ok(Vec::new())
    }
}
