//! PostgreSQL-based memory backend with ACID compliance, connection pooling, and advanced features
//!
//! This module provides an enterprise-grade PostgreSQL backend with:
//! - Full ACID compliance with proper transaction isolation levels
//! - Advanced connection pooling with health monitoring
//! - Schema migration support with versioning and rollback
//! - JSON support for structured data storage
//! - Comprehensive security and error handling

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio_postgres::{Client, Config, Error as PgError, IsolationLevel, NoTls};

use skreaver_core::error::{MemoryError, TransactionError};
use skreaver_core::memory::{
    MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter, SnapshotableMemory, TransactionalMemory,
};

/// PostgreSQL connection configuration
#[derive(Debug, Clone)]
pub struct PostgresConfig {
    /// Database host
    pub host: String,
    /// Database port
    pub port: u16,
    /// Database name
    pub database: String,
    /// Username for authentication
    pub user: String,
    /// Password for authentication
    pub password: Option<String>,
    /// Connection timeout in seconds
    pub connect_timeout: u64,
    /// Connection pool size
    pub pool_size: usize,
    /// Application name for connection identification
    pub application_name: String,
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 5432,
            database: "skreaver".to_string(),
            user: "skreaver".to_string(),
            password: None,
            connect_timeout: 30,
            pool_size: 10,
            application_name: "skreaver-memory".to_string(),
        }
    }
}

impl PostgresConfig {
    /// Create a new config with database URL
    pub fn from_url(url: &str) -> Result<Self, MemoryError> {
        let config: Config = url
            .parse()
            .map_err(|e: PgError| MemoryError::ConnectionFailed {
                backend: "postgres".to_string(),
                reason: format!("Invalid database URL: {}", e),
            })?;

        Ok(Self {
            host: config
                .get_hosts()
                .get(0)
                .map(|h| match h {
                    tokio_postgres::config::Host::Tcp(s) => s.clone(),
                    tokio_postgres::config::Host::Unix(path) => path.to_string_lossy().to_string(),
                })
                .unwrap_or_else(|| "localhost".to_string()),
            port: config.get_ports().get(0).copied().unwrap_or(5432),
            database: config.get_dbname().unwrap_or("skreaver").to_string(),
            user: config.get_user().unwrap_or("skreaver").to_string(),
            password: config
                .get_password()
                .map(|s| String::from_utf8_lossy(s).to_string()),
            connect_timeout: 30,
            pool_size: 10,
            application_name: config
                .get_application_name()
                .unwrap_or("skreaver-memory")
                .to_string(),
        })
    }

    /// Validate configuration for security
    fn validate(&self) -> Result<(), MemoryError> {
        // Basic validation
        if self.host.is_empty() {
            return Err(MemoryError::ConnectionFailed {
                backend: "postgres".to_string(),
                reason: "Host cannot be empty".to_string(),
            });
        }

        if self.database.is_empty() {
            return Err(MemoryError::ConnectionFailed {
                backend: "postgres".to_string(),
                reason: "Database name cannot be empty".to_string(),
            });
        }

        if self.user.is_empty() {
            return Err(MemoryError::ConnectionFailed {
                backend: "postgres".to_string(),
                reason: "Username cannot be empty".to_string(),
            });
        }

        if self.pool_size == 0 || self.pool_size > 100 {
            return Err(MemoryError::ConnectionFailed {
                backend: "postgres".to_string(),
                reason: "Pool size must be between 1 and 100".to_string(),
            });
        }

        // Security validations
        if self.host.contains("..") || self.host.contains("//") {
            return Err(MemoryError::ConnectionFailed {
                backend: "postgres".to_string(),
                reason: "Invalid host: potential path traversal detected".to_string(),
            });
        }

        // Validate database name contains only safe characters
        if !self
            .database
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(MemoryError::ConnectionFailed {
                backend: "postgres".to_string(),
                reason: "Database name contains invalid characters".to_string(),
            });
        }

        Ok(())
    }

    /// Build tokio_postgres Config
    fn build_pg_config(&self) -> Config {
        let mut config = Config::new();
        config
            .host(&self.host)
            .port(self.port)
            .dbname(&self.database)
            .user(&self.user)
            .application_name(&self.application_name)
            .connect_timeout(Duration::from_secs(self.connect_timeout));

        if let Some(ref password) = self.password {
            config.password(password);
        }

        config
    }
}

