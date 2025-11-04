//! SnapshotableMemory implementation for SqliteMemory

use rusqlite::params;

use skreaver_core::error::{MemoryBackend, MemoryError, MemoryErrorKind};
use skreaver_core::memory::SnapshotableMemory;

use super::SqliteMemory;
use super::pool::SqlitePool;

impl SqliteMemory {
    /// Create snapshot with proper error handling (internal method)
    pub(super) fn create_snapshot(&mut self) -> Result<String, MemoryError> {
        let conn = self
            .pool
            .acquire()
            .map_err(|e| MemoryError::SnapshotFailed {
                backend: MemoryBackend::Sqlite,
                kind: MemoryErrorKind::InternalError {
                    backend_error: format!("Failed to acquire connection for snapshot: {}", e),
                },
            })?;

        let mut stmt = conn
            .as_ref()
            .prepare("SELECT key, value FROM memory")
            .map_err(|e| MemoryError::SnapshotFailed {
                backend: MemoryBackend::Sqlite,
                kind: MemoryErrorKind::IoError {
                    details: SqlitePool::sanitize_error(&e),
                },
            })?;

        let mut snapshot = std::collections::HashMap::new();
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| MemoryError::SnapshotFailed {
                backend: MemoryBackend::Sqlite,
                kind: MemoryErrorKind::IoError {
                    details: SqlitePool::sanitize_error(&e),
                },
            })?;

        for row in rows {
            let (key, value) = row.map_err(|e| MemoryError::SnapshotFailed {
                backend: MemoryBackend::Sqlite,
                kind: MemoryErrorKind::IoError {
                    details: SqlitePool::sanitize_error(&e),
                },
            })?;
            snapshot.insert(key, value);
        }

        serde_json::to_string(&snapshot).map_err(|e| MemoryError::SnapshotFailed {
            backend: MemoryBackend::Sqlite,
            kind: MemoryErrorKind::SerializationError {
                details: format!("Failed to serialize snapshot: {}", e),
            },
        })
    }
}

impl SnapshotableMemory for SqliteMemory {
    fn snapshot(&mut self) -> Option<String> {
        // Use proper error handling internally and log errors for debugging
        match self.create_snapshot() {
            Ok(snapshot) => Some(snapshot),
            Err(e) => {
                eprintln!("Snapshot creation failed: {}", e);
                None // Interface requires Option, but we log the actual error
            }
        }
    }

    fn restore(&mut self, snapshot: &str) -> Result<(), MemoryError> {
        let data: std::collections::HashMap<String, String> = serde_json::from_str(snapshot)
            .map_err(|e| MemoryError::RestoreFailed {
                backend: MemoryBackend::Sqlite,
                kind: MemoryErrorKind::SerializationError {
                    details: format!("Invalid snapshot format: {}", e),
                },
            })?;

        let mut conn = self.pool.acquire()?;

        let tx =
            conn.as_mut()
                .unchecked_transaction()
                .map_err(|e| MemoryError::ConnectionFailed {
                    backend: MemoryBackend::Sqlite,
                    kind: MemoryErrorKind::InternalError {
                        backend_error: format!("Failed to begin restore transaction: {}", e),
                    },
                })?;

        // Clear existing data
        tx.execute("DELETE FROM memory", [])
            .map_err(|e| MemoryError::RestoreFailed {
                backend: MemoryBackend::Sqlite,
                kind: MemoryErrorKind::IoError {
                    details: format!("Failed to clear existing data: {}", e),
                },
            })?;

        // Insert snapshot data
        {
            let mut stmt = tx
                .prepare("INSERT INTO memory (key, value) VALUES (?1, ?2)")
                .map_err(|e| MemoryError::RestoreFailed {
                    backend: MemoryBackend::Sqlite,
                    kind: MemoryErrorKind::IoError {
                        details: e.to_string(),
                    },
                })?;

            for (key, value) in data {
                stmt.execute(params![key, value])
                    .map_err(|e| MemoryError::RestoreFailed {
                        backend: MemoryBackend::Sqlite,
                        kind: MemoryErrorKind::IoError {
                            details: format!("Failed to restore key {}: {}", key, e),
                        },
                    })?;
            }
        }

        tx.commit().map_err(|e| MemoryError::ConnectionFailed {
            backend: MemoryBackend::Sqlite,
            kind: MemoryErrorKind::InternalError {
                backend_error: format!("Failed to commit restore: {}", e),
            },
        })?;

        Ok(())
    }
}
