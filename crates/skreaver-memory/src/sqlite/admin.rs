//! MemoryAdmin implementation for SqliteMemory

use skreaver_core::error::MemoryError;
use skreaver_core::memory::SnapshotableMemory;

use crate::admin::{BackupFormat, BackupHandle, HealthStatus, MemoryAdmin, MigrationStatus};

use super::SqliteMemory;

impl MemoryAdmin for SqliteMemory {
    fn backup(&self) -> Result<BackupHandle, MemoryError> {
        let snapshot = match SqliteMemory::snapshot(&mut self.clone()) {
            Some(s) => s,
            None => {
                return Err(MemoryError::SnapshotFailed {
                    backend: skreaver_core::error::MemoryBackend::Sqlite,
                    kind: skreaver_core::error::MemoryErrorKind::InternalError {
                        backend_error: "Failed to create snapshot for backup".to_string(),
                    },
                });
            }
        };

        let data = snapshot.as_bytes().to_vec();
        let handle = BackupHandle {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: std::time::SystemTime::now(),
            size_bytes: data.len() as u64,
            format: BackupFormat::Json,
            data,
        };

        Ok(handle)
    }

    fn restore_from_backup(&mut self, handle: BackupHandle) -> Result<(), MemoryError> {
        match handle.format {
            BackupFormat::Json => {
                let snapshot =
                    String::from_utf8(handle.data).map_err(|e| MemoryError::RestoreFailed {
                        backend: skreaver_core::error::MemoryBackend::Sqlite,
                        kind: skreaver_core::error::MemoryErrorKind::SerializationError {
                            details: format!("Invalid UTF-8 in backup data: {}", e),
                        },
                    })?;
                self.restore(&snapshot)
            }
            BackupFormat::SqliteDump => Err(MemoryError::RestoreFailed {
                backend: skreaver_core::error::MemoryBackend::Sqlite,
                kind: skreaver_core::error::MemoryErrorKind::InternalError {
                    backend_error: "SQLite dump format not yet supported".to_string(),
                },
            }),
            BackupFormat::PostgresDump | BackupFormat::Binary => Err(MemoryError::RestoreFailed {
                backend: skreaver_core::error::MemoryBackend::Sqlite,
                kind: skreaver_core::error::MemoryErrorKind::InternalError {
                    backend_error: format!("Unsupported backup format: {:?}", handle.format),
                },
            }),
        }
    }

    fn migrate_to_version(&mut self, version: Option<u32>) -> Result<(), MemoryError> {
        let conn = self.pool.acquire()?;
        self.migration_engine
            .migrate(conn.get_connection(), version)
    }

    fn health_status(&self) -> Result<HealthStatus, MemoryError> {
        let pool_health = self.pool.health_check()?;

        // Try to get row count for additional health info
        let row_count = match self.pool.acquire() {
            Ok(conn) => conn
                .get_connection()
                .query_row("SELECT COUNT(*) FROM memory", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap_or(0),
            Err(_) => -1,
        };

        let status = if pool_health.healthy_connections == pool_health.total_connections {
            HealthStatus::healthy(
                format!(
                    "All {} connections healthy, {} keys stored",
                    pool_health.total_connections, row_count
                ),
                pool_health,
            )
        } else if pool_health.healthy_connections > 0 {
            HealthStatus::degraded(
                format!(
                    "Only {}/{} connections healthy",
                    pool_health.healthy_connections, pool_health.total_connections
                ),
                pool_health,
            )
        } else {
            HealthStatus::unhealthy("No healthy connections available", 1)
        };

        Ok(status)
    }

    fn migration_status(&self) -> Result<MigrationStatus, MemoryError> {
        let conn = self.pool.acquire()?;
        self.migration_engine
            .get_migration_status(conn.get_connection())
    }
}