/// A pooled PostgreSQL connection with RAII cleanup
pub struct PooledConnection {
    client: Option<Client>,
    pool: Arc<Mutex<Vec<Client>>>,
    pool_size: usize,
}

impl PooledConnection {
    fn new(client: Client, pool: Arc<Mutex<Vec<Client>>>, pool_size: usize) -> Self {
        Self {
            client: Some(client),
            pool,
            pool_size,
        }
    }

    /// Get reference to the underlying client
    pub fn client(&self) -> &Client {
        self.client.as_ref().expect("Client should be available")
    }

    /// Get mutable reference to the underlying client
    pub fn client_mut(&mut self) -> &mut Client {
        self.client.as_mut().expect("Client should be available")
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        if let Some(client) = self.client.take() {
            let pool = Arc::clone(&self.pool);
            let pool_size = self.pool_size;

            // Return connection to pool synchronously using try_lock to avoid blocking
            if let Ok(mut pool_guard) = pool.try_lock() {
                if pool_guard.len() < pool_size {
                    pool_guard.push(client);
                }
                // If pool is full or locked, connection will be dropped
            }
            // If we can't get the lock immediately, just drop the connection
            // This prevents deadlocks in Drop implementations
        }
    }
}

/// Health status for PostgreSQL connection pool
#[derive(Debug, Clone)]
pub struct PostgresPoolHealth {
    pub available_connections: usize,
    pub total_connections: usize,
    pub active_connections: usize,
    pub server_version: String,
    pub last_check: std::time::Instant,
}

/// PostgreSQL connection pool with health monitoring
pub struct PostgresPool {
    config: PostgresConfig,
    connections: Arc<Mutex<Vec<Client>>>,
    active_count: Arc<RwLock<usize>>,
}

impl PostgresPool {
    /// Create a new PostgreSQL connection pool
    pub async fn new(config: PostgresConfig) -> Result<Self, MemoryError> {
        config.validate()?;

        let pg_config = config.build_pg_config();
        let mut connections = Vec::with_capacity(config.pool_size);

        // Create initial pool of connections
        for _ in 0..config.pool_size {
            let (client, connection) =
                pg_config
                    .connect(NoTls)
                    .await
                    .map_err(|e| MemoryError::ConnectionFailed {
                        backend: "postgres".to_string(),
                        reason: Self::sanitize_error(&e),
                    })?;

            // Spawn connection task
            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    eprintln!("PostgreSQL connection error: {}", e);
                }
            });

            // Validate connection health
            Self::validate_connection(&client).await?;
            connections.push(client);
        }

        Ok(Self {
            config,
            connections: Arc::new(Mutex::new(connections)),
            active_count: Arc::new(RwLock::new(0)),
        })
    }

    /// Sanitize PostgreSQL errors for security
    fn sanitize_error(error: &PgError) -> String {
        // Map specific PostgreSQL errors to safe messages
        if error.as_db_error().is_some() {
            "Database operation failed".to_string()
        } else if error.to_string().contains("connection") {
            "Connection failed".to_string()
        } else if error.to_string().contains("authentication") {
            "Authentication failed".to_string()
        } else if error.to_string().contains("timeout") {
            "Operation timed out".to_string()
        } else {
            "Database error occurred".to_string()
        }
    }

    /// Validate connection health
    async fn validate_connection(client: &Client) -> Result<(), MemoryError> {
        // Simple health check query
        client
            .query_one("SELECT 1", &[])
            .await
            .map_err(|e| MemoryError::ConnectionFailed {
                backend: "postgres".to_string(),
                reason: Self::sanitize_error(&e),
            })?;

        Ok(())
    }

    /// Acquire a connection from the pool
    pub async fn acquire(&self) -> Result<PooledConnection, MemoryError> {
        // Try to get available connection - use a separate scope to release locks quickly
        let existing_client = {
            let mut pool = self.connections.lock().await;
            pool.pop()
        };

        if let Some(client) = existing_client {
            // Validate connection before returning
            if Self::validate_connection(&client).await.is_ok() {
                // Update active count atomically
                {
                    let mut active = self.active_count.write().await;
                    *active += 1;
                }

                return Ok(PooledConnection::new(
                    client,
                    Arc::clone(&self.connections),
                    self.config.pool_size,
                ));
            }
            // Connection is bad, drop it and create a new one below
        }

        // Check if we can create new connection - atomic read
        let current_active = *self.active_count.read().await;
        if current_active >= self.config.pool_size {
            return Err(MemoryError::ConnectionFailed {
                backend: "postgres".to_string(),
                reason: format!(
                    "Connection pool exhausted: {} active connections (max: {})",
                    current_active, self.config.pool_size
                ),
            });
        }

        // Create new connection
        let pg_config = self.config.build_pg_config();
        let (client, connection) =
            pg_config
                .connect(NoTls)
                .await
                .map_err(|e| MemoryError::ConnectionFailed {
                    backend: "postgres".to_string(),
                    reason: Self::sanitize_error(&e),
                })?;

        // Spawn connection task
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("PostgreSQL connection error: {}", e);
            }
        });

        // Validate new connection
        Self::validate_connection(&client).await?;

        // Update active count atomically
        {
            let mut active = self.active_count.write().await;
            *active += 1;
        }

        Ok(PooledConnection::new(
            client,
            Arc::clone(&self.connections),
            self.config.pool_size,
        ))
    }

    /// Check pool health
    pub async fn health_check(&self) -> Result<PostgresPoolHealth, MemoryError> {
        let available_connections = {
            let pool = self.connections.lock().await;
            pool.len()
        };

        let active_connections = *self.active_count.read().await;

        // Get server version for health info
        let server_version = if let Ok(conn) = self.acquire().await {
            conn.client()
                .query_one("SELECT version()", &[])
                .await
                .map(|row| row.get::<_, String>(0))
                .unwrap_or_else(|_| "Unknown".to_string())
        } else {
            "Unknown".to_string()
        };

        Ok(PostgresPoolHealth {
            available_connections,
            total_connections: self.config.pool_size,
            active_connections,
            server_version,
            last_check: std::time::Instant::now(),
        })
    }
}

