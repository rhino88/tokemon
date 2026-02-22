use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{params, Connection};

use crate::paths;
use crate::types::UsageEntry;

const DB_FILENAME: &str = "usage.db";

pub struct Cache {
    conn: Connection,
}

impl Cache {
    pub fn open() -> anyhow::Result<Self> {
        let db_path = Self::db_path();
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)?;
        // WAL mode for concurrent reads and faster writes
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        let cache = Self { conn };
        cache.init_schema()?;
        Ok(cache)
    }

    fn db_path() -> PathBuf {
        paths::cache_dir().join(DB_FILENAME)
    }

    fn init_schema(&self) -> anyhow::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS usage_entries (
                id INTEGER PRIMARY KEY,
                provider TEXT NOT NULL,
                source_file TEXT NOT NULL,
                source_mtime INTEGER NOT NULL,
                timestamp TEXT NOT NULL,
                model TEXT,
                input_tokens INTEGER NOT NULL,
                output_tokens INTEGER NOT NULL,
                cache_read_tokens INTEGER NOT NULL,
                cache_creation_tokens INTEGER NOT NULL,
                thinking_tokens INTEGER NOT NULL,
                cost_usd REAL,
                message_id TEXT,
                request_id TEXT,
                session_id TEXT,
                dedup_key TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_timestamp ON usage_entries(timestamp);
            CREATE INDEX IF NOT EXISTS idx_provider ON usage_entries(provider);
            CREATE INDEX IF NOT EXISTS idx_source_file ON usage_entries(source_file, source_mtime);",
        )?;
        Ok(())
    }

    /// Begin a transaction for batch operations. Call commit() when done.
    pub fn begin(&self) -> anyhow::Result<()> {
        self.conn.execute_batch("BEGIN")?;
        Ok(())
    }

    /// Commit an active transaction.
    pub fn commit(&self) -> anyhow::Result<()> {
        self.conn.execute_batch("COMMIT")?;
        Ok(())
    }

    /// Get all cached (file, mtime) pairs in one query for bulk staleness checking.
    pub fn cached_file_mtimes(&self) -> std::collections::HashMap<String, i64> {
        let mut map = std::collections::HashMap::new();
        if let Ok(mut stmt) = self.conn.prepare(
            "SELECT DISTINCT source_file, source_mtime FROM usage_entries",
        ) {
            if let Ok(rows) = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            }) {
                for row in rows.flatten() {
                    map.insert(row.0, row.1);
                }
            }
        }
        map
    }

    /// Load ALL cached entries in one query (fast bulk read).
    pub fn load_all_entries(&self) -> anyhow::Result<Vec<UsageEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM usage_entries ORDER BY timestamp",
        )?;
        let entries = stmt
            .query_map([], |row| {
                let ts_str: String = row.get("timestamp")?;
                let timestamp = DateTime::parse_from_rfc3339(&ts_str)
                    .map(|dt| dt.to_utc())
                    .unwrap_or_else(|_| Utc::now());

                Ok(UsageEntry {
                    timestamp,
                    provider: row.get("provider")?,
                    model: row.get("model")?,
                    input_tokens: row.get::<_, i64>("input_tokens")? as u64,
                    output_tokens: row.get::<_, i64>("output_tokens")? as u64,
                    cache_read_tokens: row.get::<_, i64>("cache_read_tokens")? as u64,
                    cache_creation_tokens: row.get::<_, i64>("cache_creation_tokens")? as u64,
                    thinking_tokens: row.get::<_, i64>("thinking_tokens")? as u64,
                    cost_usd: row.get("cost_usd")?,
                    message_id: row.get("message_id")?,
                    request_id: row.get("request_id")?,
                    session_id: row.get("session_id")?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(entries)
    }

    /// Remove stale entries for a file (different mtime) and store new ones.
    pub fn store_file_entries(
        &self,
        path: &Path,
        mtime_secs: i64,
        entries: &[UsageEntry],
    ) -> anyhow::Result<()> {
        let path_str = path.display().to_string();

        // Remove old entries for this file
        self.conn.execute(
            "DELETE FROM usage_entries WHERE source_file = ?1",
            params![path_str],
        )?;

        // Insert new entries
        let mut stmt = self.conn.prepare(
            "INSERT INTO usage_entries (
                provider, source_file, source_mtime, timestamp, model,
                input_tokens, output_tokens, cache_read_tokens,
                cache_creation_tokens, thinking_tokens, cost_usd,
                message_id, request_id, session_id, dedup_key
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        )?;

        for entry in entries {
            stmt.execute(params![
                entry.provider,
                path_str,
                mtime_secs,
                entry.timestamp.to_rfc3339(),
                entry.model,
                entry.input_tokens,
                entry.output_tokens,
                entry.cache_read_tokens,
                entry.cache_creation_tokens,
                entry.thinking_tokens,
                entry.cost_usd,
                entry.message_id,
                entry.request_id,
                entry.session_id,
                entry.dedup_key(),
            ])?;
        }

        Ok(())
    }

    /// Query all cached entries, optionally filtered by providers and date range.
    pub fn query_entries(
        &self,
        providers: &[String],
        since: Option<NaiveDate>,
        until: Option<NaiveDate>,
    ) -> anyhow::Result<Vec<UsageEntry>> {
        let mut sql = String::from("SELECT * FROM usage_entries WHERE 1=1");
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if !providers.is_empty() {
            let placeholders: Vec<String> = providers
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", i + 1))
                .collect();
            sql.push_str(&format!(" AND provider IN ({})", placeholders.join(",")));
            for p in providers {
                param_values.push(Box::new(p.clone()));
            }
        }

        if let Some(s) = since {
            let idx = param_values.len() + 1;
            sql.push_str(&format!(" AND timestamp >= ?{}", idx));
            let dt: DateTime<Utc> = s.and_hms_opt(0, 0, 0).unwrap().and_utc();
            param_values.push(Box::new(dt.to_rfc3339()));
        }

        if let Some(u) = until {
            let idx = param_values.len() + 1;
            sql.push_str(&format!(" AND timestamp <= ?{}", idx));
            let dt: DateTime<Utc> = u.and_hms_opt(23, 59, 59).unwrap().and_utc();
            param_values.push(Box::new(dt.to_rfc3339()));
        }

        sql.push_str(" ORDER BY timestamp");

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = self.conn.prepare(&sql)?;
        let entries = stmt
            .query_map(params_refs.as_slice(), |row| {
                let ts_str: String = row.get("timestamp")?;
                let timestamp = DateTime::parse_from_rfc3339(&ts_str)
                    .map(|dt| dt.to_utc())
                    .unwrap_or_else(|_| Utc::now());

                Ok(UsageEntry {
                    timestamp,
                    provider: row.get("provider")?,
                    model: row.get("model")?,
                    input_tokens: row.get::<_, i64>("input_tokens")? as u64,
                    output_tokens: row.get::<_, i64>("output_tokens")? as u64,
                    cache_read_tokens: row.get::<_, i64>("cache_read_tokens")? as u64,
                    cache_creation_tokens: row.get::<_, i64>("cache_creation_tokens")? as u64,
                    thinking_tokens: row.get::<_, i64>("thinking_tokens")? as u64,
                    cost_usd: row.get("cost_usd")?,
                    message_id: row.get("message_id")?,
                    request_id: row.get("request_id")?,
                    session_id: row.get("session_id")?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(entries)
    }

    /// Query entries for a specific file.
    pub fn query_file_entries(&self, path: &Path) -> anyhow::Result<Vec<UsageEntry>> {
        let path_str = path.display().to_string();
        let mut stmt = self.conn.prepare(
            "SELECT * FROM usage_entries WHERE source_file = ?1 ORDER BY timestamp",
        )?;
        let entries = stmt
            .query_map(params![path_str], |row| {
                let ts_str: String = row.get("timestamp")?;
                let timestamp = DateTime::parse_from_rfc3339(&ts_str)
                    .map(|dt| dt.to_utc())
                    .unwrap_or_else(|_| Utc::now());

                Ok(UsageEntry {
                    timestamp,
                    provider: row.get("provider")?,
                    model: row.get("model")?,
                    input_tokens: row.get::<_, i64>("input_tokens")? as u64,
                    output_tokens: row.get::<_, i64>("output_tokens")? as u64,
                    cache_read_tokens: row.get::<_, i64>("cache_read_tokens")? as u64,
                    cache_creation_tokens: row.get::<_, i64>("cache_creation_tokens")? as u64,
                    thinking_tokens: row.get::<_, i64>("thinking_tokens")? as u64,
                    cost_usd: row.get("cost_usd")?,
                    message_id: row.get("message_id")?,
                    request_id: row.get("request_id")?,
                    session_id: row.get("session_id")?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(entries)
    }

    /// Get the total count of cached entries.
    pub fn entry_count(&self) -> i64 {
        self.conn
            .query_row("SELECT COUNT(*) FROM usage_entries", [], |row| row.get(0))
            .unwrap_or(0)
    }
}

/// Get file modification time as seconds since epoch.
pub fn file_mtime_secs(path: &Path) -> Option<i64> {
    fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs() as i64)
}
