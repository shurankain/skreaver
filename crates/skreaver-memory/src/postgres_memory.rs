//! PostgreSQL-based memory backend with ACID compliance, connection pooling, and advanced features
//!
//! This module provides an enterprise-grade PostgreSQL backend with:
//! - Full ACID compliance with proper transaction isolation levels
//! - Advanced connection pooling with health monitoring
//! - Schema migration support with versioning and rollback
//! - JSON support for structured data storage
//! - Comprehensive security and error handling

use std::sync::Arc;
use tokio_postgres::IsolationLevel;

use skreaver_core::error::{MemoryError, TransactionError};
use skreaver_core::memory::{
    MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter, SnapshotableMemory, TransactionalMemory,
};

// Import shared admin types
use crate::admin::{
    AppliedMigration, BackupFormat, BackupHandle, HealthStatus, MemoryAdmin, MigrationStatus,
    PoolHealth,
};

// Use the modular components
use crate::postgres::{
    PostgresConfig, PostgresMigrationEngine, PostgresPool, PostgresTransactionalMemory,
};

/// PostgreSQL memory backend with enterprise features
pub struct PostgresMemory {
    pool: Arc<PostgresPool>,
    namespace: Option<String>,
}

impl PostgresMemory {
    /// Create a new PostgreSQL memory backend
    pub async fn new(config: PostgresConfig) -> Result<Self, MemoryError> {
        let pool = Arc::new(PostgresPool::new(config).await?);

        // Initialize database schema using migration engine
        let migration_engine = PostgresMigrationEngine::new();
        migration_engine.migrate(&pool, None).await?;

        Ok(Self {
            pool,
            namespace: None,
        })
    }

    /// Create with connection string
    pub async fn from_url(url: &str) -> Result<Self, MemoryError> {
        let config = PostgresConfig::from_url(url)?;
        Self::new(config).await
    }

    /// Set namespace for key isolation
    pub fn with_namespace(mut self, namespace: String) -> Result<Self, MemoryError> {
        // Validate namespace for security
        Self::validate_namespace(&namespace)?;
        self.namespace = Some(namespace);
        Ok(self)
    }

    /// Validate namespace string for security
    fn validate_namespace(namespace: &str) -> Result<(), MemoryError> {
        if namespace.is_empty() {
            return Err(MemoryError::ConnectionFailed {
                backend: skreaver_core::error::MemoryBackend::Postgres,
                kind: skreaver_core::error::MemoryErrorKind::InvalidKey {
                    validation_error: "Namespace cannot be empty".to_string(),
                },
            });
        }

        if namespace.len() > 64 {
            return Err(MemoryError::ConnectionFailed {
                backend: skreaver_core::error::MemoryBackend::Postgres,
                kind: skreaver_core::error::MemoryErrorKind::InvalidKey {
                    validation_error: "Namespace too long (max 64 characters)".to_string(),
                },
            });
        }

        // Only allow safe characters
        if !namespace
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(MemoryError::ConnectionFailed {
                backend: skreaver_core::error::MemoryBackend::Postgres,
                kind: skreaver_core::error::MemoryErrorKind::InvalidKey {
                    validation_error:
                        "Namespace contains invalid characters (only alphanumeric, _, - allowed)"
                            .to_string(),
                },
            });
        }