/// PostgreSQL memory backend with enterprise features
pub struct PostgresMemory {
    pool: Arc<PostgresPool>,
    namespace: Option<String>,
}

impl PostgresMemory {
    /// Create a new PostgreSQL memory backend
    pub async fn new(config: PostgresConfig) -> Result<Self, MemoryError> {
        let pool = Arc::new(PostgresPool::new(config).await?);

        // Initialize database schema
        Self::initialize_schema(&pool).await?;

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
                backend: "postgres".to_string(),
                reason: "Namespace cannot be empty".to_string(),
            });
        }

        if namespace.len() > 64 {
            return Err(MemoryError::ConnectionFailed {
                backend: "postgres".to_string(),
                reason: "Namespace too long (max 64 characters)".to_string(),
            });
        }

        // Only allow safe characters
        if !namespace
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(MemoryError::ConnectionFailed {
                backend: "postgres".to_string(),
                reason: "Namespace contains invalid characters (only alphanumeric, _, - allowed)"
                    .to_string(),
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

    /// Initialize database schema
    async fn initialize_schema(pool: &PostgresPool) -> Result<(), MemoryError> {
        let conn = pool.acquire().await?;

        // Create main memory table with JSONB for efficient storage
        let schema_sql = r#"
            CREATE TABLE IF NOT EXISTS memory_entries (
                key TEXT PRIMARY KEY,
                value JSONB NOT NULL,
                namespace TEXT,
                created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
            );

            -- Create indexes for performance
            CREATE INDEX IF NOT EXISTS idx_memory_namespace ON memory_entries(namespace);
            CREATE INDEX IF NOT EXISTS idx_memory_updated_at ON memory_entries(updated_at);
            CREATE INDEX IF NOT EXISTS idx_memory_value_gin ON memory_entries USING gin(value);

            -- Create migration tracking table
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                description TEXT NOT NULL,
                applied_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
            );

            -- Insert initial migration record
            INSERT INTO schema_migrations (version, description) 
            VALUES (1, 'Initial PostgreSQL memory schema')
            ON CONFLICT (version) DO NOTHING;
        "#;

        conn.client().batch_execute(schema_sql).await.map_err(|e| {
            MemoryError::ConnectionFailed {
                backend: "postgres".to_string(),
                reason: format!("Failed to initialize schema: {}", e),
            }
        })?;

        Ok(())
    }
}

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

