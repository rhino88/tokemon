use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{params, Connection, Row, types::Value};

use crate::paths;
use crate::types::Record;

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
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA cache_size=-10000;
             PRAGMA temp_store=MEMORY;
             PRAGMA mmap_size=268435456;",
        )?;
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
            CREATE INDEX IF NOT EXISTS idx_source_file ON usage_entries(source_file, source_mtime);
            CREATE TABLE IF NOT EXISTS cache_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )?;
        Ok(())
    }

    pub fn begin(&self) -> anyhow::Result<()> {
        self.conn.execute_batch("BEGIN")?;
        Ok(())
    }

    pub fn commit(&self) -> anyhow::Result<()> {
        self.conn.execute_batch("COMMIT")?;
        Ok(())
    }

    /// Get all cached (file, mtime) pairs in one query for bulk staleness checking.
    pub fn cached_file_mtimes(&self) -> std::collections::HashMap<String, i64> {
        let mut map = std::collections::HashMap::new();
        let Ok(mut stmt) = self.conn.prepare(
            "SELECT DISTINCT source_file, source_mtime FROM usage_entries",
        ) else {
            return map;
        };
        let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        }) else {
            return map;
        };
        for row in rows.flatten() {
            map.insert(row.0, row.1);
        }
        map
    }

    /// Load ALL cached entries in one query.
    pub fn load_all_entries(&self) -> anyhow::Result<Vec<Record>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM usage_entries ORDER BY timestamp")?;
        let entries = stmt
            .query_map([], Self::row_to_entry)?
            .filter_map(|r| r.ok())
            .collect();
        Ok(entries)
    }

    /// Load cached entries with SQL-level filtering by date range and provider.
    pub fn load_entries_filtered(
        &self,
        since: Option<NaiveDate>,
        until: Option<NaiveDate>,
        providers: &[String],
    ) -> anyhow::Result<Vec<Record>> {
        let mut conditions: Vec<String> = Vec::new();
        let mut param_values: Vec<Value> = Vec::new();

        if let Some(s) = since {
            conditions.push("timestamp >= ?".to_string());
            param_values.push(Value::Text(s.to_string()));
        }

        if let Some(u) = until {
            if let Some(next) = u.succ_opt() {
                conditions.push("timestamp < ?".to_string());
                param_values.push(Value::Text(next.to_string()));
            }
        }

        if !providers.is_empty() {
            let placeholders: Vec<&str> = providers.iter().map(|_| "?").collect();
            conditions.push(format!("provider IN ({})", placeholders.join(",")));
            for p in providers {
                param_values.push(Value::Text(p.clone()));
            }
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };
        let sql = format!(
            "SELECT * FROM usage_entries{} ORDER BY timestamp",
            where_clause
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let entries = stmt
            .query_map(rusqlite::params_from_iter(param_values), Self::row_to_entry)?
            .filter_map(|r| r.ok())
            .collect();
        Ok(entries)
    }

    /// Remove stale entries for a file and store new ones.
    pub fn store_file_entries(
        &self,
        path: &Path,
        mtime_secs: i64,
        entries: &[Record],
    ) -> anyhow::Result<()> {
        let path_str = path.display().to_string();

        self.conn.execute(
            "DELETE FROM usage_entries WHERE source_file = ?1",
            params![path_str],
        )?;

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

    /// Check whether file discovery should be skipped because
    /// the cache was populated recently (within `max_age_secs`).
    #[must_use]
    pub fn should_rediscover(&self, max_age_secs: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let last: Option<u64> = self
            .conn
            .query_row(
                "SELECT value FROM cache_meta WHERE key = 'last_discovery_at'",
                [],
                |row| {
                    let v: String = row.get(0)?;
                    Ok(v.parse::<u64>().unwrap_or(0))
                },
            )
            .ok();

        match last {
            Some(ts) => now.saturating_sub(ts) > max_age_secs,
            None => true,
        }
    }

    /// Record the current time as the last discovery timestamp.
    pub fn set_last_discovery(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let _ = self.conn.execute(
            "INSERT OR REPLACE INTO cache_meta (key, value) VALUES ('last_discovery_at', ?1)",
            params![now.to_string()],
        );
    }

    /// Single source of truth for mapping a SQLite row to a Record.
    fn row_to_entry(row: &Row) -> rusqlite::Result<Record> {
        let ts_str: String = row.get("timestamp")?;
        let timestamp = DateTime::parse_from_rfc3339(&ts_str)
            .map(|dt| dt.to_utc())
            .unwrap_or_else(|_| Utc::now());

        Ok(Record {
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
