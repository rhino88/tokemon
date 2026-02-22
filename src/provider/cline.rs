use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::types::UsageEntry;

use super::cline_format::ClineFormatParser;

pub struct ClineProvider {
    parser: ClineFormatParser,
}

impl ClineProvider {
    pub fn new() -> Self {
        Self {
            parser: ClineFormatParser {
                provider_name: "cline",
                extension_id: "saoudrizwan.claude-dev",
            },
        }
    }
}

impl super::Provider for ClineProvider {
    fn name(&self) -> &str {
        "cline"
    }

    fn display_name(&self) -> &str {
        "Cline"
    }

    fn data_dir(&self) -> PathBuf {
        self.parser.data_dir()
    }

    fn discover_files(&self) -> Vec<PathBuf> {
        self.parser.discover_files()
    }

    fn parse_file(&self, path: &Path) -> Result<Vec<UsageEntry>> {
        self.parser.parse_file(path)
    }
}
