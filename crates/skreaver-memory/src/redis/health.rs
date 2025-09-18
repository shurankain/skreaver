//! Redis health monitoring and connection pool statistics
//!
//! This module provides health monitoring capabilities for Redis connections,
//! including pool statistics and connection health tracking.

use std::time::Instant;

/// Health status for Redis connections
#[cfg(feature = "redis")]
#[derive(Debug, Clone)]
pub struct RedisHealth {
    /// Is the connection healthy?
    pub healthy: bool,
    /// Last successful ping timestamp
    pub last_ping: Option<Instant>,
    /// Current pool statistics
    pub pool_stats: PoolStats,
    /// Redis server info
    pub server_info: Option<String>,
    /// Error message if unhealthy
    pub error: Option<String>,
}

/// Connection pool statistics
#[cfg(feature = "redis")]
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Total connections in pool
    pub total_connections: usize,
    /// Currently idle connections
    pub idle_connections: usize,
    /// Currently active connections
    pub active_connections: usize,
    /// Pool creation timestamp
    pub created_at: Instant,
}

/// Connection metrics for monitoring
#[cfg(feature = "redis")]
#[derive(Debug, Default)]
pub struct ConnectionMetrics {
    pub total_commands: u64,
    pub successful_commands: u64,
    pub failed_commands: u64,
    pub avg_latency_ms: f64,
    pub last_error: Option<String>,
}
