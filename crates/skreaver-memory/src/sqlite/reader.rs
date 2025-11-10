//! MemoryReader implementation for SqliteMemory

use rusqlite::{OptionalExtension, params};
use std::sync::OnceLock;

use skreaver_core::error::{MemoryBackend, MemoryError, MemoryErrorKind};
use skreaver_core::memory::{MemoryKey, MemoryReader};

use super::SqliteMemory;

/// Fallback memory key for batch operations
static BATCH_KEY: OnceLock<MemoryKey> = OnceLock::new();

fn batch_key() -> &'static MemoryKey {
    BATCH_KEY
        .get_or_init(|| MemoryKey::new("batch").expect("BUG: 'batch' should be a valid memory key"))
}

impl MemoryReader for SqliteMemory {
    fn load(&self, key: &MemoryKey) -> Result<Option<String>, MemoryError> {
        let conn = self.pool.acquire()?;
        let namespaced_key = self.namespaced_key(key);

        conn.get_connection()
            .query_row(
                "SELECT value FROM memory WHERE key = ?1",
                params![namespaced_key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| MemoryError::LoadFailed {
                key: key.clone(),
                backend: MemoryBackend::Sqlite,
                kind: MemoryErrorKind::IoError {
                    details: e.to_string(),
                },
            })
    }

    fn load_many(&self, keys: &[MemoryKey]) -> Result<Vec<Option<String>>, MemoryError> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        let conn = self.pool.acquire()?;
        let namespaced_keys: Vec<String> = keys.iter().map(|k| self.namespaced_key(k)).collect();
        let placeholders = vec!["?"; namespaced_keys.len()].join(",");
        let query = format!(
            "SELECT key, value FROM memory WHERE key IN ({})",
            placeholders
        );

        let mut stmt = conn.prepare(&query).map_err(|e| MemoryError::LoadFailed {
            key: batch_key().clone(),
            backend: MemoryBackend::Sqlite,
            kind: MemoryErrorKind::IoError {
                details: e.to_string(),
            },
        })?;

        let mut results = std::collections::HashMap::new();
        let params: Vec<&dyn rusqlite::ToSql> = namespaced_keys
            .iter()
            .map(|k| k as &dyn rusqlite::ToSql)
            .collect();

        let rows = stmt
            .query_map(&params[..], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| MemoryError::LoadFailed {
                key: batch_key().clone(),
                backend: MemoryBackend::Sqlite,
                kind: MemoryErrorKind::IoError {
                    details: e.to_string(),
                },
            })?;

        for row in rows {
            let (k, v) = row.map_err(|e| MemoryError::LoadFailed {
                key: batch_key().clone(),
                backend: MemoryBackend::Sqlite,
                kind: MemoryErrorKind::IoError {
                    details: e.to_string(),
                },
            })?;
            results.insert(k, v);
        }

        // Return in the same order as requested
        Ok(namespaced_keys
            .iter()
            .map(|k| results.get(k).cloned())
            .collect())
    }
}
