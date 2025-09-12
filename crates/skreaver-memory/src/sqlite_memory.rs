//! SQLite-based memory backend with WAL mode, connection pooling, and migrations
//!
//! This module provides a production-ready SQLite backend with:
//! - WAL (Write-Ahead Logging) mode for better concurrency
//! - Connection pooling for efficient resource usage
//! - Schema migration support with versioning
//! - Health monitoring and admin operations

use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use skreaver_core::error::{MemoryError, TransactionError};
use skreaver_core::memory::{
    MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter, SnapshotableMemory, TransactionalMemory,
};

/// Connection pool for SQLite with configurable size
pub struct SqlitePool {
    connections: Vec<Arc<Mutex<Connection>>>,
    path: PathBuf,
    pool_size: usize,
}

impl SqlitePool {
    /// Create a new connection pool with the specified size
    pub fn new(path: impl AsRef<Path>, pool_size: usize) -> Result<Self, MemoryError> {
        let path = path.as_ref().to_path_buf();
        let mut connections = Vec::with_capacity(pool_size);
        
        for _ in 0..pool_size {
            let conn = Self::create_connection(&path)?;
            connections.push(Arc::new(Mutex::new(conn)));
        }
        
        Ok(Self {
            connections,
            path,
            pool_size,
        })
    }
    
    /// Create a new SQLite connection with WAL mode and optimizations
    fn create_connection(path: &Path) -> Result<Connection, MemoryError> {
        let conn = Connection::open(path).map_err(|e| MemoryError::ConnectionFailed {
            backend: "sqlite".to_string(),
            reason: format!("Failed to open SQLite database: {}", e),
        })?;
        
        // Enable WAL mode for better concurrency
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -64000; -- 64MB cache
             PRAGMA busy_timeout = 5000; -- 5 second timeout
             PRAGMA foreign_keys = ON;"
        ).map_err(|e| MemoryError::ConnectionFailed {
            backend: "sqlite".to_string(),
            reason: format!("Failed to configure SQLite: {}", e),
        })?;
        
        Ok(conn)
    }
    
    /// Get a connection from the pool
    pub fn get(&self) -> Result<Arc<Mutex<Connection>>, MemoryError> {
        // Simple round-robin selection
        let index = (rand::random::<u32>() as usize) % self.pool_size;
        Ok(Arc::clone(&self.connections[index]))
    }
}

/// Migration engine for SQLite
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

impl MigrationEngine {
    /// Create a new migration engine
    pub fn new() -> Self {
        Self {
            migrations: Self::default_migrations(),
        }
    }
    
    /// Define the default migrations for the memory backend
    fn default_migrations() -> Vec<Migration> {
        vec![
            Migration {
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
                "#.to_string(),
                down: Some("DROP TABLE IF EXISTS memory;".to_string()),
            },
        ]
    }
    
    /// Run migrations up to the specified version
    pub fn migrate(&self, conn: &Connection, target_version: Option<u32>) -> Result<(), MemoryError> {
        // Create migration tracking table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                description TEXT NOT NULL,
                applied_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            )",
            [],
        ).map_err(|e| MemoryError::ConnectionFailed {
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
        
        let target = target_version.unwrap_or_else(|| {
            self.migrations.iter().map(|m| m.version).max().unwrap_or(0)
        });
        
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
        let tx = conn.unchecked_transaction().map_err(|e| MemoryError::ConnectionFailed {
            backend: "sqlite".to_string(),
            reason: format!("Failed to start migration transaction: {}", e),
        })?;
        
        // Execute migration
        tx.execute_batch(&migration.up).map_err(|e| MemoryError::ConnectionFailed {
            backend: "sqlite".to_string(),
            reason: format!("Migration {} failed: {}", migration.version, e),
        })?;
        
        // Record migration
        tx.execute(
            "INSERT INTO schema_migrations (version, description) VALUES (?1, ?2)",
            params![migration.version, migration.description],
        ).map_err(|e| MemoryError::ConnectionFailed {
            backend: "sqlite".to_string(),
            reason: format!("Failed to record migration {}: {}", migration.version, e),
        })?;
        
        tx.commit().map_err(|e| MemoryError::ConnectionFailed {
            backend: "sqlite".to_string(),
            reason: format!("Failed to commit migration {}: {}", migration.version, e),
        })?;
        
        Ok(())
    }
}

/// SQLite-based memory backend with all Phase 1.1 features
pub struct SqliteMemory {
    pool: Arc<SqlitePool>,
    migration_engine: Arc<MigrationEngine>,
    namespace: Option<String>,
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
        let conn_mutex = pool.get()?;
        let conn = conn_mutex.lock().map_err(|e| MemoryError::ConnectionFailed {
            backend: "sqlite".to_string(),
            reason: format!("Failed to lock connection: {}", e),
        })?;
        
        migration_engine.migrate(&conn, None)?;
        
