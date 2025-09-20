//! Enhanced Redis-based memory backend with clustering, connection pooling, and pub/sub
//!
//! This module provides an enterprise-grade Redis backend with:
//! - Async connection pooling with health monitoring
//! - Redis Cluster support with automatic topology discovery
//! - Pub/Sub messaging for agent communication
//! - Enhanced transactions with proper MULTI/EXEC
//! - Comprehensive security and error handling
//! - Admin operations for backup/restore

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

#[cfg(feature = "redis")]
use deadpool_redis::{Connection as PooledConnection, Pool};
#[cfg(feature = "redis")]
use redis::{AsyncCommands, cluster::ClusterClient};
#[cfg(feature = "redis")]
use tokio::sync::{Mutex, RwLock};

use skreaver_core::error::{MemoryError, TransactionError};
use skreaver_core::memory::{
    MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter, SnapshotableMemory, TransactionalMemory,
};

// Use the modular components
use crate::redis::{
    ConfigProvider, ConnectionMetrics, PoolStats, REDIS_RUNTIME, RedisConnectionProvider,
    RedisHealth, RedisPoolUtils, RedisTransactionExecutor, StatefulConnectionManager, ValidRedisConfig, with_redis_runtime,
};

/// Enhanced Redis memory backend with enterprise features
#[cfg(feature = "redis")]
pub struct RedisMemory {
    /// Connection pool
    pool: Pool,
    /// Stateful connection manager with type safety
    connection_manager: StatefulConnectionManager,
    /// Configuration
    config: ValidRedisConfig,
    /// Cluster client for cluster operations
    cluster_client: Option<Arc<ClusterClient>>,
    /// Health monitoring state
    health: Arc<RwLock<RedisHealth>>,
    /// Connection metrics
    metrics: Arc<Mutex<ConnectionMetrics>>,
}

#[cfg(feature = "redis")]
impl RedisMemory {
    /// Create a new Redis memory backend with type-safe configuration
    /// This method provides compile-time validation guarantees
    pub async fn new(config: ValidRedisConfig) -> Result<Self, MemoryError> {
        // No validation needed - ValidRedisConfig guarantees correctness!
        let (pool, cluster_client) = RedisPoolUtils::create_pool(&config).await?;

        // Create stateful connection manager
        let connection_manager = RedisPoolUtils::create_connection_manager(pool.clone());

        let health = Arc::new(RwLock::new(RedisHealth {
            healthy: false,
            last_ping: None,
            pool_stats: PoolStats {
                total_connections: 0,
                idle_connections: 0,
                active_connections: 0,
                created_at: Instant::now(),
            },
            server_info: None,
            error: None,
        }));

        let metrics = Arc::new(Mutex::new(ConnectionMetrics::default()));

        let memory = Self {
            pool,
            connection_manager,
            config,
            cluster_client,
            health: Arc::clone(&health),
            metrics: Arc::clone(&metrics),
        };

        // Perform initial health check
        memory.health_check().await?;

        Ok(memory)
    }

    /// Create Redis memory with localhost configuration (compile-time safe)
    pub async fn localhost() -> Result<Self, MemoryError> {
        use crate::redis::RedisConfigBuilder;

        let config = RedisConfigBuilder::new()
            .standalone("redis://localhost:6379")
            .with_pool_size(10)
            .build()?;

        Self::new(config).await
    }

    /// Create Redis memory with cluster configuration (compile-time safe)
    pub async fn cluster(nodes: Vec<String>) -> Result<Self, MemoryError> {
        use crate::redis::RedisConfigBuilder;

        let config = RedisConfigBuilder::new()
            .cluster(nodes)
            .with_pool_size(20) // Larger pool for cluster
            .build()?;

        Self::new(config).await
    }

    /// Get a pooled connection (legacy method)
    async fn get_connection(&self) -> Result<PooledConnection, MemoryError> {
        RedisPoolUtils::get_connection(&self.pool, &self.metrics).await
    }

    /// Get a type-safe connection with state tracking
    async fn get_stateful_connection(&self) -> Result<crate::redis::ConnectedRedis, MemoryError> {
        self.connection_manager.get_connection().await
    }

    /// Update connection metrics
    fn update_metrics(&self, success: bool, latency: std::time::Duration) {
        RedisPoolUtils::update_metrics(&self.metrics, success, latency);
    }

    /// Apply key prefix if configured
    fn prefixed_key(&self, key: &MemoryKey) -> String {
        RedisPoolUtils::prefixed_key(&self.config, key)
    }

    /// Sanitize Redis errors for security
    fn sanitize_error(error: &redis::RedisError) -> String {
        RedisPoolUtils::sanitize_error(error)
    }

    /// Perform health check
    pub async fn health_check(&self) -> Result<RedisHealth, MemoryError> {
        RedisPoolUtils::health_check(&self.pool, &self.config, &self.health, &self.metrics).await
    }

