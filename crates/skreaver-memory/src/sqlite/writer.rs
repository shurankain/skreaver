//! MemoryWriter implementation for SqliteMemory

use rusqlite::params;

use skreaver_core::error::{MemoryError, MemoryErrorKind, MemoryBackend};
use skreaver_core::memory::{MemoryKey, MemoryUpdate, MemoryWriter};

use super::SqliteMemory;

impl MemoryWriter for SqliteMemory {
    fn store(&mut self, update: MemoryUpdate) -> Result<(), MemoryError> {
        let conn = self.pool.acquire()?;
        let namespaced_key = self.namespaced_key(&update.key);

        conn.as_ref()
            .execute(
                "INSERT INTO memory (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                updated_at = strftime('%s', 'now')",
                params![namespaced_key, update.value],
            )
            .map_err(|e| MemoryError::StoreFailed {
                key: update.key.clone(),
                backend: MemoryBackend::Sqlite,
                kind: MemoryErrorKind::IoError {
                    details: e.to_string(),
                },
            })?;

        Ok(())
    }

    fn store_many(&mut self, updates: Vec<MemoryUpdate>) -> Result<(), MemoryError> {
        if updates.is_empty() {
            return Ok(());
        }

        let mut conn = self.pool.acquire()?;

        let tx =
            conn.as_mut()
                .unchecked_transaction()
                .map_err(|e| MemoryError::ConnectionFailed {
                    backend: MemoryBackend::Sqlite,
                    kind: MemoryErrorKind::InternalError {
                        backend_error: format!("Failed to begin transaction: {}", e),
                    },
                })?;

        {
            let mut stmt = tx
                .prepare(
                    "INSERT INTO memory (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET
                    value = excluded.value,
                    updated_at = strftime('%s', 'now')",
                )
                .map_err(|e| MemoryError::StoreFailed {
                    key: MemoryKey::new("batch").unwrap(),
                    backend: MemoryBackend::Sqlite,
                    kind: MemoryErrorKind::IoError {
                        details: e.to_string(),
                    },
                })?;

            for update in updates {
                let namespaced_key = self.namespaced_key(&update.key);
                stmt.execute(params![namespaced_key, update.value])
                    .map_err(|e| MemoryError::StoreFailed {
                        key: update.key.clone(),
                        backend: MemoryBackend::Sqlite,
                        kind: MemoryErrorKind::IoError {
                            details: e.to_string(),
                        },
                    })?;
            }
        }

        tx.commit().map_err(|e| MemoryError::ConnectionFailed {
            backend: MemoryBackend::Sqlite,
            kind: MemoryErrorKind::InternalError {
                backend_error: format!("Failed to commit transaction: {}", e),
            },
        })?;

        Ok(())
    }
}