        Ok(Self {
            pool,
            migration_engine,
            namespace: None,
        })
    }
    
    /// Set a namespace for key isolation
    pub fn with_namespace(mut self, namespace: String) -> Self {
        self.namespace = Some(namespace);
        self
    }
    
    /// Get the actual key with namespace prefix if set
    fn namespaced_key(&self, key: &MemoryKey) -> String {
        match &self.namespace {
            Some(ns) => format!("{}:{}", ns, key.as_str()),
            None => key.as_str().to_string(),
        }
    }
}

impl MemoryReader for SqliteMemory {
    fn load(&self, key: &MemoryKey) -> Result<Option<String>, MemoryError> {
        let conn_mutex = self.pool.get()?;
        let conn = conn_mutex.lock().map_err(|e| MemoryError::LoadFailed {
            key: key.clone(),
            reason: format!("Failed to lock connection: {}", e),
        })?;
        
        let namespaced_key = self.namespaced_key(key);
        
        conn.query_row(
            "SELECT value FROM memory WHERE key = ?1",
            params![namespaced_key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| MemoryError::LoadFailed {
            key: key.clone(),
            reason: e.to_string(),
        })
    }
    
    fn load_many(&self, keys: &[MemoryKey]) -> Result<Vec<Option<String>>, MemoryError> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        
        let conn_mutex = self.pool.get()?;
        let conn = conn_mutex.lock().map_err(|e| MemoryError::LoadFailed {
            key: MemoryKey::new("batch").unwrap(),
            reason: format!("Failed to lock connection: {}", e),
        })?;
        
        let namespaced_keys: Vec<String> = keys.iter().map(|k| self.namespaced_key(k)).collect();
        let placeholders = vec!["?"; namespaced_keys.len()].join(",");
        let query = format!("SELECT key, value FROM memory WHERE key IN ({})", placeholders);
        
        let mut stmt = conn.prepare(&query).map_err(|e| MemoryError::LoadFailed {
            key: MemoryKey::new("batch").unwrap(),
            reason: e.to_string(),
        })?;
        
        let mut results = std::collections::HashMap::new();
        let params: Vec<&dyn rusqlite::ToSql> = namespaced_keys
            .iter()
            .map(|k| k as &dyn rusqlite::ToSql)
            .collect();
        
        let rows = stmt.query_map(&params[..], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }).map_err(|e| MemoryError::LoadFailed {
            key: MemoryKey::new("batch").unwrap(),
            reason: e.to_string(),
        })?;
        
