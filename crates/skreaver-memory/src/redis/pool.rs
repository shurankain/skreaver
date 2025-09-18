//! Redis connection pool management with health monitoring and metrics
//!
//! This module provides connection pooling functionality with support for
//! standalone, cluster, and sentinel Redis deployments, along with
//! comprehensive connection health monitoring and performance metrics.

use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(feature = "redis")]
use deadpool_redis::{Config as PoolConfig, Connection as PooledConnection, Pool};
#[cfg(feature = "redis")]
use redis::{ErrorKind as RedisErrorKind, RedisError, cluster::ClusterClient};

use skreaver_core::error::MemoryError;
use skreaver_core::memory::MemoryKey;

use super::config::{RedisConfig, RedisDeployment};
use super::health::{ConnectionMetrics, PoolStats, RedisHealth};

/// Redis connection pool utility functions
#[cfg(feature = "redis")]
pub struct RedisPoolUtils;

#[cfg(feature = "redis")]
impl RedisPoolUtils {
    /// Create connection pool based on deployment type
    pub async fn create_pool(
        config: &RedisConfig,
    ) -> Result<(Pool, Option<Arc<ClusterClient>>), MemoryError> {
        match &config.deployment {
            RedisDeployment::Standalone { url } => {
                let pool_config = PoolConfig::from_url(url);
                let pool = pool_config
                    .create_pool(Some(deadpool_redis::Runtime::Tokio1))
                    .map_err(|e| MemoryError::ConnectionFailed {
                        backend: "redis".to_string(),
                        reason: format!("Failed to create connection pool: {}", e),
                    })?;

                Ok((pool, None))
            }
            RedisDeployment::Cluster { nodes } => {
                let cluster_client = ClusterClient::new(nodes.clone()).map_err(|e| {
                    MemoryError::ConnectionFailed {
                        backend: "redis".to_string(),
                        reason: format!("Failed to create cluster client: {}", e),
                    }
                })?;

                // For cluster, we'll use the first node for the pool
                // In production, you might want a more sophisticated approach
                let pool_config = PoolConfig::from_url(&nodes[0]);
                let pool = pool_config
                    .create_pool(Some(deadpool_redis::Runtime::Tokio1))
                    .map_err(|e| MemoryError::ConnectionFailed {
                        backend: "redis".to_string(),
                        reason: format!("Failed to create cluster pool: {}", e),
                    })?;

                Ok((pool, Some(Arc::new(cluster_client))))
            }
            RedisDeployment::Sentinel {
                sentinels: _,
                service_name: _,
            } => {
                // Sentinel support would be implemented here
                Err(MemoryError::ConnectionFailed {
                    backend: "redis".to_string(),
                    reason: "Redis Sentinel support not implemented yet".to_string(),
                })
            }
        }
    }

    /// Get a pooled connection
    pub async fn get_connection(
        pool: &Pool,
        metrics: &tokio::sync::Mutex<ConnectionMetrics>,
    ) -> Result<PooledConnection, MemoryError> {
        let start = Instant::now();

        let conn = pool.get().await.map_err(|e| {
            Self::update_metrics(metrics, false, start.elapsed());
            MemoryError::ConnectionFailed {
                backend: "redis".to_string(),
                reason: format!("Failed to get connection from pool: {}", e),
            }
        })?;

        Self::update_metrics(metrics, true, start.elapsed());
        Ok(conn)
    }

    /// Update connection metrics
    pub fn update_metrics(
        metrics: &tokio::sync::Mutex<ConnectionMetrics>,
        success: bool,
        latency: Duration,
    ) {
        if let Ok(mut metrics_guard) = metrics.try_lock() {
            metrics_guard.total_commands += 1;
            if success {
                metrics_guard.successful_commands += 1;
            } else {
                metrics_guard.failed_commands += 1;
            }

            let latency_ms = latency.as_secs_f64() * 1000.0;
            metrics_guard.avg_latency_ms = (metrics_guard.avg_latency_ms
                * (metrics_guard.total_commands - 1) as f64
                + latency_ms)
                / metrics_guard.total_commands as f64;
        }
    }

    /// Apply key prefix if configured
    pub fn prefixed_key(config: &RedisConfig, key: &MemoryKey) -> String {
        match &config.key_prefix {
            Some(prefix) => format!("{}:{}", prefix, key.as_str()),
            None => key.as_str().to_string(),
        }
    }

    /// Sanitize Redis errors for security
    pub fn sanitize_error(error: &RedisError) -> String {
        match error.kind() {
            RedisErrorKind::AuthenticationFailed => "Authentication failed".to_string(),
            RedisErrorKind::TypeError => "Data type error".to_string(),
            RedisErrorKind::ExecAbortError => "Transaction aborted".to_string(),
            RedisErrorKind::BusyLoadingError => "Redis is loading data".to_string(),
            RedisErrorKind::NoScriptError => "Script not found".to_string(),
            RedisErrorKind::ReadOnly => "Redis is read-only".to_string(),
            _ if error.to_string().contains("connection") => "Connection error".to_string(),
            _ if error.to_string().contains("timeout") => "Operation timeout".to_string(),
            _ => "Redis operation failed".to_string(),
        }
    }

    /// Perform health check
    pub async fn health_check(
        pool: &Pool,
        config: &RedisConfig,
        health: &tokio::sync::RwLock<RedisHealth>,
        metrics: &tokio::sync::Mutex<ConnectionMetrics>,
    ) -> Result<RedisHealth, MemoryError> {
        let start = Instant::now();

        let result = async {
            let mut conn = Self::get_connection(pool, metrics).await?;

            // Perform PING
            let _: String = redis::cmd("PING")
                .query_async(&mut *conn)
                .await
                .map_err(|e| MemoryError::ConnectionFailed {
                    backend: "redis".to_string(),
                    reason: Self::sanitize_error(&e),
                })?;

            // Get server info
            let info: String = redis::cmd("INFO")
                .arg("server")
                .query_async(&mut *conn)
                .await
                .unwrap_or_else(|_| "unavailable".to_string());

            Ok::<_, MemoryError>(info)
        }
        .await;

        let mut health_guard = health.write().await;

        match result {
            Ok(server_info) => {
                health_guard.healthy = true;
                health_guard.last_ping = Some(start);
                health_guard.server_info = Some(server_info);
                health_guard.error = None;

                // Update pool stats (simplified)
                health_guard.pool_stats = PoolStats {
                    total_connections: config.pool_size,
                    idle_connections: config.pool_size, // Simplified
                    active_connections: 0,
                    created_at: health_guard.pool_stats.created_at,
                };
            }
            Err(e) => {
                health_guard.healthy = false;
                health_guard.error = Some(e.to_string());
            }
        }

        Ok(health_guard.clone())
    }

    /// Get current health status
    pub async fn get_health(health: &tokio::sync::RwLock<RedisHealth>) -> RedisHealth {
        health.read().await.clone()
    }
}
