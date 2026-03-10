use std::collections::HashMap;
use std::time::Instant;

use chrono::{Datelike, NaiveDate, Timelike, Utc};
use crossterm::event::{KeyCode, KeyEvent};

use crate::config::Config;
use crate::render::{self, format_tokens_short};
use crate::source::SourceSet;
use crate::types::{DailySummary, ModelUsage, Record};
use crate::{cache, cost, dedup, rollup};

use super::diff::{self, RowKey};
use super::event::Event;

/// Duration (in seconds) for the per-cell highlight fade animation.
const HIGHLIGHT_DURATION_SECS: f64 = 1.5;

/// Duration (in seconds) for warnings to remain visible in the status bar.
const WARNING_DISPLAY_SECS: f64 = 5.0;

// ── View scope ────────────────────────────────────────────────────────────

/// Which time window the detail table shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Today,
    Week,
    Month,
}

impl Scope {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::Week => "This Week",
            Self::Month => "This Month",
        }
    }

    /// Return the start date for this scope.
    #[must_use]
    pub fn since(self) -> NaiveDate {
        let today = Utc::now().date_naive();
        match self {
            Self::Today => today,
            Self::Week => {
                today - chrono::Duration::days(i64::from(today.weekday().num_days_from_monday()))
            }
            Self::Month => NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today),
        }
    }
}

// ── Summary card data ─────────────────────────────────────────────────────

/// Data for one summary card (Today / This Week / This Month).
#[derive(Debug, Clone)]
pub struct CardData {
    pub label: &'static str,
    pub cost: f64,
    pub tokens: u64,
    pub sparkline: Vec<u64>,
    /// Trend indicator: positive = increasing, negative = decreasing, zero = flat.
    pub trend: i8,
}

impl CardData {
    #[must_use]
    pub fn cost_str(&self) -> String {
        render::format_cost(self.cost)
    }

    #[must_use]
    pub fn tokens_str(&self) -> String {
        format!("{} tokens", format_tokens_short(self.tokens))
    }

    /// Trend arrow for display.
    #[must_use]
    pub fn trend_symbol(&self) -> &'static str {
        match self.trend.cmp(&0) {
            std::cmp::Ordering::Greater => "↑",
            std::cmp::Ordering::Less => "↓",
            std::cmp::Ordering::Equal => "−",
        }
    }
}

// ── Group-by mode ─────────────────────────────────────────────────────────

/// How to group rows in the detail table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupBy {
    /// One row per model (aggregated across all clients).
    Model,
    /// One row per model+client combination.
    ModelClient,
    /// One row per client (aggregated across all models).
    Client,
}

impl GroupBy {
    /// Cycle to the next group-by mode.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Model => Self::ModelClient,
            Self::ModelClient => Self::Client,
            Self::Client => Self::Model,
        }
    }

    /// Short label for display.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Model => "model",
            Self::ModelClient => "model+client",
            Self::Client => "client",
        }
    }
}

// ── Sort order ────────────────────────────────────────────────────────────

/// Sort order for the detail table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    /// Sort by cost descending (default).
    CostDesc,
    /// Sort by total tokens descending.
    TokensDesc,
    /// Sort by model name ascending.
    NameAsc,
    /// Sort by request count descending.
    RequestsDesc,
}

impl SortOrder {
    /// Cycle to the next sort order.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::CostDesc => Self::TokensDesc,
            Self::TokensDesc => Self::NameAsc,
            Self::NameAsc => Self::RequestsDesc,
            Self::RequestsDesc => Self::CostDesc,
        }
    }

    /// Short label for display.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::CostDesc => "cost",
            Self::TokensDesc => "tokens",
            Self::NameAsc => "name",
            Self::RequestsDesc => "requests",
        }
    }
}

// ── App state ─────────────────────────────────────────────────────────────

