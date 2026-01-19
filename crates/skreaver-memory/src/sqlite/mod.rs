//! SQLite-based memory backend with WAL mode, connection pooling, and migrations
//!
//! This module provides a production-ready SQLite backend with:
//! - WAL (Write-Ahead Logging) mode for better concurrency
//! - Thread-safe connection pooling for efficient resource usage
//! - Schema migration support with versioning and rollback
//! - Health monitoring and admin operations

use std::path::Path;
use std::sync::Arc;

use skreaver_core::error::MemoryError;
use skreaver_core::memory::MemoryKey;

// Module declarations
mod admin;
pub mod migration;
pub mod pool;
mod reader;
mod snapshot;
pub mod timeout;
mod transaction;
mod writer;

// Re-exports for public API
pub use migration::{Migration, MigrationEngine};
pub use pool::{PooledConnection, SqlitePool};
pub use timeout::TimeoutConfig;

/// SQLite-based memory backend with all Phase 1.1 features
pub struct SqliteMemory {
    pub(crate) pool: Arc<SqlitePool>,
    pub(crate) migration_engine: Arc<MigrationEngine>,
    pub(crate) namespace: Option<String>,
}

impl SqliteMemory {
    /// Create a new SQLite memory backend
    pub fn new(path: impl AsRef<Path>) -> Result<Self, MemoryError> {
        Self::with_pool_size(path, 5) // Default to 5 connections
    }

    /// Create with custom pool size
    pub fn with_pool_size(path: impl AsRef<Path>, pool_size: usize) -> Result<Self, MemoryError> {
        let pool = Arc::new(SqlitePool::new(path, pool_size)?);
        let migration_engine = Arc::new(MigrationEngine::new());

        // Run migrations on first connection
        let conn = pool.acquire()?;
        migration_engine.migrate(&conn, None)?;

        Ok(Self {
            pool,
            migration_engine,
            namespace: None,
        })
    }

    /// Set a namespace for key isolation
    pub fn with_namespace(mut self, namespace: String) -> Result<Self, MemoryError> {
        // Validate namespace for security
        SqlitePool::validate_namespace(&namespace)?;
        self.namespace = Some(namespace);
        Ok(self)
    }

    /// Get the actual key with namespace prefix if set
    pub(crate) fn namespaced_key(&self, key: &MemoryKey) -> String {
        match &self.namespace {
            Some(ns) => format!("{}:{}", ns, key.as_str()),
            None => key.as_str().to_string(),
        }
    }
}

// Need Clone for backup method
impl Clone for SqliteMemory {
    fn clone(&self) -> Self {
        Self {
            pool: Arc::clone(&self.pool),
            migration_engine: Arc::clone(&self.migration_engine),
            namespace: self.namespace.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::admin::BackupFormat;
    use crate::admin::MemoryAdmin;
    use skreaver_core::memory::{MemoryReader, MemoryUpdate, MemoryWriter};
    use tempfile::tempdir;

    #[test]
    fn test_sqlite_memory_basic_operations() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let mut memory = SqliteMemory::new(&db_path).unwrap();

        // Test store and load
        let key = MemoryKey::new("test_key").unwrap();
        let update = MemoryUpdate::new("test_key", "test_value").unwrap();

        memory.store(update).unwrap();
        let value = memory.load(&key).unwrap();
        assert_eq!(value, Some("test_value".to_string()));
    }

    #[test]
    fn test_sqlite_memory_wal_mode() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_wal.db");

        let memory = SqliteMemory::new(&db_path).unwrap();
        let conn = memory.pool.acquire().unwrap();

        // Verify WAL mode is enabled
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
    }

    #[test]
    fn test_sqlite_memory_migrations() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_migrations.db");

        let memory = SqliteMemory::new(&db_path).unwrap();
        let conn = memory.pool.acquire().unwrap();

        // Check that migrations were applied
        let version: u32 = conn
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(version, 1); // We have 1 migration defined

        // Check that table exists
        let table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='memory'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(table_count, 1);
    }

    #[test]
    fn test_sqlite_memory_admin_operations() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_admin.db");

        let mut memory = SqliteMemory::new(&db_path).unwrap();

        // Store some data
        memory
            .store(MemoryUpdate::new("key1", "value1").unwrap())
            .unwrap();

        // Test backup
        let backup = memory.backup().unwrap();
        assert_eq!(backup.format, BackupFormat::Json);
        assert!(backup.size_bytes > 0);

        // Test health status
        let health = memory.health_status().unwrap();
        assert!(health.is_healthy());
        assert!(health.message.contains("connections healthy"));

        // Test migration status
        let migration_status = memory.migration_status().unwrap();
        assert_eq!(migration_status.current_version, 1);
        assert_eq!(migration_status.latest_version, 1);
        assert!(migration_status.pending_migrations.is_empty());
    }

    #[test]
    fn test_sqlite_memory_thread_safety() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_threads.db");

        let memory = Arc::new(std::sync::Mutex::new(SqliteMemory::new(&db_path).unwrap()));

        let handles: Vec<_> = (0..10)
            .map(|i| {
                let memory = Arc::clone(&memory);
                std::thread::spawn(move || {
                    for j in 0..10 {
                        let key = format!("key_{}_{}", i, j);
                        let value = format!("value_{}_{}", i, j);
                        let update = MemoryUpdate::new(&key, &value).unwrap();

                        memory.lock().unwrap().store(update).unwrap();

                        let loaded = memory
                            .lock()
                            .unwrap()
                            .load(&MemoryKey::new(&key).unwrap())
                            .unwrap();
                        assert_eq!(loaded, Some(value));
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }
    }
}