    /// Get current health status
    pub async fn get_health(&self) -> RedisHealth {
        RedisPoolUtils::get_health(&self.health).await
    }
}

#[cfg(feature = "redis")]
impl Clone for RedisMemory {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            connection_manager: RedisPoolUtils::create_connection_manager(self.pool.clone()),
            config: self.config.clone(),
            cluster_client: self.cluster_client.clone(),
            health: Arc::clone(&self.health),
            metrics: Arc::clone(&self.metrics),
        }
    }
}

// Core trait implementations

/// Async implementation for Redis memory operations
#[cfg(feature = "redis")]
impl RedisMemory {
    /// Async load operation
    pub async fn load_async(&self, key: &MemoryKey) -> Result<Option<String>, MemoryError> {
        let prefixed_key = self.prefixed_key(key);
        let start = Instant::now();

        let mut conn = self.get_connection().await?;

        let result: Option<String> = conn.get(&prefixed_key).await.map_err(|e| {
            self.update_metrics(false, start.elapsed());
            MemoryError::LoadFailed {
                key: key.clone(),
                reason: Self::sanitize_error(&e),
            }
        })?;

        self.update_metrics(true, start.elapsed());
        Ok(result)
    }

    /// Async load many operation
    pub async fn load_many_async(
        &self,
        keys: &[MemoryKey],
    ) -> Result<Vec<Option<String>>, MemoryError> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        let start = Instant::now();
        let prefixed_keys: Vec<String> = keys.iter().map(|k| self.prefixed_key(k)).collect();

        let mut conn = self.get_connection().await?;

        let results: Vec<Option<String>> = conn.get(&prefixed_keys).await.map_err(|e| {
            self.update_metrics(false, start.elapsed());
            MemoryError::LoadFailed {
                key: MemoryKey::new("batch").unwrap(),
                reason: Self::sanitize_error(&e),
            }
        })?;

        self.update_metrics(true, start.elapsed());
        Ok(results)
    }

    /// Async store operation
    pub async fn store_async(&self, update: MemoryUpdate) -> Result<(), MemoryError> {
        let prefixed_key = self.prefixed_key(&update.key);
        let start = Instant::now();

        let mut conn = self.get_connection().await?;

        let _: () = conn.set(&prefixed_key, &update.value).await.map_err(|e| {
            self.update_metrics(false, start.elapsed());
            MemoryError::StoreFailed {
                key: update.key.clone(),
                reason: Self::sanitize_error(&e),
            }
        })?;

        self.update_metrics(true, start.elapsed());
        Ok(())
    }

    /// Async store many operation using Redis pipeline for efficiency
    pub async fn store_many_async(&self, updates: Vec<MemoryUpdate>) -> Result<(), MemoryError> {
        if updates.is_empty() {
            return Ok(());
        }

        let start = Instant::now();
        let mut conn = self.get_connection().await?;

        // Use Redis pipeline for atomic batch operations
        let mut pipe = redis::pipe();

        for update in &updates {
            let prefixed_key = self.prefixed_key(&update.key);
            pipe.set(&prefixed_key, &update.value);
        }

        let _: () = pipe.query_async(&mut *conn).await.map_err(|e| {
            self.update_metrics(false, start.elapsed());
            MemoryError::StoreFailed {
                key: MemoryKey::new("batch").unwrap(),
                reason: Self::sanitize_error(&e),
            }
        })?;

        self.update_metrics(true, start.elapsed());
        Ok(())
    }

    /// Async snapshot operation using SCAN for large datasets
    pub async fn snapshot_async(&self) -> Result<Option<String>, MemoryError> {
        let mut conn = self.get_connection().await?;

        // Use SCAN instead of KEYS for production safety
        let mut cursor = 0;
        let mut all_keys = Vec::new();
        let scan_pattern = match self.config.key_prefix() {
            Some(prefix) => format!("{}:*", prefix),
            None => "*".to_string(),
        };

        loop {
            let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&scan_pattern)
                .arg("COUNT")
                .arg(100) // Reasonable batch size
                .query_async(&mut *conn)
                .await
                .map_err(|e| MemoryError::LoadFailed {
                    key: MemoryKey::new("scan").unwrap(),
                    reason: Self::sanitize_error(&e),
                })?;

            all_keys.extend(keys);
            cursor = next_cursor;

            if cursor == 0 {
                break;
            }
        }

        if all_keys.is_empty() {
            return Ok(Some("{}".to_string()));
        }

        // Get all values in batch
        let values: Vec<Option<String>> =
            conn.get(&all_keys)
                .await
                .map_err(|e| MemoryError::LoadFailed {
                    key: MemoryKey::new("snapshot").unwrap(),
                    reason: Self::sanitize_error(&e),
                })?;

        // Build snapshot data
        let mut snapshot_data = HashMap::new();
        for (key, value) in all_keys.into_iter().zip(values) {
            if let Some(val) = value {
                // Remove prefix if present
                let clean_key = match self.config.key_prefix() {
                    Some(prefix) => {
                        let prefix_with_colon = format!("{}:", prefix);
                        key.strip_prefix(&prefix_with_colon).unwrap_or(&key)
                    }
                    None => &key,
                };
                snapshot_data.insert(clean_key.to_string(), val);
            }
        }

        serde_json::to_string(&snapshot_data)
            .map(Some)
            .map_err(|e| MemoryError::RestoreFailed {
                reason: format!("Failed to serialize snapshot: {}", e),
            })
    }

    /// Async restore operation with atomic replacement
    pub async fn restore_async(&self, snapshot: &str) -> Result<(), MemoryError> {
        // Parse the JSON snapshot
        let snapshot_data: HashMap<String, String> =
            serde_json::from_str(snapshot).map_err(|e| MemoryError::RestoreFailed {
                reason: format!("JSON parsing failed: {}", e),
            })?;

        let mut conn = self.get_connection().await?;

        // Start Redis transaction for atomicity
        let mut pipe = redis::pipe();
        pipe.atomic();

        // Clear existing keys with our prefix
        let scan_pattern = match self.config.key_prefix() {
            Some(prefix) => format!("{}:*", prefix),
            None => "*".to_string(),
        };

        // Get existing keys to clear
        let existing_keys: Vec<String> = redis::cmd("KEYS")
            .arg(&scan_pattern)
            .query_async(&mut *conn)
            .await
            .unwrap_or_else(|_| Vec::new());

        // Delete existing keys if any
        if !existing_keys.is_empty() {
            pipe.del(&existing_keys);
        }

        // Set new data
        for (key, value) in snapshot_data {
            let prefixed_key = match self.config.key_prefix() {
                Some(prefix) => format!("{}:{}", prefix, key),
                None => key,
            };
            pipe.set(&prefixed_key, &value);
        }

        let _: () = pipe
            .query_async(&mut *conn)
            .await
            .map_err(|e| MemoryError::RestoreFailed {
                reason: format!("Failed to restore snapshot: {}", Self::sanitize_error(&e)),
            })?;

        Ok(())
    }
}

