//! File watcher for live data updates.
//!
//! Uses the `notify` crate to watch source data directories for changes.
//! When a file changes, it re-parses that file and writes the updated
//! records to the `SQLite` cache. The TUI is then notified via the event
//! channel to re-poll the cache.

use std::path::PathBuf;
use std::time::Duration;

use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
use tokio::sync::mpsc;

use crate::cache::Cache;
use crate::source::SourceSet;
use crate::{cache, cost, dedup};

use super::event::Event;

/// Start the file watcher in the background.
///
/// This spawns a thread that watches all available source data directories
/// for file changes. When changes are detected (debounced to 500ms), it
/// re-parses the changed files and updates the `SQLite` cache, then sends
/// an [`Event::DataChanged`] through the event channel.
///
/// # Arguments
///
/// * `event_tx` — channel to notify the TUI of data changes
/// * `no_cost` — whether to skip pricing (from config)
pub fn start(event_tx: mpsc::UnboundedSender<Event>, no_cost: bool) {
    std::thread::spawn(move || {
        if let Err(e) = run_watcher(&event_tx, no_cost) {
            eprintln!("[tokemon] Warning: file watcher failed: {e}");
        }
    });
}

fn run_watcher(event_tx: &mpsc::UnboundedSender<Event>, no_cost: bool) -> anyhow::Result<()> {
    let registry = SourceSet::new();
    let available = registry.available();

    if available.is_empty() {
        return Ok(());
    }

    // Collect unique data directories to watch
    let mut watch_dirs: Vec<PathBuf> = available
        .iter()
        .map(|s| s.data_dir())
        .filter(|d| d.exists())
        .collect();
    watch_dirs.sort();
    watch_dirs.dedup();

    if watch_dirs.is_empty() {
        return Ok(());
    }

    // Channel for debounced file events
    let (tx, rx) = std::sync::mpsc::channel();

    // Create a debouncer with 500ms delay
    let mut debouncer = new_debouncer(Duration::from_millis(500), tx)?;

    // Watch each data directory recursively
    for dir in &watch_dirs {
        if let Err(e) = debouncer
            .watcher()
            .watch(dir, notify::RecursiveMode::Recursive)
        {
            eprintln!("[tokemon] Warning: could not watch {}: {e}", dir.display());
        }
    }

    // Process debounced events
    loop {
        match rx.recv() {
            Ok(Ok(events)) => {
                // Only process if there are actual file changes (not just metadata)
                let has_changes = events
                    .iter()
                    .any(|e| matches!(e.kind, DebouncedEventKind::Any));

                if has_changes {
                    // Re-parse changed files and update the cache
                    if let Err(e) = incremental_update(&registry, no_cost) {
                        eprintln!("[tokemon] Warning: incremental update failed: {e}");
                    }

                    // Notify the TUI
                    if event_tx.send(Event::DataChanged).is_err() {
                        // TUI has been dropped, stop watching
                        break;
                    }
                }
            }
            Ok(Err(errors)) => {
                eprintln!("[tokemon] Warning: watch error: {errors}");
            }
            Err(_) => {
                // Channel closed, debouncer dropped
                break;
            }
        }
    }

    Ok(())
}

/// Re-discover files, parse any that have changed, and update the cache.
///
/// This mirrors the logic in `main.rs::parse_with_cache` but is designed
/// to run from a background thread. It only re-parses files whose
/// modification time has changed since the last cache write.
fn incremental_update(registry: &SourceSet, no_cost: bool) -> anyhow::Result<()> {
    let cache = Cache::open()?;
    let cached_mtimes = cache.cached_file_mtimes();

    let providers = registry.available();

    // Discover all files and find changed ones
    let files_to_parse: Vec<(&dyn crate::source::Source, PathBuf, i64)> = providers
        .iter()
        .flat_map(|provider| {
            provider
                .discover_files()
                .into_iter()
                .filter_map(|file| {
                    let mtime = cache::file_mtime_secs(&file).unwrap_or(0);
                    let file_key = file.display().to_string();
                    if cached_mtimes.get(&file_key) == Some(&mtime) {
                        None
                    } else {
                        Some((*provider, file, mtime))
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect();

    if files_to_parse.is_empty() {
        return Ok(());
    }

    // Parse changed files (sequentially to avoid contention on the watcher thread)
    cache.begin()?;

    for (provider, file, mtime) in &files_to_parse {
        match provider.parse_file(file) {
            Ok(mut entries) => {
                // Apply pricing if enabled
                if !no_cost {
                    if let Ok(engine) = cost::PricingEngine::load(true) {
                        engine.apply_costs(&mut entries);
                    }
                }
                entries = dedup::deduplicate(entries);
                if let Err(e) = cache.store_file_entries(file, *mtime, &entries) {
                    eprintln!(
                        "[tokemon] Warning: cache write failed for {}: {e}",
                        file.display()
                    );
                }
            }
            Err(e) => {
                eprintln!("[tokemon] Warning: failed to parse {}: {e}", file.display());
            }
        }
    }

    cache.commit()?;
    cache.set_last_discovery();

    Ok(())
}
