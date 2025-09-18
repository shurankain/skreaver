//! SQLite schema migration engine
//!
//! This module provides a migration system for managing SQLite database schema
//! changes with versioning, rollback support, and migration tracking.

use rusqlite::{Connection, params};
use skreaver_core::error::MemoryError;

/// Migration engine for managing database schema changes
pub struct MigrationEngine {
    migrations: Vec<Migration>,
}

/// Individual migration definition
pub struct Migration {
    pub version: u32,
    pub description: String,
    pub up: String,
    pub down: Option<String>,
}

/// Status information about database migrations
pub struct MigrationStatus {
    pub current_version: u32,
    pub latest_version: u32,
    pub pending_migrations: Vec<u32>,
    pub applied_migrations: Vec<AppliedMigration>,
}

/// Information about an applied migration
pub struct AppliedMigration {
    pub version: u32,
    pub description: String,
    pub applied_at: i64,
}

impl MigrationEngine {
    /// Create a new migration engine
    pub fn new() -> Self {
        Self {
            migrations: Self::default_migrations(),
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
            backend: "sqlite".to_string(),
            reason: format!("Failed to create migrations table: {}", e),
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
                backend: "sqlite".to_string(),
                reason: format!("Failed to start migration transaction: {}", e),
            })?;

        // Execute migration
        tx.execute_batch(&migration.up)
            .map_err(|e| MemoryError::ConnectionFailed {
                backend: "sqlite".to_string(),
                reason: format!("Migration {} failed: {}", migration.version, e),
            })?;

        // Record migration
        tx.execute(
            "INSERT INTO schema_migrations (version, description) VALUES (?1, ?2)",
            params![migration.version, migration.description],
        )
        .map_err(|e| MemoryError::ConnectionFailed {
            backend: "sqlite".to_string(),
            reason: format!("Failed to record migration {}: {}", migration.version, e),
        })?;

        tx.commit().map_err(|e| MemoryError::ConnectionFailed {
            backend: "sqlite".to_string(),
            reason: format!("Failed to commit migration {}: {}", migration.version, e),
        })?;

        Ok(())
    }

    /// Rollback to a specific version
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
                            backend: "sqlite".to_string(),
                            reason: format!("Failed to start rollback transaction: {}", e),
                        })?;

                tx.execute_batch(down_sql)
                    .map_err(|e| MemoryError::ConnectionFailed {
                        backend: "sqlite".to_string(),
                        reason: format!(
                            "Rollback of migration {} failed: {}",
                            migration.version, e
                        ),
                    })?;

                tx.execute(
                    "DELETE FROM schema_migrations WHERE version = ?1",
                    params![migration.version],
                )
                .map_err(|e| MemoryError::ConnectionFailed {
                    backend: "sqlite".to_string(),
                    reason: format!(
                        "Failed to remove migration record {}: {}",
                        migration.version, e
                    ),
                })?;

                tx.commit().map_err(|e| MemoryError::ConnectionFailed {
                    backend: "sqlite".to_string(),
                    reason: format!("Failed to commit rollback {}: {}", migration.version, e),
                })?;
            } else {
                return Err(MemoryError::ConnectionFailed {
                    backend: "sqlite".to_string(),
                    reason: format!(
                        "Migration {} has no down migration defined",
                        migration.version
                    ),
                });
            }
        }

        Ok(())
    }

    /// Get migration status information
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

        // Get applied migrations
        let mut stmt = conn
            .prepare(
                "SELECT version, description, applied_at FROM schema_migrations ORDER BY version",
            )
            .map_err(|e| MemoryError::ConnectionFailed {
                backend: "sqlite".to_string(),
                reason: format!("Failed to prepare migration query: {}", e),
            })?;

        let applied_migrations: Vec<AppliedMigration> = stmt
            .query_map([], |row| {
                Ok(AppliedMigration {
                    version: row.get(0)?,
                    description: row.get(1)?,
                    applied_at: row.get(2)?,
                })
            })
            .map_err(|e| MemoryError::ConnectionFailed {
                backend: "sqlite".to_string(),
                reason: format!("Failed to query migrations: {}", e),
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| MemoryError::ConnectionFailed {
                backend: "sqlite".to_string(),
                reason: format!("Failed to read migration data: {}", e),
            })?;

        Ok(MigrationStatus {
            current_version,
            latest_version,
            pending_migrations,
            applied_migrations,
        })
    }

    /// Add a custom migration to the engine
    pub fn add_migration(&mut self, migration: Migration) {
        self.migrations.push(migration);
        self.migrations.sort_by(|a, b| a.version.cmp(&b.version));
    }
}

impl Default for MigrationEngine {
    fn default() -> Self {
        Self::new()
    }
}
