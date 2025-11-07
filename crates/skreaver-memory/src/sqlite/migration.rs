//! Schema migration engine for SQLite

use rusqlite::{Connection, params};

use skreaver_core::error::{MemoryBackend, MemoryError, MemoryErrorKind};

use crate::admin::{AppliedMigration, MigrationStatus};
use crate::sqlite::timeout::{TimeoutConfig, with_timeout};

/// Migration engine for SQLite
pub struct MigrationEngine {
    migrations: Vec<Migration>,
    timeout_config: TimeoutConfig,
}

/// Individual migration definition
#[derive(Debug, Clone)]
pub struct Migration {
    pub version: u32,
    pub description: String,
    pub up: String,
    #[allow(dead_code)]
    pub down: Option<String>,
}

impl MigrationEngine {
    /// Create a new migration engine with default timeout configuration
    pub fn new() -> Self {
        Self::with_timeout_config(TimeoutConfig::default())
    }

    /// Create a new migration engine with custom timeout configuration
    pub fn with_timeout_config(timeout_config: TimeoutConfig) -> Self {
        Self {
            migrations: Self::default_migrations(),
            timeout_config,
        }
    }

    /// Define the default migrations for the memory backend
    fn default_migrations() -> Vec<Migration> {
        vec![Migration {
            version: 1,
            description: "Create initial memory table".to_string(),
            up: r#"
                    CREATE TABLE IF NOT EXISTS memory (
                        key TEXT PRIMARY KEY,
                        value TEXT NOT NULL,
                        created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                        updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
                    );
                    
                    CREATE INDEX IF NOT EXISTS idx_memory_updated_at ON memory(updated_at);
                "#
            .to_string(),
            down: Some("DROP TABLE IF EXISTS memory;".to_string()),
        }]
    }