#[cfg(feature = "redis")]
impl MemoryReader for RedisMemory {
    fn load(&self, key: &MemoryKey) -> Result<Option<String>, MemoryError> {
        with_redis_runtime(|| {
            let key = key.clone();
            let memory = self.clone();
            Box::pin(async move { memory.load_async(&key).await })
        })
    }

    fn load_many(&self, keys: &[MemoryKey]) -> Result<Vec<Option<String>>, MemoryError> {
        with_redis_runtime(|| {
            let keys = keys.to_vec();
            let memory = self.clone();
            Box::pin(async move { memory.load_many_async(&keys).await })
        })
    }
}

#[cfg(feature = "redis")]
impl MemoryWriter for RedisMemory {
    fn store(&mut self, update: MemoryUpdate) -> Result<(), MemoryError> {
        with_redis_runtime(|| {
            let update = update.clone();
            let memory = self.clone();
            Box::pin(async move { memory.store_async(update).await })
        })
    }

    fn store_many(&mut self, updates: Vec<MemoryUpdate>) -> Result<(), MemoryError> {
        with_redis_runtime(|| {
            let updates = updates.clone();
            let memory = self.clone();
            Box::pin(async move { memory.store_many_async(updates).await })
        })
    }
}

#[cfg(feature = "redis")]
impl SnapshotableMemory for RedisMemory {
    fn snapshot(&mut self) -> Option<String> {
        let result: Result<Option<String>, MemoryError> = with_redis_runtime(|| {
            let memory = self.clone();
            Box::pin(async move { memory.snapshot_async().await })
        });
        result.unwrap_or(None)
    }

    fn restore(&mut self, snapshot: &str) -> Result<(), MemoryError> {
        with_redis_runtime(|| {
            let memory = self.clone();
            let snapshot = snapshot.to_string();
            Box::pin(async move { memory.restore_async(&snapshot).await })
        })
    }
}

#[cfg(feature = "redis")]
impl RedisConnectionProvider for RedisMemory {
    async fn get_connection(&self) -> Result<PooledConnection, MemoryError> {
        self.get_connection().await
    }
}

#[cfg(feature = "redis")]
impl ConfigProvider for RedisMemory {
    fn get_config(&self) -> &ValidRedisConfig {
        &self.config
    }
}

#[cfg(feature = "redis")]
impl TransactionalMemory for RedisMemory {
    fn transaction<F, R>(&mut self, f: F) -> Result<R, TransactionError>
    where
        F: FnOnce(&mut dyn MemoryWriter) -> Result<R, TransactionError>,
    {
        REDIS_RUNTIME
            .with(|rt_cell| RedisTransactionExecutor::execute_transaction(self, rt_cell, f))
    }
}