        Ok(())
    }

    /// Get namespaced key
    fn namespaced_key(&self, key: &MemoryKey) -> String {
        match &self.namespace {
            Some(ns) => format!("{}:{}", ns, key.as_str()),
            None => key.as_str().to_string(),
        }
    }

    /// Async load operation
    pub async fn load_async(&self, key: &MemoryKey) -> Result<Option<String>, MemoryError> {
        let conn = self.pool.acquire().await?;
        let namespaced_key = self.namespaced_key(key);

        let row = conn
            .client()
            .query_opt(
                "SELECT value FROM memory_entries WHERE key = $1",
                &[&namespaced_key],
            )
            .await
            .map_err(|e| MemoryError::LoadFailed {
                key: key.clone(),
                backend: skreaver_core::error::MemoryBackend::Postgres,
                kind: skreaver_core::error::MemoryErrorKind::IoError {
                    details: format!("Database error: {}", e),
                },
            })?;

        match row {
            Some(row) => {
                let value: String = row.get(0);
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Async store operation
    pub async fn store_async(&self, update: MemoryUpdate) -> Result<(), MemoryError> {
        let conn = self.pool.acquire().await?;
        let namespaced_key = self.namespaced_key(&update.key);

        conn.execute(
            r#"
                INSERT INTO memory_entries (key, value, namespace, updated_at)
                VALUES ($1, $2, $3, NOW())
                ON CONFLICT (key) DO UPDATE SET
                    value = EXCLUDED.value,
                    updated_at = NOW()
                "#,
            &[
                &namespaced_key,
                &update.value,
                &self.namespace.as_deref().unwrap_or("").to_string(),
            ],
        )
        .await
        .map_err(|e| MemoryError::StoreFailed {
            key: update.key.clone(),
            backend: skreaver_core::error::MemoryBackend::Postgres,
            kind: skreaver_core::error::MemoryErrorKind::IoError {
                details: format!("Database error: {}", e),
            },
        })?;

        Ok(())
    }

    /// Get all data for snapshot operations
    async fn get_all_data(&self) -> Result<std::collections::HashMap<String, String>, MemoryError> {
        let conn = self.pool.acquire().await?;

        let namespace_filter = match &self.namespace {
            Some(ns) => format!("WHERE namespace = '{}'", ns),
            None => "WHERE namespace = ''".to_string(),
        };

        let query = format!("SELECT key, value FROM memory_entries {}", namespace_filter);

        let rows = conn
            .client()
            .query(&query, &[])
            .await
            .map_err(|e| MemoryError::LoadFailed {
                key: MemoryKey::new("snapshot").unwrap(),
                backend: skreaver_core::error::MemoryBackend::Postgres,
                kind: skreaver_core::error::MemoryErrorKind::IoError {
                    details: format!("Database error: {}", e),
                },
            })?;

        let mut data = std::collections::HashMap::new();
        for row in rows {
            let key: String = row.get(0);
            let value: String = row.get(1);

            // Remove namespace prefix if present
            let clean_key = match &self.namespace {
                Some(ns) => {
                    let prefix = format!("{}:", ns);
                    key.strip_prefix(&prefix).unwrap_or(&key).to_string()
                }
                None => key,
            };

            data.insert(clean_key, value);
        }

        Ok(data)
    }

    /// Clear all data for restore operations
    async fn clear_all_data(&self) -> Result<(), MemoryError> {
        let conn = self.pool.acquire().await?;

        let namespace_filter = match &self.namespace {
            Some(ns) => format!("WHERE namespace = '{}'", ns),
            None => "WHERE namespace = ''".to_string(),
        };

        let query = format!("DELETE FROM memory_entries {}", namespace_filter);

        conn.execute(&query, &[])
            .await
            .map_err(|e| MemoryError::StoreFailed {
                key: MemoryKey::new("clear_all").unwrap(),
                backend: skreaver_core::error::MemoryBackend::Postgres,
                kind: skreaver_core::error::MemoryErrorKind::IoError {
                    details: format!("Database error: {}", e),
                },
            })?;

        Ok(())
    }
}

impl MemoryReader for PostgresMemory {
    fn load(&self, key: &MemoryKey) -> Result<Option<String>, MemoryError> {
        // Block on async operation
        let rt = tokio::runtime::Handle::current();
        rt.block_on(self.load_async(key))
    }

    fn load_many(&self, keys: &[MemoryKey]) -> Result<Vec<Option<String>>, MemoryError> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        // Block on async operation
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            let mut results = Vec::with_capacity(keys.len());
            for key in keys {
                results.push(self.load_async(key).await?);
            }
            Ok(results)
        })
    }
}

impl Clone for PostgresMemory {
    fn clone(&self) -> Self {
        Self {
            pool: Arc::clone(&self.pool),
            namespace: self.namespace.clone(),
        }
    }
}