    /// Run migrations up to the specified version
    pub fn migrate(
        &self,
        conn: &Connection,
        target_version: Option<u32>,
    ) -> Result<(), MemoryError> {
        // Create migration tracking table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                description TEXT NOT NULL,
                applied_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            )",
            [],
        )
        .map_err(|e| MemoryError::ConnectionFailed {
            backend: MemoryBackend::Sqlite,
            kind: MemoryErrorKind::InternalError {
                backend_error: format!("Failed to create migrations table: {}", e),
            },
        })?;

        // Get current version
        let current_version: u32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let target = target_version
            .unwrap_or_else(|| self.migrations.iter().map(|m| m.version).max().unwrap_or(0));

        // Apply migrations
        for migration in &self.migrations {
            if migration.version > current_version && migration.version <= target {
                self.apply_migration(conn, migration)?;
            }
        }

        Ok(())
    }

    /// Apply a single migration
    fn apply_migration(&self, conn: &Connection, migration: &Migration) -> Result<(), MemoryError> {
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| MemoryError::ConnectionFailed {
                backend: MemoryBackend::Sqlite,
                kind: MemoryErrorKind::InternalError {
                    backend_error: format!("Failed to start migration transaction: {}", e),
                },
            })?;

        // Execute migration
        tx.execute_batch(&migration.up)
            .map_err(|e| MemoryError::ConnectionFailed {
                backend: MemoryBackend::Sqlite,
                kind: MemoryErrorKind::InternalError {
                    backend_error: format!("Migration {} failed: {}", migration.version, e),
                },
            })?;

        // Record migration
        tx.execute(
            "INSERT INTO schema_migrations (version, description) VALUES (?1, ?2)",
            params![migration.version, migration.description],
        )
        .map_err(|e| MemoryError::ConnectionFailed {
            backend: MemoryBackend::Sqlite,
            kind: MemoryErrorKind::InternalError {
                backend_error: format!("Failed to record migration {}: {}", migration.version, e),
            },
        })?;

        tx.commit().map_err(|e| MemoryError::ConnectionFailed {
            backend: MemoryBackend::Sqlite,
            kind: MemoryErrorKind::InternalError {
                backend_error: format!("Failed to commit migration {}: {}", migration.version, e),
            },
        })?;

        Ok(())
    }

    /// Rollback to a specific version
    #[allow(dead_code)]
    pub fn rollback(&self, conn: &Connection, target_version: u32) -> Result<(), MemoryError> {
        let current_version: u32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Apply down migrations in reverse order
        let mut migrations_to_rollback: Vec<_> = self
            .migrations
            .iter()
            .filter(|m| m.version > target_version && m.version <= current_version)
            .collect();
        migrations_to_rollback.sort_by(|a, b| b.version.cmp(&a.version)); // Reverse order

        for migration in migrations_to_rollback {
            if let Some(ref down_sql) = migration.down {
                let tx =
                    conn.unchecked_transaction()
                        .map_err(|e| MemoryError::ConnectionFailed {
                            backend: MemoryBackend::Sqlite,
                            kind: MemoryErrorKind::InternalError {
                                backend_error: format!(
                                    "Failed to start rollback transaction: {}",
                                    e
                                ),
                            },
                        })?;

                tx.execute_batch(down_sql)
                    .map_err(|e| MemoryError::ConnectionFailed {
                        backend: MemoryBackend::Sqlite,
                        kind: MemoryErrorKind::InternalError {
                            backend_error: format!(
                                "Rollback of migration {} failed: {}",
                                migration.version, e
                            ),
                        },
                    })?;

                tx.execute(
                    "DELETE FROM schema_migrations WHERE version = ?1",
                    params![migration.version],
                )
                .map_err(|e| MemoryError::ConnectionFailed {
                    backend: MemoryBackend::Sqlite,
                    kind: MemoryErrorKind::InternalError {
                        backend_error: format!(
                            "Failed to remove migration record {}: {}",
                            migration.version, e
                        ),
                    },
                })?;

                tx.commit().map_err(|e| MemoryError::ConnectionFailed {
                    backend: MemoryBackend::Sqlite,
                    kind: MemoryErrorKind::InternalError {
                        backend_error: format!(
                            "Failed to commit rollback {}: {}",
                            migration.version, e
                        ),
                    },
                })?;
            } else {
                return Err(MemoryError::ConnectionFailed {
                    backend: MemoryBackend::Sqlite,
                    kind: MemoryErrorKind::InternalError {
                        backend_error: format!(
                            "Migration {} has no down migration defined",
                            migration.version
                        ),
                    },
                });
            }
        }

        Ok(())
    }

    /// Get migration status information
    #[allow(dead_code)]
    pub fn get_migration_status(&self, conn: &Connection) -> Result<MigrationStatus, MemoryError> {
        let current_version: u32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let latest_version = self.migrations.iter().map(|m| m.version).max().unwrap_or(0);

        let pending_migrations: Vec<u32> = self
            .migrations
            .iter()
            .filter(|m| m.version > current_version)
            .map(|m| m.version)
            .collect();

        // Get applied migrations from database
        let mut stmt = conn
            .prepare(
                "SELECT version, description, applied_at FROM schema_migrations ORDER BY version",
            )
            .map_err(|e| MemoryError::ConnectionFailed {
                backend: MemoryBackend::Sqlite,
                kind: MemoryErrorKind::InternalError {
                    backend_error: format!("Failed to query applied migrations: {}", e),
                },
            })?;

        let applied_migrations: Result<Vec<AppliedMigration>, _> = stmt
            .query_map([], |row| {
                let timestamp: i64 = row.get(2)?;
                let system_time =
                    std::time::UNIX_EPOCH + std::time::Duration::from_secs(timestamp as u64);

                Ok(AppliedMigration {
                    version: row.get(0)?,
                    description: row.get(1)?,
                    applied_at: system_time,
                })
            })
            .map_err(|e| MemoryError::ConnectionFailed {
                backend: MemoryBackend::Sqlite,
                kind: MemoryErrorKind::InternalError {
                    backend_error: format!("Failed to query applied migrations: {}", e),
                },
            })?
            .collect();

        let applied_migrations = applied_migrations.map_err(|e| MemoryError::ConnectionFailed {
            backend: MemoryBackend::Sqlite,
            kind: MemoryErrorKind::InternalError {
                backend_error: format!("Failed to parse applied migrations: {}", e),
            },
        })?;

        Ok(MigrationStatus {
            current_version,
            latest_version,
            pending_migrations,
            applied_migrations,
        })
    }
}
