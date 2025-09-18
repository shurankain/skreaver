//! PostgreSQL migration engine for schema versioning
//!
//! This module provides database migration functionality with version control
//! and rollback support for PostgreSQL schemas.

use skreaver_core::error::MemoryError;
use super::pool::PostgresPool;

/// PostgreSQL migration engine for schema versioning
pub struct PostgresMigrationEngine {
    migrations: Vec<PostgresMigration>,
}

/// A PostgreSQL database migration
#[derive(Debug, Clone)]
pub struct PostgresMigration {
    pub version: u32,
    pub description: String,
    pub up_sql: String,
    pub down_sql: Option<String>,
}

impl PostgresMigrationEngine {
    pub fn new() -> Self {
        Self {
            migrations: Self::default_migrations(),
        }
    }

    fn default_migrations() -> Vec<PostgresMigration> {
        vec![PostgresMigration {
            version: 1,
            description: "Initial PostgreSQL memory schema".to_string(),
            up_sql: r#"
                CREATE TABLE IF NOT EXISTS memory_entries (
                    key TEXT PRIMARY KEY,
                    value JSONB NOT NULL,
                    namespace TEXT,
                    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
                );

                CREATE INDEX IF NOT EXISTS idx_memory_namespace ON memory_entries(namespace);
                CREATE INDEX IF NOT EXISTS idx_memory_updated_at ON memory_entries(updated_at);
                CREATE INDEX IF NOT EXISTS idx_memory_value_gin ON memory_entries USING gin(value);

                CREATE TABLE IF NOT EXISTS schema_migrations (
                    version INTEGER PRIMARY KEY,
                    description TEXT NOT NULL,
                    applied_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
                );
            "#.to_string(),
            down_sql: Some("DROP TABLE IF EXISTS memory_entries CASCADE; DROP TABLE IF EXISTS schema_migrations CASCADE;".to_string()),
        }]
    }

    pub async fn migrate(
        &self,
        pool: &PostgresPool,
        target_version: Option<u32>,
    ) -> Result<(), MemoryError> {
        let conn = pool.acquire().await?;

        // Get current version
        let current_version: i32 = conn
            .client()
            .query_one(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                &[],
            )
            .await
            .map(|row| row.get(0))
            .unwrap_or(0);

        let target = target_version
            .unwrap_or_else(|| self.migrations.iter().map(|m| m.version).max().unwrap_or(0))
            as i32;

        // Apply migrations
        for migration in &self.migrations {
            let version = migration.version as i32;
            if version > current_version && version <= target {
                self.apply_migration(pool, migration).await?;
            }
        }

        Ok(())
    }

    async fn apply_migration(
        &self,
        pool: &PostgresPool,
        migration: &PostgresMigration,
    ) -> Result<(), MemoryError> {
        let mut conn = pool.acquire().await?;

        // Start transaction
        let tx =
            conn.client_mut()
                .transaction()
                .await
                .map_err(|e| MemoryError::ConnectionFailed {
                    backend: "postgres".to_string(),
                    reason: format!("Failed to start migration transaction: {}", e),
                })?;

        // Execute migration
        tx.batch_execute(&migration.up_sql)
            .await
            .map_err(|e| MemoryError::ConnectionFailed {
                backend: "postgres".to_string(),
                reason: format!("Migration {} failed: {}", migration.version, e),
            })?;

        // Record migration
        tx.execute(
            "INSERT INTO schema_migrations (version, description) VALUES ($1, $2)",
            &[&(migration.version as i32), &migration.description],
        )
        .await
        .map_err(|e| MemoryError::ConnectionFailed {
            backend: "postgres".to_string(),
            reason: format!("Failed to record migration {}: {}", migration.version, e),
        })?;

        tx.commit()
            .await
            .map_err(|e| MemoryError::ConnectionFailed {
                backend: "postgres".to_string(),
                reason: format!("Failed to commit migration {}: {}", migration.version, e),
            })?;

        Ok(())
    }
}

impl Default for PostgresMigrationEngine {
    fn default() -> Self {
        Self::new()
    }
}