impl MemoryWriter for PostgresMemory {
    fn store(&mut self, update: MemoryUpdate) -> Result<(), MemoryError> {
        // Block on async operation
        let rt = tokio::runtime::Handle::current();
        rt.block_on(self.store_async(update))
    }

    fn store_many(&mut self, updates: Vec<MemoryUpdate>) -> Result<(), MemoryError> {
        // Block on async operation
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            for update in updates {
                self.store_async(update).await?;
            }
            Ok(())
        })
    }
}

impl SnapshotableMemory for PostgresMemory {
    fn snapshot(&mut self) -> Option<String> {
        // PostgreSQL provides snapshot isolation through its MVCC implementation
        // For a simple implementation, we'll export all data as JSON
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            match self.get_all_data().await {
                Ok(data) => serde_json::to_string(&data).ok(),
                Err(_) => None,
            }
        })
    }

    fn restore(&mut self, snapshot: &str) -> Result<(), MemoryError> {
        // Parse snapshot and restore all data
        let data: std::collections::HashMap<String, String> = serde_json::from_str(snapshot)
            .map_err(|e| MemoryError::RestoreFailed {
                backend: skreaver_core::error::MemoryBackend::Postgres,
                kind: skreaver_core::error::MemoryErrorKind::SerializationError {
                    details: format!("Failed to parse snapshot: {}", e),
                },
            })?;

        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            // Clear existing data and restore from snapshot
            self.clear_all_data().await?;
            for (key, value) in data {
                let memory_key = MemoryKey::new(&key).map_err(|e| MemoryError::RestoreFailed {
                    backend: skreaver_core::error::MemoryBackend::Postgres,
                    kind: skreaver_core::error::MemoryErrorKind::InvalidKey {
                        validation_error: format!("Invalid key in snapshot: {}", e),
                    },
                })?;
                let update = MemoryUpdate::from_validated(memory_key, value);
                self.store(update)?;
            }
            Ok(())
        })
    }
}

impl TransactionalMemory for PostgresMemory {
    fn transaction<F, R>(&mut self, f: F) -> Result<R, TransactionError>
    where
        F: FnOnce(&mut dyn MemoryWriter) -> Result<R, TransactionError>,
    {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            let mut conn =
                self.pool
                    .acquire()
                    .await
                    .map_err(|e| TransactionError::TransactionFailed {
                        reason: format!("Failed to acquire connection: {}", e),
                    })?;

            let tx = conn
                .client_mut()
                .build_transaction()
                .isolation_level(IsolationLevel::Serializable)
                .start()
                .await
                .map_err(|e| TransactionError::TransactionFailed {
                    reason: format!("Failed to start transaction: {}", e),
                })?;

            // Use the proper transactional wrapper
            let mut tx_memory = PostgresTransactionalMemory::new(tx, self.namespace.clone());
            let result = f(&mut tx_memory);

            match result {
                Ok(r) => {
                    tx_memory
                        .commit()
                        .await
                        .map_err(|e| TransactionError::TransactionFailed {
                            reason: format!("Failed to commit transaction: {}", e),
                        })?;
                    Ok(r)
                }
                Err(e) => {
                    let _ = tx_memory.rollback().await; // Best effort rollback
                    Err(e)
                }
            }
        })
    }
}

// MemoryAdmin implementation for PostgresMemory
impl MemoryAdmin for PostgresMemory {
    fn backup(&self) -> Result<BackupHandle, MemoryError> {
        // Block on async operation since trait is sync
        let rt =
            tokio::runtime::Handle::try_current().map_err(|_| MemoryError::SnapshotFailed {
                backend: skreaver_core::error::MemoryBackend::Postgres,
                kind: skreaver_core::error::MemoryErrorKind::InternalError {
                    backend_error: "No tokio runtime available".to_string(),
                },
            })?;

        rt.block_on(async {
            let conn = self.pool.acquire().await?;

            // Query all entries
            let rows = conn
                .client()
                .query("SELECT key, value FROM memory_entries ORDER BY key", &[])
                .await
                .map_err(|e| MemoryError::SnapshotFailed {
                    backend: skreaver_core::error::MemoryBackend::Postgres,
                    kind: skreaver_core::error::MemoryErrorKind::IoError {
                        details: format!("Failed to query entries: {}", e),
                    },
                })?;

            // Convert to JSON format
            let mut data_map = std::collections::HashMap::new();
            for row in rows {
                let key: String = row.get(0);
                let value: serde_json::Value = row.get(1);
                // Convert JSONB to string
                data_map.insert(key, value.to_string());
            }

            let json = serde_json::to_vec(&data_map).map_err(|e| MemoryError::SnapshotFailed {
                backend: skreaver_core::error::MemoryBackend::Postgres,
                kind: skreaver_core::error::MemoryErrorKind::SerializationError {
                    details: format!("Failed to serialize backup: {}", e),
                },
            })?;

            Ok(BackupHandle::new(BackupFormat::Json, json))
        })
    }

