use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::types::UsageEntry;

use super::cline_format::ClineFormatParser;

pub struct RooCodeProvider {
    parser: ClineFormatParser,
}

impl RooCodeProvider {
    pub fn new() -> Self {
        Self {
            parser: ClineFormatParser {
                provider_name: "roo-code",
                extension_id: "rooveterinaryinc.roo-cline",
            },
        }
    }
}

impl super::Provider for RooCodeProvider {
    fn name(&self) -> &str {
        "roo-code"
    }

    fn display_name(&self) -> &str {
        "Roo Code"
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