#[allow(clippy::struct_excessive_bools)]
pub struct App {
    /// Currently selected detail scope.
    pub scope: Scope,
    /// How to group rows in the detail table.
    pub group_by: GroupBy,
    /// Whether history mode is toggled on.
    pub show_history: bool,
    /// Summary cards (always three: today, week, month).
    pub cards: [CardData; 3],
    /// Detail table rows for the selected scope.
    pub detail_models: Vec<ModelUsage>,
    /// Detail totals.
    pub detail_total_cost: f64,
    pub detail_total_tokens: u64,
    /// Historical summaries (populated when `show_history` is true).
    pub history_summaries: Vec<DailySummary>,
    /// Scroll offset for the detail table.
    pub scroll_offset: u16,
    /// Whether the app should quit.
    pub should_quit: bool,
    /// Whether the help overlay is shown.
    pub show_help: bool,
    /// Whether the filter input is active.
    pub filter_active: bool,
    /// Current filter input text.
    pub filter_text: String,
    /// Applied filter (empty = no filter).
    pub applied_filter: String,
    /// Current sort order.
    pub sort_order: SortOrder,
    /// Per-row highlight timestamps: maps a `RowKey` to the instant
    /// it was last updated. Used for the green fade animation on
    /// individual table cells.
    pub highlight_map: HashMap<RowKey, Instant>,
    /// Last warning message from the background watcher or data loading,
    /// with the instant it was received. Displayed in the status bar
    /// for a few seconds then cleared.
    pub last_warning: Option<(String, Instant)>,
    /// Whether the UI state has changed and needs a redraw.
    /// Set by event handlers, cleared after each frame is drawn.
    pub dirty: bool,
    /// Cached pricing engine (loaded once at startup, reused for all reads).
    pricing: Option<cost::PricingEngine>,
    /// Source registry (created once, reused for tick-based polling).
    registry: SourceSet,
    /// Whether to skip cost calculation.
    no_cost: bool,
    /// Cached raw records for the current data load.
    cached_records: Vec<Record>,
    /// Previous model snapshot for diffing.
    prev_models: Vec<ModelUsage>,
    /// Whether the initial data load is complete. The first load
    /// populates `prev_models` but does NOT trigger highlights,
    /// preventing the "everything flashes" effect on startup.
    initial_load_done: bool,
}

impl App {
    /// Create a new app and perform the initial data load.
    pub fn new(config: &Config, initial_scope: Scope) -> Self {
        let mut app = Self {
            scope: initial_scope,
            group_by: GroupBy::ModelClient,
            show_history: false,
            cards: [
                CardData {
                    label: "Today",
                    cost: 0.0,
                    tokens: 0,
                    sparkline: Vec::new(),
                    trend: 0,
                },
                CardData {
                    label: "This Week",
                    cost: 0.0,
                    tokens: 0,
                    sparkline: Vec::new(),
                    trend: 0,
                },
                CardData {
                    label: "This Month",
                    cost: 0.0,
                    tokens: 0,
                    sparkline: Vec::new(),
                    trend: 0,
                },
            ],
            detail_models: Vec::new(),
            detail_total_cost: 0.0,
            detail_total_tokens: 0,
            history_summaries: Vec::new(),
            scroll_offset: 0,
            should_quit: false,
            show_help: false,
            filter_active: false,
            filter_text: String::new(),
            applied_filter: String::new(),
            sort_order: SortOrder::CostDesc,
            highlight_map: HashMap::new(),
            last_warning: None,
            dirty: true,
            pricing: None,
            registry: SourceSet::new(),
            no_cost: config.no_cost,
            cached_records: Vec::new(),
            prev_models: Vec::new(),
            initial_load_done: false,
        };
        // Load pricing engine once (offline to avoid blocking).
        if !config.no_cost {
            app.pricing = cost::PricingEngine::load(true).ok();
        }
        // Initial data load: sync sources then read cache.
        app.poll_sources();
        app.reload_from_cache();
        app
    }