    fn restore_from_backup(&mut self, handle: BackupHandle) -> Result<(), MemoryError> {
        match handle.format {
            BackupFormat::Json => {
                let rt = tokio::runtime::Handle::try_current().map_err(|_| {
                    MemoryError::RestoreFailed {
                        backend: skreaver_core::error::MemoryBackend::Postgres,
                        kind: skreaver_core::error::MemoryErrorKind::InternalError {
                            backend_error: "No tokio runtime available".to_string(),
                        },
                    }
                })?;

                rt.block_on(async {
                    let data_map: std::collections::HashMap<String, String> =
                        serde_json::from_slice(&handle.data).map_err(|e| {
                            MemoryError::RestoreFailed {
                                backend: skreaver_core::error::MemoryBackend::Postgres,
                                kind: skreaver_core::error::MemoryErrorKind::SerializationError {
                                    details: format!("Failed to deserialize backup: {}", e),
                                },
                            }
                        })?;

                    let conn = self.pool.acquire().await?;

                    // Clear existing data (without transaction for simplicity)
                    conn.execute("DELETE FROM memory_entries", &[])
                        .await
                        .map_err(|e| MemoryError::RestoreFailed {
                            backend: skreaver_core::error::MemoryBackend::Postgres,
                            kind: skreaver_core::error::MemoryErrorKind::IoError {
                                details: format!("Failed to clear entries: {}", e),
                            },
                        })?;

                    // Restore data
                    for (key, value) in data_map {
                        let value_json: serde_json::Value = serde_json::from_str(&value)
                            .unwrap_or(serde_json::Value::String(value));

                        conn.execute(
                            "INSERT INTO memory_entries (key, value) VALUES ($1, $2)",
                            &[&key, &value_json],
                        )
                        .await
                        .map_err(|e| MemoryError::RestoreFailed {
                            backend: skreaver_core::error::MemoryBackend::Postgres,
                            kind: skreaver_core::error::MemoryErrorKind::IoError {
                                details: format!("Failed to insert key {}: {}", key, e),
                            },
                        })?;
                    }

                    Ok(())
                })
            }
            BackupFormat::PostgresDump => Err(MemoryError::RestoreFailed {
                backend: skreaver_core::error::MemoryBackend::Postgres,
                kind: skreaver_core::error::MemoryErrorKind::InternalError {
                    backend_error: "PostgreSQL dump format not yet supported".to_string(),
                },
            }),
            BackupFormat::SqliteDump | BackupFormat::Binary => Err(MemoryError::RestoreFailed {
                backend: skreaver_core::error::MemoryBackend::Postgres,
                kind: skreaver_core::error::MemoryErrorKind::InternalError {
                    backend_error: format!("Unsupported backup format: {:?}", handle.format),
                },
            }),
        }
    }

    fn migrate_to_version(&mut self, version: Option<u32>) -> Result<(), MemoryError> {
        let rt =
            tokio::runtime::Handle::try_current().map_err(|_| MemoryError::ConnectionFailed {
                backend: skreaver_core::error::MemoryBackend::Postgres,
                kind: skreaver_core::error::MemoryErrorKind::InternalError {
                    backend_error: "No tokio runtime available".to_string(),
                },
            })?;

        rt.block_on(async {
            let migration_engine = PostgresMigrationEngine::new();
            migration_engine.migrate(&self.pool, version).await
        })
    }