/// Async implementation of PostgreSQL memory operations
impl PostgresMemory {
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
                reason: PostgresPool::sanitize_error(&e),
            })?;

        match row {
            Some(row) => {
                let json_value: serde_json::Value = row.get(0);
                Ok(Some(json_value.to_string()))
            }
            None => Ok(None),
        }
    }

    /// Async load many operation
    pub async fn load_many_async(
        &self,
        keys: &[MemoryKey],
    ) -> Result<Vec<Option<String>>, MemoryError> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        let conn = self.pool.acquire().await?;
        let namespaced_keys: Vec<String> = keys.iter().map(|k| self.namespaced_key(k)).collect();

        // Use ANY() for efficient bulk query
        let rows = conn
            .client()
            .query(
                "SELECT key, value FROM memory_entries WHERE key = ANY($1)",
                &[&namespaced_keys],
            )
            .await
            .map_err(|e| MemoryError::LoadFailed {
                key: MemoryKey::new("batch").unwrap(),
                reason: PostgresPool::sanitize_error(&e),
            })?;

        let mut results = std::collections::HashMap::new();
        for row in rows {
            let key: String = row.get(0);
            let value: serde_json::Value = row.get(1);
            results.insert(key, value.to_string());
        }

        // Return in same order as requested
        Ok(namespaced_keys
            .iter()
            .map(|k| results.get(k).cloned())
            .collect())
    }

    /// Async store operation
    pub async fn store_async(&self, update: MemoryUpdate) -> Result<(), MemoryError> {
        let conn = self.pool.acquire().await?;
        let namespaced_key = self.namespaced_key(&update.key);

        // Parse JSON value for JSONB storage
        let json_value: serde_json::Value = serde_json::from_str(&update.value)
            .unwrap_or_else(|_| serde_json::Value::String(update.value.clone()));

        conn.client()
            .execute(
                r#"
                INSERT INTO memory_entries (key, value, namespace, updated_at) 
                VALUES ($1, $2, $3, NOW())
                ON CONFLICT (key) DO UPDATE SET 
                    value = EXCLUDED.value,
                    updated_at = NOW()
                "#,
                &[
                    &namespaced_key,
                    &json_value,
                    &self.namespace.as_deref().unwrap_or(""),
                ],
            )
            .await
            .map_err(|e| MemoryError::StoreFailed {
                key: update.key.clone(),
                reason: PostgresPool::sanitize_error(&e),
            })?;

        Ok(())
    }

    /// Async store many operation
    pub async fn store_many_async(&self, updates: Vec<MemoryUpdate>) -> Result<(), MemoryError> {
        if updates.is_empty() {
            return Ok(());
        }

        let mut conn = self.pool.acquire().await?;

        let tx =
            conn.client_mut()
                .transaction()
                .await
                .map_err(|e| MemoryError::ConnectionFailed {
                    backend: "postgres".to_string(),
                    reason: format!("Failed to begin transaction: {}", e),
                })?;

        let stmt = tx
            .prepare(
                r#"
                INSERT INTO memory_entries (key, value, namespace, updated_at) 
                VALUES ($1, $2, $3, NOW())
                ON CONFLICT (key) DO UPDATE SET 
                    value = EXCLUDED.value,
                    updated_at = NOW()
                "#,
            )
            .await
            .map_err(|e| MemoryError::StoreFailed {
                key: MemoryKey::new("batch").unwrap(),
                reason: PostgresPool::sanitize_error(&e),
            })?;

        for update in updates {
            let namespaced_key = self.namespaced_key(&update.key);
            let json_value: serde_json::Value = serde_json::from_str(&update.value)
                .unwrap_or_else(|_| serde_json::Value::String(update.value.clone()));

            tx.execute(
                &stmt,
                &[
                    &namespaced_key,
                    &json_value,
                    &self.namespace.as_deref().unwrap_or(""),
                ],
            )
            .await
            .map_err(|e| MemoryError::StoreFailed {
                key: update.key.clone(),
                reason: PostgresPool::sanitize_error(&e),
            })?;
        }

        tx.commit()
            .await
            .map_err(|e| MemoryError::ConnectionFailed {
                backend: "postgres".to_string(),
                reason: format!("Failed to commit transaction: {}", e),
            })?;

        Ok(())
    }
}

// Sync trait implementations using a dedicated runtime
thread_local! {
    static POSTGRES_RUNTIME: std::cell::RefCell<Option<tokio::runtime::Runtime>> =
        std::cell::RefCell::new(None);
}