    /// Handle an incoming event. Returns `true` if the UI needs a redraw.
    pub fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::Key(key) => {
                let changed = self.handle_key(*key);
                self.dirty |= changed;
                changed
            }
            Event::Tick => {
                // Poll source files for changes (lightweight mtime checks),
                // re-parse any that changed, and update the cache.
                // The watcher thread also does this on file events, but
                // tick-based polling catches changes that `notify` may miss
                // (e.g. SQLite WAL writes on some platforms).
                self.poll_sources();
                self.dirty |= self.reload_from_cache();
                // Expire old warnings
                if let Some((_, t)) = &self.last_warning {
                    if t.elapsed().as_secs_f64() >= WARNING_DISPLAY_SECS {
                        self.last_warning = None;
                        self.dirty = true;
                    }
                }
                self.dirty
            }
            Event::DataChanged => {
                // The watcher already wrote to the cache — just re-read it.
                self.dirty |= self.reload_from_cache();
                self.dirty
            }
            Event::Warning(msg) => {
                self.last_warning = Some((msg.clone(), Instant::now()));
                self.dirty = true;
                true
            }
            Event::Resize(_, _) => {
                self.dirty = true;
                true
            }
            Event::Render => false,
        }
    }

    /// Returns the current warning message if it's still fresh (< 5 seconds old).
    #[must_use]
    pub fn active_warning(&self) -> Option<&str> {
        self.last_warning.as_ref().and_then(|(msg, t)| {
            if t.elapsed().as_secs_f64() < WARNING_DISPLAY_SECS {
                Some(msg.as_str())
            } else {
                None
            }
        })
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        // If help is shown, any key dismisses it
        if self.show_help {
            self.show_help = false;
            return true;
        }

        // Filter input mode
        if self.filter_active {
            return self.handle_filter_key(key);
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                if self.applied_filter.is_empty() {
                    self.should_quit = true;
                    false
                } else {
                    // First Esc/q clears the filter
                    self.applied_filter.clear();
                    self.recompute_detail();
                    true
                }
            }
            KeyCode::Char('?') => {
                self.show_help = true;
                true
            }
            KeyCode::Char('/') => {
                self.filter_active = true;
                self.filter_text = self.applied_filter.clone();
                true
            }
            KeyCode::Char('t') => {
                self.scope = Scope::Today;
                self.scroll_offset = 0;
                self.recompute_detail();
                true
            }
            KeyCode::Char('w') => {
                self.scope = Scope::Week;
                self.scroll_offset = 0;
                self.recompute_detail();
                true
            }
            KeyCode::Char('m') => {
                self.scope = Scope::Month;
                self.scroll_offset = 0;
                self.recompute_detail();
                true
            }
            KeyCode::Char('s') => {
                self.sort_order = self.sort_order.next();
                self.recompute_detail();
                true
            }
            KeyCode::Char('g') => {
                self.group_by = self.group_by.next();
                self.recompute_detail();
                true
            }
            KeyCode::Char('h') => {
                self.show_history = !self.show_history;
                self.recompute_detail();
                true
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
                true
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                true
            }
            KeyCode::Left => {
                let new_scope = match self.scope {
                    Scope::Today | Scope::Week => Scope::Today,
                    Scope::Month => Scope::Week,
                };
                if new_scope != self.scope {
                    self.scope = new_scope;
                    self.scroll_offset = 0;
                    self.recompute_detail();
                }
                true
            }
            KeyCode::Right => {
                let new_scope = match self.scope {
                    Scope::Today => Scope::Week,
                    Scope::Week | Scope::Month => Scope::Month,
                };
                if new_scope != self.scope {
                    self.scope = new_scope;
                    self.scroll_offset = 0;
                    self.recompute_detail();
                }
                true
            }
            _ => false,
        }
    }

    fn handle_filter_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Enter => {
                self.filter_active = false;
                self.applied_filter = self.filter_text.clone();
                self.scroll_offset = 0;
                self.recompute_detail();
                true
            }
            KeyCode::Esc => {
                self.filter_active = false;
                self.filter_text.clear();
                true
            }
            KeyCode::Char(c) => {
                self.filter_text.push(c);
                true
            }
            KeyCode::Backspace => {
                self.filter_text.pop();
                true
            }
            _ => false,
        }
    }

    /// Poll source files for mtime changes, re-parse any that changed,
    /// and write the results to the cache in a single transaction.
    ///
    /// This is the primary mechanism for detecting live data updates.
    /// It checks file modification times (including SQLite WAL siblings)
    /// against the mtimes stored in the cache. Only files with newer
    /// mtimes are re-parsed, so the cost is negligible when nothing changed.
    fn poll_sources(&self) {
        let Ok(mut cache) = cache::Cache::open() else {
            return;
        };
        let cached_mtimes = cache.cached_file_mtimes().unwrap_or_default();
        let providers = self.registry.available();

        // Collect parsed results with owned PathBufs.
        let mut parsed: Vec<(std::path::PathBuf, i64, Vec<crate::types::Record>)> = Vec::new();

        for provider in &providers {
            for file in provider.discover_files() {
                let mtime = cache::file_mtime_secs_for_db(&file).unwrap_or(0);
                let file_key = file.display().to_string();
                if cached_mtimes.get(&file_key) == Some(&mtime) {
                    continue;
                }
                if let Ok(mut entries) = provider.parse_file(&file) {
                    if !self.no_cost {
                        if let Some(engine) = self.pricing.as_ref() {
                            engine.apply_costs(&mut entries);
                        }
                    }
                    let entries = dedup::deduplicate(entries);
                    parsed.push((file, mtime, entries));
                }
            }
        }

        if parsed.is_empty() {
            return;
        }

        // write_entries takes &[(&Path, i64, Vec<Record>)].
        // We own PathBufs in `parsed` — build refs that borrow from them.
        let refs: Vec<(&std::path::Path, i64, Vec<crate::types::Record>)> = parsed
            .iter_mut()
            .map(|(p, m, e)| (p.as_path(), *m, std::mem::take(e)))
            .collect();

        let _ = cache.write_entries(&refs);
    }

    /// Re-read the cache and recompute all derived state.
    /// Returns `true` if data actually changed (new highlights or initial load).
    ///
    /// This is lightweight — it only queries SQLite and recomputes
    /// in-memory aggregations. File discovery and parsing are handled
    /// by the background watcher thread.
    fn reload_from_cache(&mut self) -> bool {
        let records = load_records_from_cache(self.pricing.as_ref());
        self.cached_records = records;
        self.recompute_cards();

        // Snapshot current models before recomputing for diff
        let prev = std::mem::take(&mut self.prev_models);
        self.recompute_detail();

        // On first load, just seed prev_models — no highlights.
        if !self.initial_load_done {
            self.initial_load_done = true;
            self.prev_models = self.detail_models.clone();
            return true;
        }

        // Compute diff against previous state
        let changes = diff::diff(&prev, &self.detail_models);
        let has_changes = !changes.is_empty();
        let now = Instant::now();
        for change in &changes {
            self.highlight_map.insert(change.key.clone(), now);
        }

        // Expire old highlights
        self.highlight_map
            .retain(|_, t| t.elapsed().as_secs_f64() < HIGHLIGHT_DURATION_SECS);

        // Save current models for next diff
        self.prev_models = self.detail_models.clone();

        has_changes
    }

    /// Returns `true` if any per-row highlights are still actively fading.
    /// Used by the main loop to keep redrawing during animations.
    #[must_use]
    pub fn has_active_highlights(&self) -> bool {
        self.highlight_map
            .values()
            .any(|t| t.elapsed().as_secs_f64() < HIGHLIGHT_DURATION_SECS)
    }

    /// Return the highlight intensity (0.0–1.0) for a given row key.
    /// Returns 0.0 if the row has no active highlight.
    #[must_use]
    pub fn highlight_intensity(&self, key: &RowKey) -> f64 {
        self.highlight_map.get(key).map_or(0.0, |t| {
            let elapsed = t.elapsed().as_secs_f64();
            if elapsed >= HIGHLIGHT_DURATION_SECS {
                0.0
            } else {
                1.0 - (elapsed / HIGHLIGHT_DURATION_SECS)
            }
        })
    }

    fn recompute_cards(&mut self) {
        let today = Utc::now().date_naive();

        // Today card
        let today_records: Vec<&Record> = self
            .cached_records
            .iter()
            .filter(|r| r.timestamp.date_naive() == today)
            .collect();
        self.cards[0].cost = sum_cost(&today_records);
        self.cards[0].tokens = sum_tokens(&today_records);

        // This week card
        let week_start = Scope::Week.since();
        let week_records: Vec<&Record> = self
            .cached_records
            .iter()
            .filter(|r| r.timestamp.date_naive() >= week_start)
            .collect();
        self.cards[1].cost = sum_cost(&week_records);
        self.cards[1].tokens = sum_tokens(&week_records);

        // Build sparkline: daily totals for the last 7 days
        self.cards[1].sparkline = build_daily_sparkline(&self.cached_records, 7);

        // This month card
        let month_start = Scope::Month.since();
        let month_records: Vec<&Record> = self
            .cached_records
            .iter()
            .filter(|r| r.timestamp.date_naive() >= month_start)
            .collect();
        self.cards[2].cost = sum_cost(&month_records);
        self.cards[2].tokens = sum_tokens(&month_records);

        // Build sparkline: daily totals for the last 30 days
        self.cards[2].sparkline = build_daily_sparkline(&self.cached_records, 30);

        // Today sparkline: hourly totals for today
        self.cards[0].sparkline = build_hourly_sparkline(&self.cached_records);

        // Compute trends from sparkline data
        for card in &mut self.cards {
            card.trend = compute_trend(&card.sparkline);
        }
    }

    #[allow(clippy::too_many_lines)]
    fn recompute_detail(&mut self) {
        let since = self.scope.since();
        let filtered: Vec<Record> = self
            .cached_records
            .iter()
            .filter(|r| r.timestamp.date_naive() >= since)
            .cloned()
            .collect();

        // Aggregate into model-level breakdown for the selected scope
        let summaries = rollup::aggregate_daily(&filtered);

        // Flatten all model usages across all days in the scope,
        // grouping by the selected group-by mode.
        let mut model_map: std::collections::HashMap<(String, String), ModelUsage> =
            std::collections::HashMap::new();

        for summary in &summaries {
            for mu in &summary.models {
                let key = match self.group_by {
                    GroupBy::Model => (mu.model.clone(), String::new()),
                    GroupBy::ModelClient => (mu.model.clone(), mu.provider.clone()),
                    GroupBy::Client => (String::new(), mu.provider.clone()),
                };
                let entry = model_map.entry(key).or_insert_with(|| match self.group_by {
                    GroupBy::Model => ModelUsage {
                        model: mu.model.clone(),
                        // Use the normalized name (not the first-seen raw
                        // name) so that `infer_api_provider` returns the
                        // model's native vendor (e.g. "Anthropic") rather
                        // than a random routing layer from whichever client
                        // happened to be inserted first.
                        raw_model: mu.model.clone(),
                        provider: String::new(),
                        ..Default::default()
                    },
                    GroupBy::ModelClient => ModelUsage {
                        model: mu.model.clone(),
                        raw_model: mu.raw_model.clone(),
                        provider: mu.provider.clone(),
                        ..Default::default()
                    },
                    GroupBy::Client => ModelUsage {
                        model: String::new(),
                        raw_model: String::new(),
                        provider: mu.provider.clone(),
                        ..Default::default()
                    },
                });
                entry.input_tokens += mu.input_tokens;
                entry.output_tokens += mu.output_tokens;
                entry.cache_read_tokens += mu.cache_read_tokens;
                entry.cache_creation_tokens += mu.cache_creation_tokens;
                entry.thinking_tokens += mu.thinking_tokens;
                entry.cost_usd += mu.cost_usd;
                entry.request_count += mu.request_count;
            }
        }

        let mut models: Vec<ModelUsage> = model_map.into_values().collect();

        // Apply provider/model filter if set
        if !self.applied_filter.is_empty() {
            let filter_lower = self.applied_filter.to_lowercase();
            models.retain(|m| {
                m.model.to_lowercase().contains(&filter_lower)
                    || m.provider.to_lowercase().contains(&filter_lower)
                    || crate::display::infer_api_provider(&m.model)
                        .to_lowercase()
                        .contains(&filter_lower)
            });
        }

        // Apply current sort order (stable sort to prevent shuffling of equal rows)
        // Always use model name as tiebreaker for deterministic ordering.
        match self.sort_order {
            SortOrder::CostDesc => {
                models.sort_by(|a, b| {
                    b.cost_usd
                        .total_cmp(&a.cost_usd)
                        .then_with(|| a.model.cmp(&b.model))
                });
            }
            SortOrder::TokensDesc => {
                models.sort_by(|a, b| {
                    let ta = a.total_tokens();
                    let tb = b.total_tokens();
                    tb.cmp(&ta).then_with(|| a.model.cmp(&b.model))
                });
            }
            SortOrder::NameAsc => {
                models.sort_by(|a, b| a.model.cmp(&b.model));
            }
            SortOrder::RequestsDesc => {
                models.sort_by(|a, b| {
                    b.request_count
                        .cmp(&a.request_count)
                        .then_with(|| a.model.cmp(&b.model))
                });
            }
        }

        self.detail_total_cost = models.iter().map(|m| m.cost_usd).sum();
        self.detail_total_tokens = models.iter().map(|m| m.total_tokens()).sum();
        self.detail_models = models;

        // Historical summaries for the history view
        if self.show_history {
            self.history_summaries = match self.scope {
                Scope::Today | Scope::Week => rollup::aggregate_daily(&filtered),
                Scope::Month => rollup::aggregate_weekly(&filtered),
            };
        } else {
            self.history_summaries.clear();
        }
    }
}