    fn health_status(&self) -> Result<HealthStatus, MemoryError> {
        let rt =
            tokio::runtime::Handle::try_current().map_err(|_| MemoryError::ConnectionFailed {
                backend: skreaver_core::error::MemoryBackend::Postgres,
                kind: skreaver_core::error::MemoryErrorKind::InternalError {
                    backend_error: "No tokio runtime available".to_string(),
                },
            })?;

        rt.block_on(async {
            let health = self.pool.health_check().await?;

            let pool_health = PoolHealth {
                healthy_connections: health.active_connections,
                total_connections: health.total_connections,
                last_check: std::time::SystemTime::now(),
            };

            // Check if pool is healthy (all connections active)
            let is_healthy = health.active_connections > 0
                && health.active_connections == health.total_connections;

            if is_healthy {
                Ok(HealthStatus::healthy(
                    format!(
                        "PostgreSQL pool healthy: {}/{} connections (server: {})",
                        health.active_connections, health.total_connections, health.server_version
                    ),
                    pool_health,
                ))
            } else {
                Ok(HealthStatus::degraded(
                    format!(
                        "Pool degraded: {}/{} connections active",
                        health.active_connections, health.total_connections
                    ),
                    pool_health,
                ))
            }
        })
    }

    fn migration_status(&self) -> Result<MigrationStatus, MemoryError> {
        let rt =
            tokio::runtime::Handle::try_current().map_err(|_| MemoryError::ConnectionFailed {
                backend: skreaver_core::error::MemoryBackend::Postgres,
                kind: skreaver_core::error::MemoryErrorKind::InternalError {
                    backend_error: "No tokio runtime available".to_string(),
                },
            })?;

        rt.block_on(async {
            let conn = self.pool.acquire().await?;

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

            // Get applied migrations
            let rows = conn
                .client()
                .query(
                    "SELECT version, description, applied_at FROM schema_migrations ORDER BY version",
                    &[],
                )
                .await
                .map_err(|e| MemoryError::ConnectionFailed {
                    backend: skreaver_core::error::MemoryBackend::Postgres,
                    kind: skreaver_core::error::MemoryErrorKind::IoError {
                        details: format!("Failed to query migrations: {}", e),
                    },
                })?;

            let mut applied_migrations = Vec::new();
            for row in rows {
                let version: i32 = row.get(0);
                let description: String = row.get(1);
                let applied_at: chrono::DateTime<chrono::Utc> = row.get(2);

                applied_migrations.push(AppliedMigration {
                    version: version as u32,
                    description,
                    applied_at: applied_at.into(),
                });
            }

            // Get latest available version from migration engine
            let migration_engine = PostgresMigrationEngine::new();
            let latest_version = migration_engine.latest_version();

            // Calculate pending migrations
            let pending_migrations: Vec<u32> = ((current_version as u32 + 1)..=latest_version).collect();

            Ok(MigrationStatus {
                current_version: current_version as u32,
                latest_version,
                pending_migrations,
                applied_migrations,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require a running PostgreSQL instance
    // In CI/CD, we would use testcontainers or similar

    #[tokio::test]
    async fn test_postgres_config_validation() {
        let config = PostgresConfig {
            host: "".to_string(),
            ..Default::default()
        };

        let result = PostgresMemory::new(config).await;
        assert!(result.is_err());

        let _valid_config = PostgresConfig::default();
        // This would fail without a real PostgreSQL instance
        // assert!(PostgresMemory::new(valid_config).await.is_ok());
    }

    #[tokio::test]
    async fn test_postgres_config_from_url() {
        let url = "postgresql://user:pass@localhost:5432/testdb";
        let config = PostgresConfig::from_url(url).unwrap();

        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 5432);
        assert_eq!(config.database, "testdb");
        assert_eq!(config.user, "user");
        assert_eq!(config.password, Some("pass".to_string()));
    }

    // Additional tests would require PostgreSQL instance
    // We'll add integration tests later
}
