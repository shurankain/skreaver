//! Redis connection pool management with health monitoring
//!
//! This module handles Redis connection pooling for different deployment types
//! with comprehensive health monitoring and error handling.

use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(feature = "redis")]
use deadpool_redis::{Config as PoolConfig, Pool};
#[cfg(feature = "redis")]
use redis::cluster::ClusterClient;

use skreaver_core::error::MemoryError;
use skreaver_core::memory::MemoryKey;

use super::config::{Cluster, RedisDeploymentV2, Sentinel, Standalone, ValidRedisConfig};
use super::connection::StatefulConnectionManager;
use super::health::{ConnectionMetrics, PoolStats, RedisHealth};

/// Redis connection pool utility functions
#[cfg(feature = "redis")]
pub struct RedisPoolUtils;

#[cfg(feature = "redis")]
impl RedisPoolUtils {
    /// Create connection pool based on deployment type
    pub async fn create_pool(
        config: &ValidRedisConfig,
    ) -> Result<(Pool, Option<Arc<ClusterClient>>), MemoryError> {
        match config.deployment() {
            RedisDeploymentV2::Standalone(standalone) => {
                Self::create_standalone_pool(standalone, config).await
            }
            RedisDeploymentV2::Cluster(cluster) => {
                Self::create_cluster_pool(cluster, config).await
            }
            RedisDeploymentV2::Sentinel(sentinel) => {
                Self::create_sentinel_pool(sentinel, config).await
            }
        }
    }

    /// Create standalone Redis pool
    async fn create_standalone_pool(
        standalone: &Standalone,
        _config: &ValidRedisConfig,
    ) -> Result<(Pool, Option<Arc<ClusterClient>>), MemoryError> {
        let pool_config = PoolConfig::from_url(standalone.url.as_str());
        let pool = pool_config
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .map_err(|e| MemoryError::ConnectionFailed {
                backend: skreaver_core::error::MemoryBackend::Redis,
                kind: skreaver_core::error::MemoryErrorKind::InternalError {
                    backend_error: format!("Failed to create connection pool: {}", e),
                },
            })?;

        Ok((pool, None))
    }

    /// Create cluster Redis pool
    async fn create_cluster_pool(
        cluster: &Cluster,
        _config: &ValidRedisConfig,
    ) -> Result<(Pool, Option<Arc<ClusterClient>>), MemoryError> {
        let urls: Vec<&str> = cluster
            .nodes
            .as_slice()
            .iter()
            .map(|node| node.as_str())
            .collect();

        let cluster_client =
            ClusterClient::new(urls).map_err(|e| MemoryError::ConnectionFailed {
                backend: skreaver_core::error::MemoryBackend::Redis,
                kind: skreaver_core::error::MemoryErrorKind::InternalError {
                    backend_error: format!("Failed to create cluster client: {}", e),
                },
            })?;

        // For cluster, use the first node for pool creation
        let pool_config = PoolConfig::from_url(cluster.nodes.first().as_str());
        let pool = pool_config
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .map_err(|e| MemoryError::ConnectionFailed {
                backend: skreaver_core::error::MemoryBackend::Redis,
                kind: skreaver_core::error::MemoryErrorKind::InternalError {
                    backend_error: format!("Failed to create cluster pool: {}", e),
                },
            })?;

        Ok((pool, Some(Arc::new(cluster_client))))
    }

    /// Create sentinel Redis pool
    async fn create_sentinel_pool(
        sentinel: &Sentinel,
        _config: &ValidRedisConfig,
    ) -> Result<(Pool, Option<Arc<ClusterClient>>), MemoryError> {
        // For now, use the first sentinel as the connection URL
        // In a full implementation, this would use Redis Sentinel protocol
        let pool_config = PoolConfig::from_url(sentinel.sentinels.first().as_str());
        let pool = pool_config
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .map_err(|e| MemoryError::ConnectionFailed {
                backend: skreaver_core::error::MemoryBackend::Redis,
                kind: skreaver_core::error::MemoryErrorKind::InternalError {
                    backend_error: format!("Failed to create sentinel pool: {}", e),
                },
            })?;

        Ok((pool, None))
    }

