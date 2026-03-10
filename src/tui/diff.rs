//! State diffing for detecting new and changed rows.
//!
//! Compares two snapshots of model usage data to identify:
//! - New rows (model+provider combo not in previous snapshot)
//! - Changed rows (same key, different token counts or cost)
//!
//! The diff results are used to trigger animations in Phase 3.

use std::collections::HashMap;

use crate::types::ModelUsage;

/// A unique key for a model usage row.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct RowKey {
    pub model: String,
    pub provider: String,
}

impl From<&ModelUsage> for RowKey {
    fn from(mu: &ModelUsage) -> Self {
        Self {
            model: mu.model.clone(),
            provider: mu.provider.clone(),
        }
    }
}

/// What kind of change happened to a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// A completely new row appeared.
    New,
    /// An existing row's values changed (tokens or cost updated).
    Updated,
}

/// A detected change in the usage data.
///
/// Used by the animation system (Phase 3) to determine which rows
/// need visual effects after a data refresh.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RowChange {
    /// Unique identifier for the changed row (model+provider).
    pub key: RowKey,
    /// What kind of change was detected.
    pub kind: ChangeKind,
}

/// Snapshot of model usage values for diffing.
#[derive(Debug, Clone)]
struct RowSnapshot {
    total_tokens: u64,
    cost_usd_cents: i64,
}

impl From<&ModelUsage> for RowSnapshot {
    fn from(mu: &ModelUsage) -> Self {
        let total = mu.input_tokens
            + mu.output_tokens
            + mu.cache_read_tokens
            + mu.cache_creation_tokens
            + mu.thinking_tokens;
        Self {
            total_tokens: total,
            // Compare costs at 4 decimal places to avoid float comparison issues
            #[allow(clippy::cast_possible_truncation)]
            cost_usd_cents: (mu.cost_usd * 10_000.0) as i64,
        }
    }
}

/// Compute the diff between two sets of model usage data.
///
/// Returns a list of changes: new rows and rows whose values changed.
#[must_use]
pub fn diff(previous: &[ModelUsage], current: &[ModelUsage]) -> Vec<RowChange> {
    let prev_map: HashMap<RowKey, RowSnapshot> = previous
        .iter()
        .map(|mu| (RowKey::from(mu), RowSnapshot::from(mu)))
        .collect();

    let mut changes = Vec::new();

    for mu in current {
        let key = RowKey::from(mu);
        let snap = RowSnapshot::from(mu);

        match prev_map.get(&key) {
            None => {
                // New row — not in previous snapshot
                changes.push(RowChange {
                    key,
                    kind: ChangeKind::New,
                });
            }
            Some(prev_snap) => {
                // Existing row — check if values changed
                if prev_snap.total_tokens != snap.total_tokens
                    || prev_snap.cost_usd_cents != snap.cost_usd_cents
                {
                    changes.push(RowChange {
                        key,
                        kind: ChangeKind::Updated,
                    });
                }
            }
        }
    }

    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_usage(model: &str, provider: &str, tokens: u64, cost: f64) -> ModelUsage {
        ModelUsage {
            model: model.to_string(),
            provider: provider.to_string(),
            input_tokens: tokens,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            thinking_tokens: 0,
            cost_usd: cost,
            request_count: 1,
        }
    }

    #[test]
    fn test_diff_empty_to_empty() {
        let changes = diff(&[], &[]);
        assert!(changes.is_empty());
    }

    #[test]
    fn test_diff_empty_to_some() {
        let current = vec![make_usage("opus-4", "claude-code", 1000, 0.50)];
        let changes = diff(&[], &current);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, ChangeKind::New);
        assert_eq!(changes[0].key.model, "opus-4");
    }

    #[test]
    fn test_diff_no_change() {
        let data = vec![make_usage("opus-4", "claude-code", 1000, 0.50)];
        let changes = diff(&data, &data);
        assert!(changes.is_empty());
    }

    #[test]
    fn test_diff_token_change() {
        let prev = vec![make_usage("opus-4", "claude-code", 1000, 0.50)];
        let curr = vec![make_usage("opus-4", "claude-code", 2000, 1.00)];
        let changes = diff(&prev, &curr);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, ChangeKind::Updated);
    }

    #[test]
    fn test_diff_new_and_updated() {
        let prev = vec![make_usage("opus-4", "claude-code", 1000, 0.50)];
        let curr = vec![
            make_usage("opus-4", "claude-code", 2000, 1.00),
            make_usage("sonnet-4", "claude-code", 500, 0.10),
        ];
        let changes = diff(&prev, &curr);
        assert_eq!(changes.len(), 2);

        let updated = changes.iter().find(|c| c.kind == ChangeKind::Updated);
        let new = changes.iter().find(|c| c.kind == ChangeKind::New);
        assert!(updated.is_some());
        assert!(new.is_some());
        assert_eq!(updated.unwrap().key.model, "opus-4");
        assert_eq!(new.unwrap().key.model, "sonnet-4");
    }

    #[test]
    fn test_diff_removed_row_not_reported() {
        // Rows that disappear are not reported as changes
        let prev = vec![
            make_usage("opus-4", "claude-code", 1000, 0.50),
            make_usage("sonnet-4", "claude-code", 500, 0.10),
        ];
        let curr = vec![make_usage("opus-4", "claude-code", 1000, 0.50)];
        let changes = diff(&prev, &curr);
        assert!(changes.is_empty());
    }
}