// ── Data loading ──────────────────────────────────────────────────────────

fn load_records_from_cache(pricing: Option<&cost::PricingEngine>) -> Vec<Record> {
    let Ok(c) = cache::Cache::open() else {
        return Vec::new();
    };

    // Load everything — the TUI filters in memory for card summaries.
    // We load the last ~60 days to keep things bounded.
    let since = Scope::Month.since() - chrono::Duration::days(30);
    let mut entries = c
        .load_entries_filtered(Some(since), None, &[])
        .unwrap_or_default();

    // Apply pricing from pre-loaded engine (no disk I/O here).
    if let Some(engine) = pricing {
        engine.apply_costs(&mut entries);
    }

    entries.sort_by_key(|e| e.timestamp);
    entries
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn sum_cost(records: &[&Record]) -> f64 {
    records.iter().map(|r| r.cost_usd.unwrap_or(0.0)).sum()
}

fn sum_tokens(records: &[&Record]) -> u64 {
    records.iter().map(|r| r.total_tokens()).sum()
}

/// Build a sparkline of daily token totals for the last `days` days.
fn build_daily_sparkline(records: &[Record], days: usize) -> Vec<u64> {
    let today = Utc::now().date_naive();
    let mut data = vec![0u64; days];

    for record in records {
        let record_date = record.timestamp.date_naive();
        let day_offset = (today - record_date).num_days();
        if let Ok(idx) = usize::try_from(day_offset) {
            if idx < days {
                data[days - 1 - idx] += record.total_tokens();
            }
        }
    }

    data
}

/// Build a sparkline of hourly token totals for today (24 buckets).
fn build_hourly_sparkline(records: &[Record]) -> Vec<u64> {
    let today = Utc::now().date_naive();
    let mut data = vec![0u64; 24];

    for record in records {
        if record.timestamp.date_naive() == today {
            let hour = record.timestamp.hour() as usize;
            if hour < 24 {
                data[hour] += record.total_tokens();
            }
        }
    }

    data
}

/// Compute a simple trend from sparkline data.
/// Compares the last value to the average of previous values.
fn compute_trend(data: &[u64]) -> i8 {
    if data.len() < 2 {
        return 0;
    }
    let last = data[data.len() - 1];
    let prev_avg = data[..data.len() - 1].iter().sum::<u64>() / (data.len() as u64 - 1).max(1);
    if last > prev_avg.saturating_add(prev_avg / 10) {
        1 // increasing
    } else if last < prev_avg.saturating_sub(prev_avg / 10) {
        -1 // decreasing
    } else {
        0 // flat
    }
}