    /// Get connection from pool with metrics tracking
    pub async fn get_connection(
        pool: &Pool,
        metrics: &Arc<tokio::sync::Mutex<ConnectionMetrics>>,
    ) -> Result<deadpool_redis::Connection, MemoryError> {
        let start = Instant::now();

        let conn = pool.get().await.map_err(|e| {
            Self::update_metrics(metrics, false, start.elapsed());
            MemoryError::ConnectionFailed {
                backend: skreaver_core::error::MemoryBackend::Redis,
                kind: skreaver_core::error::MemoryErrorKind::InternalError {
                    backend_error: format!("Failed to get connection from pool: {}", e),
                },
            }
        })?;

        Self::update_metrics(metrics, true, start.elapsed());
        Ok(conn)
    }

    /// Apply key prefix based on configuration
    pub fn prefixed_key(config: &ValidRedisConfig, key: &MemoryKey) -> String {
        match &config.key_prefix {
            Some(prefix) => format!("{}:{}", prefix.as_str(), key.as_str()),
            None => key.as_str().to_string(),
        }
    }

    /// Update connection metrics
    pub fn update_metrics(
        metrics: &Arc<tokio::sync::Mutex<ConnectionMetrics>>,
        success: bool,
        latency: Duration,
    ) {
        // Use spawn to avoid blocking
        let metrics = Arc::clone(metrics);
        tokio::spawn(async move {
            let mut metrics_guard = metrics.lock().await;
            if success {
                metrics_guard.successful_commands += 1;
            } else {
                metrics_guard.failed_commands += 1;
            }
            metrics_guard.total_commands += 1;
            metrics_guard.avg_latency_ms =
                (metrics_guard.avg_latency_ms as f64 + latency.as_millis() as f64) / 2.0;
        });
    }

    /// Sanitize Redis errors for security (remove sensitive info)
    pub fn sanitize_error(error: &redis::RedisError) -> String {
        let error_str = error.to_string();

        // Remove potential sensitive information from error messages
        if error_str.contains("password") || error_str.contains("auth") {
            "Authentication failed".to_string()
        } else if error_str.contains("connection") {
            "Connection error".to_string()
        } else {
            // Keep generic error info but limit length
            let sanitized = error_str.chars().take(100).collect::<String>();
            if sanitized.len() < error_str.len() {
                format!("{}...", sanitized)
            } else {
                sanitized
            }
        }
    }

    /// Perform comprehensive health check
    pub async fn health_check(
        pool: &Pool,
        config: &ValidRedisConfig,
        health: &Arc<tokio::sync::RwLock<RedisHealth>>,
        metrics: &Arc<tokio::sync::Mutex<ConnectionMetrics>>,
    ) -> Result<RedisHealth, MemoryError> {
        let start = Instant::now();

        let mut conn = Self::get_connection(pool, metrics).await?;

        // Perform health check operations
        let result: Result<String, redis::RedisError> =
            redis::cmd("PING").query_async(&mut *conn).await;

        let mut health_guard = health.write().await;

        match result {
            Ok(_) => {
                health_guard.healthy = true;
                health_guard.last_ping = Some(start);
                health_guard.error = None;

                // Update pool stats
                health_guard.pool_stats = PoolStats {
                    total_connections: config.pool_size(),
                    idle_connections: config.pool_size(), // Simplified
                    active_connections: 0,
                    created_at: health_guard.pool_stats.created_at,
                };
            }
            Err(e) => {
                health_guard.healthy = false;
                health_guard.error = Some(Self::sanitize_error(&e));
            }
        }

        Ok(health_guard.clone())
    }

    /// Get current health status
    pub async fn get_health(health: &Arc<tokio::sync::RwLock<RedisHealth>>) -> RedisHealth {
        health.read().await.clone()
    }

    /// Create a stateful connection manager from pool
    pub fn create_connection_manager(pool: Pool) -> StatefulConnectionManager {
        StatefulConnectionManager::new(pool)
            .with_max_idle_duration(Duration::from_secs(300))
            .with_max_retry_attempts(3)
    }
}