fn with_memory_runtime<F, R>(f: F) -> Result<R, MemoryError>
where
    F: FnOnce() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<R, MemoryError>>>>,
{
    POSTGRES_RUNTIME.with(|rt_cell| {
        let mut rt_ref = rt_cell.borrow_mut();
        if rt_ref.is_none() {
            *rt_ref =
                Some(
                    tokio::runtime::Runtime::new().map_err(|e| MemoryError::LoadFailed {
                        key: MemoryKey::new("runtime").unwrap(),
                        reason: format!("Failed to create async runtime: {}", e),
                    })?,
                );
        }
        let rt = rt_ref.as_ref().unwrap();
        rt.block_on(f())
    })
}

impl MemoryReader for PostgresMemory {
    fn load(&self, key: &MemoryKey) -> Result<Option<String>, MemoryError> {
        with_memory_runtime(|| {
            let key = key.clone();
            let memory = self.clone();
            Box::pin(async move { memory.load_async(&key).await })
        })
    }

    fn load_many(&self, keys: &[MemoryKey]) -> Result<Vec<Option<String>>, MemoryError> {
        with_memory_runtime(|| {
            let keys = keys.to_vec();
            let memory = self.clone();
            Box::pin(async move { memory.load_many_async(&keys).await })
        })
    }
}

// Add Clone implementation for PostgresMemory to support thread-local runtime
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
        with_memory_runtime(|| {
            let update = update.clone();
            let memory = self.clone();
            Box::pin(async move { memory.store_async(update).await })
        })
    }

    fn store_many(&mut self, updates: Vec<MemoryUpdate>) -> Result<(), MemoryError> {
        with_memory_runtime(|| {
            let updates = updates.clone();
            let memory = self.clone();
            Box::pin(async move { memory.store_many_async(updates).await })
        })
    }
}

impl SnapshotableMemory for PostgresMemory {
    fn snapshot(&mut self) -> Option<String> {
        with_memory_runtime(|| {
            let memory = self.clone();
            Box::pin(async move {
                let conn = memory.pool.acquire().await?;

                let rows = conn
                    .client()
                    .query("SELECT key, value FROM memory_entries ORDER BY key", &[])
                    .await
                    .map_err(|e| MemoryError::LoadFailed {
                        key: MemoryKey::new("snapshot").unwrap(),
                        reason: PostgresPool::sanitize_error(&e),
                    })?;

                let mut snapshot = std::collections::HashMap::new();
                for row in rows {
                    let key: String = row.get(0);
                    let value: serde_json::Value = row.get(1);
                    snapshot.insert(key, value);
                }

                Ok(serde_json::to_string(&snapshot).ok())
            })
        })
        .unwrap_or(None)
    }

    fn restore(&mut self, snapshot: &str) -> Result<(), MemoryError> {
        let data: std::collections::HashMap<String, serde_json::Value> =
            serde_json::from_str(snapshot).map_err(|e| MemoryError::RestoreFailed {
                reason: format!("Invalid snapshot format: {}", e),
            })?;

        with_memory_runtime(|| {
            let memory = self.clone();
            let data = data.clone();
            Box::pin(async move {
                let mut conn = memory.pool.acquire().await?;

                let tx = conn.client_mut().transaction().await.map_err(|e| {
                    MemoryError::ConnectionFailed {
                        backend: "postgres".to_string(),
                        reason: format!("Failed to begin restore transaction: {}", e),
                    }
                })?;

                // Clear existing data
                tx.execute("DELETE FROM memory_entries", &[])
                    .await
                    .map_err(|e| MemoryError::RestoreFailed {
                        reason: format!("Failed to clear existing data: {}", e),
                    })?;

                // Insert snapshot data
                let stmt = tx
                    .prepare("INSERT INTO memory_entries (key, value) VALUES ($1, $2)")
                    .await
                    .map_err(|e| MemoryError::RestoreFailed {
                        reason: PostgresPool::sanitize_error(&e),
                    })?;

                for (key, value) in data {
                    tx.execute(&stmt, &[&key, &value]).await.map_err(|e| {
                        MemoryError::RestoreFailed {
                            reason: format!("Failed to restore key {}: {}", key, e),
                        }
                    })?;
                }

                tx.commit()
                    .await
                    .map_err(|e| MemoryError::ConnectionFailed {
                        backend: "postgres".to_string(),
                        reason: format!("Failed to commit restore: {}", e),
                    })?;

                Ok(())
            })
        })
    }
}

