use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::types::UsageEntry;

use super::cline_format::ClineFormatParser;

pub struct KiloCodeProvider {
    parser: ClineFormatParser,
}

impl KiloCodeProvider {
    pub fn new() -> Self {
        Self {
            parser: ClineFormatParser {
                provider_name: "kilo-code",
                extension_id: "kilocode.kilo-code",
            },
        }
    }
}

impl super::Provider for KiloCodeProvider {
    fn name(&self) -> &str {
        "kilo-code"
    }

    fn display_name(&self) -> &str {
        "Kilo Code"
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
