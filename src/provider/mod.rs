pub mod amp;
pub mod claude_code;
pub mod cline;
pub mod cline_format;
pub mod codex;
pub mod jsonl_provider;
pub mod copilot;
pub mod cursor;
pub mod droid;
pub mod gemini;
pub mod kilo_code;
pub mod kimi;
pub mod openclaw;
pub mod opencode;
pub mod pi_agent;
pub mod piebald;
pub mod qwen;
pub mod roo_code;

use std::path::{Path, PathBuf};

use rayon::prelude::*;

use crate::dedup;
use crate::error::Result;
use crate::types::UsageEntry;

pub trait Provider: Send + Sync {
    /// Short identifier: "claude-code", "codex", "gemini"
    fn name(&self) -> &str;

    /// Human-readable: "Claude Code", "Codex CLI", "Gemini CLI"
    fn display_name(&self) -> &str;

    /// Return the base data directory for display purposes
    fn data_dir(&self) -> PathBuf;

    /// Return all data files for this provider on this machine
    fn discover_files(&self) -> Vec<PathBuf>;

    /// Parse one file into usage entries
    fn parse_file(&self, path: &Path) -> Result<Vec<UsageEntry>>;

    /// Whether this provider has any data
    fn is_available(&self) -> bool {
        !self.discover_files().is_empty()
    }

    /// Parse all files in parallel with dedup
    fn parse_all(&self) -> Result<Vec<UsageEntry>> {
        let files = self.discover_files();
        let all: Vec<UsageEntry> = files
            .par_iter()
            .flat_map(|f| {
                self.parse_file(f).unwrap_or_else(|e| {
                    eprintln!("[tokemon] Warning: failed to parse {}: {}", f.display(), e);
                    Vec::new()
                })
            })
            .collect();
        Ok(dedup::deduplicate(all))
    }
}

pub struct ProviderRegistry {
    providers: Vec<Box<dyn Provider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: vec![
                Box::new(claude_code::ClaudeCodeProvider::new()),
                Box::new(codex::CodexProvider::new()),
                Box::new(gemini::GeminiProvider::new()),
                Box::new(opencode::OpenCodeProvider::new()),
                Box::new(amp::AmpProvider::new()),
                Box::new(cline::ClineProvider::new()),
                Box::new(roo_code::RooCodeProvider::new()),
                Box::new(kilo_code::KiloCodeProvider::new()),
                Box::new(copilot::CopilotProvider::new()),
                Box::new(pi_agent::PiAgentProvider::new()),
                Box::new(kimi::KimiProvider::new()),
                Box::new(droid::DroidProvider::new()),
                Box::new(openclaw::OpenClawProvider::new()),
                Box::new(qwen::QwenProvider::new()),
                Box::new(piebald::PiebaldProvider::new()),
                Box::new(cursor::CursorProvider::new()),
            ],
        }
    }

    pub fn available(&self) -> Vec<&dyn Provider> {
        self.providers
            .iter()
            .filter(|p| p.is_available())
            .map(|p| p.as_ref())
            .collect()
    }

    pub fn all_providers(&self) -> Vec<&dyn Provider> {
        self.providers.iter().map(|p| p.as_ref()).collect()
    }

    pub fn get(&self, name: &str) -> Option<&dyn Provider> {
        self.providers
            .iter()
            .find(|p| p.name() == name)
            .map(|p| p.as_ref())
    }

    /// Parse entries from all (or filtered) providers, merge, sort by timestamp
    pub fn all_entries(&self, filter: &[String]) -> Result<Vec<UsageEntry>> {
        let providers: Vec<&dyn Provider> = if filter.is_empty() {
            self.available()
        } else {
            let mut selected = Vec::new();
            for name in filter {
                match self.get(name) {
                    Some(p) => selected.push(p),
                    None => {
                        return Err(crate::error::TokemonError::ProviderNotFound(
                            name.clone(),
                        ))
                    }
                }
            }
            selected
        };

        let mut all_entries: Vec<UsageEntry> = Vec::new();
        for provider in providers {
            match provider.parse_all() {
                Ok(entries) => all_entries.extend(entries),
                Err(e) => {
                    eprintln!(
                        "[tokemon] Warning: failed to parse {}: {}",
                        provider.name(),
                        e
                    );
                }
            }
        }

        all_entries.sort_by_key(|e| e.timestamp);
        Ok(all_entries)
    }
}
