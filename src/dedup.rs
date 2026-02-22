use std::collections::HashSet;

use crate::types::UsageEntry;

#[must_use]
pub fn deduplicate(entries: Vec<UsageEntry>) -> Vec<UsageEntry> {
    let mut seen: HashSet<String> = HashSet::with_capacity(entries.len());
    let mut result = Vec::with_capacity(entries.len());

    for entry in entries {
        match entry.dedup_key() {
            Some(key) => {
                if seen.insert(key) {
                    result.push(entry);
                }
            }
            None => {
                result.push(entry);
            }
        }
    }
    result
}