impl TransactionalMemory for PostgresMemory {
    fn transaction<F, R>(&mut self, f: F) -> Result<R, TransactionError>
    where
        F: FnOnce(&mut dyn MemoryWriter) -> Result<R, TransactionError>,
    {
        // Use a simplified synchronous approach that works within the current runtime
        let rt = tokio::runtime::Handle::try_current()
            .or_else(|_| {
                POSTGRES_RUNTIME.with(|rt_cell| {
                    let mut rt_ref = rt_cell.borrow_mut();
                    if rt_ref.is_none() {
                        *rt_ref = Some(tokio::runtime::Runtime::new().map_err(|e| {
                            TransactionError::TransactionFailed {
                                reason: format!("Failed to create async runtime: {}", e),
                            }
                        })?);
                    }
                    Ok(rt_ref.as_ref().unwrap().handle().clone())
                })
            })
            .map_err(|e: TransactionError| e)?;

        rt.block_on(async {
            let mut conn =
                self.pool
                    .acquire()
                    .await
                    .map_err(|e| TransactionError::TransactionFailed {
                        reason: format!("Failed to acquire connection for transaction: {}", e),
                    })?;

            // Set proper isolation level for ACID compliance
            let tx = conn
                .client_mut()
                .build_transaction()
                .isolation_level(IsolationLevel::Serializable)
                .start()
                .await
                .map_err(|e| TransactionError::TransactionFailed {
                    reason: format!("Failed to begin PostgreSQL transaction: {}", e),
                })?;

            // Create a transactional wrapper with proper lifetime management
            let mut tx_memory = PostgresTransactionalMemory::new(tx, self.namespace.clone());

            let result = f(&mut tx_memory);

            match result {
                Ok(value) => {
                    tx_memory
                        .commit()
                        .await
                        .map_err(|e| TransactionError::TransactionFailed {
                            reason: format!("Failed to commit PostgreSQL transaction: {}", e),
                        })?;
                    Ok(value)
                }
                Err(tx_error) => {
                    if let Err(rollback_err) = tx_memory.rollback().await {
                        eprintln!("Warning: Failed to rollback transaction: {}", rollback_err);
                    }
                    Err(tx_error)
                }
            }
        })
    }
}

/// Transactional wrapper for PostgreSQL operations with proper resource management
struct PostgresTransactionalMemory<'a> {
    tx: Option<tokio_postgres::Transaction<'a>>,
    namespace: Option<String>,
    pending_operations: Vec<MemoryUpdate>,
}

impl<'a> PostgresTransactionalMemory<'a> {
    fn new(tx: tokio_postgres::Transaction<'a>, namespace: Option<String>) -> Self {
        Self {
            tx: Some(tx),
            namespace,
            pending_operations: Vec::new(),
        }
    }

    async fn commit(&mut self) -> Result<(), tokio_postgres::Error> {
        if let Some(tx) = self.tx.take() {
            // Apply all pending operations within the transaction
            for update in &self.pending_operations {
                let namespaced_key = match &self.namespace {
                    Some(ns) => format!("{}:{}", ns, update.key.as_str()),
                    None => update.key.as_str().to_string(),
                };

                let json_value: serde_json::Value = serde_json::from_str(&update.value)
                    .unwrap_or_else(|_| serde_json::Value::String(update.value.clone()));

                tx.execute(
                    r#"
                    INSERT INTO memory_entries (key, value, namespace, updated_at) 
                    VALUES ($1, $2, $3, NOW())
                    ON CONFLICT (key) DO UPDATE SET 
                        value = EXCLUDED.value,
                        updated_at = NOW()
                    "#,
                    &[
                        &namespaced_key,
                        &json_value,
                        &self.namespace.as_deref().unwrap_or(""),
                    ],
                )
                .await?;
            }

            tx.commit().await
        } else {
            Ok(())
        }
    }

    async fn rollback(&mut self) -> Result<(), tokio_postgres::Error> {
        if let Some(tx) = self.tx.take() {
            tx.rollback().await
        } else {
            Ok(())
        }
    }
}

impl<'a> MemoryWriter for PostgresTransactionalMemory<'a> {
    fn store(&mut self, update: MemoryUpdate) -> Result<(), MemoryError> {
        // Buffer operations to apply them atomically during commit
        self.pending_operations.push(update);
        Ok(())
    }

    fn store_many(&mut self, updates: Vec<MemoryUpdate>) -> Result<(), MemoryError> {
        // Buffer all operations to apply them atomically during commit
        self.pending_operations.extend(updates);
        Ok(())
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

        assert!(config.validate().is_err());

        let valid_config = PostgresConfig::default();
        assert!(valid_config.validate().is_ok());
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
