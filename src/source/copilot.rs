use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::paths;
use crate::types::Record;

pub struct CopilotSource;

impl CopilotSource {
    pub fn new() -> Self {
        Self
    }
}

impl super::Source for CopilotSource {
    fn name(&self) -> &str {
        "copilot"
    }

    fn display_name(&self) -> &str {
        "GitHub Copilot"
    }

    fn data_dir(&self) -> PathBuf {
        let storage_dirs = paths::vscode_global_storage_dirs();
        if let Some(first) = storage_dirs.first() {
            // Copilot stores in workspaceStorage, not globalStorage
            first
                .parent()
                .map(|p| p.join("workspaceStorage"))
                .unwrap_or_default()
        } else {
            PathBuf::from("(VSCode workspaceStorage)")
        }
    }

    fn discover_files(&self) -> Vec<PathBuf> {
        // Copilot chat sessions are in workspaceStorage
        let storage_dirs = paths::vscode_global_storage_dirs();
        let mut files = Vec::new();

        for storage_dir in storage_dirs {
            if let Some(parent) = storage_dir.parent() {
                let ws_storage = parent.join("workspaceStorage");
                let pattern = ws_storage
                    .join("*/chatSessions/*.json")
                    .display()
                    .to_string();
                if let Ok(paths) = glob::glob(&pattern) {
                    files.extend(paths.filter_map(|p| p.ok()));
                }
            }
        }
        files
    }

    fn parse_file(&self, _path: &Path) -> Result<Vec<Record>> {
        // Copilot doesn't store token counts in its session files.
        // Would need tiktoken for estimation. For PoC, return empty.
        Ok(Vec::new())
    }
}