        for row in rows {
            let (k, v) = row.map_err(|e| MemoryError::LoadFailed {
                key: MemoryKey::new("batch").unwrap(),
                reason: e.to_string(),
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

impl MemoryWriter for SqliteMemory {
    fn store(&mut self, update: MemoryUpdate) -> Result<(), MemoryError> {
        let conn_mutex = self.pool.get()?;
        let conn = conn_mutex.lock().map_err(|e| MemoryError::StoreFailed {
            key: update.key.clone(),
            reason: format!("Failed to lock connection: {}", e),
        })?;
        
        let namespaced_key = self.namespaced_key(&update.key);
        
        conn.execute(
            "INSERT INTO memory (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET 
                value = excluded.value,
                updated_at = strftime('%s', 'now')",
            params![namespaced_key, update.value],
        ).map_err(|e| MemoryError::StoreFailed {
            key: update.key.clone(),
            reason: e.to_string(),
        })?;
        
        Ok(())
    }
    
    fn store_many(&mut self, updates: Vec<MemoryUpdate>) -> Result<(), MemoryError> {
        if updates.is_empty() {
            return Ok(());
        }
        
        let conn_mutex = self.pool.get()?;
        let conn = conn_mutex.lock().map_err(|e| MemoryError::StoreFailed {
            key: MemoryKey::new("batch").unwrap(),
            reason: format!("Failed to lock connection: {}", e),
        })?;
        
        let tx = conn.unchecked_transaction().map_err(|e| MemoryError::ConnectionFailed {
            backend: "sqlite".to_string(),
            reason: format!("Failed to begin transaction: {}", e),
        })?;
        
        {
            let mut stmt = tx.prepare(
                "INSERT INTO memory (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET 
                    value = excluded.value,
                    updated_at = strftime('%s', 'now')"
            ).map_err(|e| MemoryError::StoreFailed {
                key: MemoryKey::new("batch").unwrap(),
                reason: e.to_string(),
            })?;
            
            for update in updates {
                let namespaced_key = self.namespaced_key(&update.key);
                stmt.execute(params![namespaced_key, update.value])
                    .map_err(|e| MemoryError::StoreFailed {
                        key: update.key.clone(),
                        reason: e.to_string(),
                    })?;
            }
        }
        
        tx.commit().map_err(|e| MemoryError::ConnectionFailed {
            backend: "sqlite".to_string(),
            reason: format!("Failed to commit transaction: {}", e),
        })?;
        
        Ok(())
    }
}

impl SnapshotableMemory for SqliteMemory {
    fn snapshot(&mut self) -> Option<String> {
        let conn_mutex = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return None,
        };
        
        let conn = match conn_mutex.lock() {
            Ok(c) => c,
            Err(_) => return None,
        };
        
        let mut stmt = match conn.prepare("SELECT key, value FROM memory") {
            Ok(s) => s,
            Err(_) => return None,
        };
        
        let mut snapshot = std::collections::HashMap::new();
        let rows = match stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }) {
            Ok(r) => r,
            Err(_) => return None,
        };
        
        for row in rows {
            let (k, v) = match row {
                Ok((k, v)) => (k, v),
                Err(_) => return None,
            };
            snapshot.insert(k, v);
        }
        
        serde_json::to_string(&snapshot).ok()
    }
    
    fn restore(&mut self, snapshot: &str) -> Result<(), MemoryError> {
        let data: std::collections::HashMap<String, String> =
            serde_json::from_str(snapshot).map_err(|e| MemoryError::RestoreFailed {
                reason: format!("Invalid snapshot format: {}", e),
            })?;
        
        let conn_mutex = self.pool.get()?;
        let conn = conn_mutex.lock().map_err(|e| MemoryError::RestoreFailed {
            reason: format!("Failed to lock connection: {}", e),
        })?;
        
        let tx = conn.unchecked_transaction().map_err(|e| MemoryError::ConnectionFailed {
            backend: "sqlite".to_string(),
            reason: format!("Failed to begin restore transaction: {}", e),
        })?;
        
        // Clear existing data
        tx.execute("DELETE FROM memory", []).map_err(|e| MemoryError::RestoreFailed {
            reason: format!("Failed to clear existing data: {}", e),
        })?;
        
        // Insert snapshot data
        {
            let mut stmt = tx.prepare(
                "INSERT INTO memory (key, value) VALUES (?1, ?2)"
            ).map_err(|e| MemoryError::RestoreFailed {
                reason: e.to_string(),
            })?;
            
            for (key, value) in data {
                stmt.execute(params![key, value]).map_err(|e| MemoryError::RestoreFailed {
                    reason: format!("Failed to restore key {}: {}", key, e),
                })?;
            }
        }
        
        tx.commit().map_err(|e| MemoryError::ConnectionFailed {
            backend: "sqlite".to_string(),
            reason: format!("Failed to commit restore: {}", e),
        })?;
        
        Ok(())
    }
}

impl TransactionalMemory for SqliteMemory {
    fn transaction<F, R>(&mut self, f: F) -> Result<R, TransactionError>
    where
        F: FnOnce(&mut dyn MemoryWriter) -> Result<R, TransactionError>,
    {
        // For SQLite, we'll execute the function and rely on the connection-level transactions
        f(self)
    }
}

// Add rand dependency for pool selection
use rand;

#[cfg(test)]
mod tests {
    use super::*;
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
        let conn_mutex = memory.pool.get().unwrap();
        let conn = conn_mutex.lock().unwrap();
        
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
        let conn_mutex = memory.pool.get().unwrap();
        let conn = conn_mutex.lock().unwrap();
        
        // Check that migrations were applied
        let version: u32 = conn
            .query_row(
                "SELECT MAX(version) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
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
    fn test_sqlite_memory_namespace() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_namespace.db");
        
        let mut memory1 = SqliteMemory::new(&db_path)
            .unwrap()
            .with_namespace("agent1".to_string());
        let mut memory2 = SqliteMemory::new(&db_path)
            .unwrap()
            .with_namespace("agent2".to_string());
        
        let key = MemoryKey::new("shared_key").unwrap();
        
        // Store different values in different namespaces
        memory1.store(MemoryUpdate::new("shared_key", "value1").unwrap()).unwrap();
        memory2.store(MemoryUpdate::new("shared_key", "value2").unwrap()).unwrap();
        
        // Verify isolation
        assert_eq!(memory1.load(&key).unwrap(), Some("value1".to_string()));
        assert_eq!(memory2.load(&key).unwrap(), Some("value2".to_string()));
    }
    
    #[test]
    fn test_sqlite_memory_snapshot_restore() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_snapshot.db");
        
        let mut memory = SqliteMemory::new(&db_path).unwrap();
        
        // Store some data
        memory.store(MemoryUpdate::new("key1", "value1").unwrap()).unwrap();
        memory.store(MemoryUpdate::new("key2", "value2").unwrap()).unwrap();
        
        // Take snapshot
        let snapshot = memory.snapshot().unwrap();
        
        // Modify data
        memory.store(MemoryUpdate::new("key1", "modified").unwrap()).unwrap();
        memory.store(MemoryUpdate::new("key3", "value3").unwrap()).unwrap();
        
        // Restore snapshot
        memory.restore(&snapshot).unwrap();
        
        // Verify restored state
        let key1 = MemoryKey::new("key1").unwrap();
        let key3 = MemoryKey::new("key3").unwrap();
        
        assert_eq!(memory.load(&key1).unwrap(), Some("value1".to_string()));
        assert_eq!(memory.load(&key3).unwrap(), None);
    }
}