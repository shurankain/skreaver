//! Redis health monitoring and connection pool statistics
//!
//! This module provides health monitoring capabilities for Redis connections,
//! including pool statistics and connection health tracking.

use std::time::Instant;

/// Health status for Redis connections.
///
/// This enum uses the typestate pattern to make invalid states unrepresentable.
/// A connection cannot be both healthy and have an error simultaneously.
#[cfg(feature = "redis")]
#[derive(Debug, Clone)]
pub enum RedisHealth {
    /// Connection is healthy and operational
    Healthy {
        /// Last successful ping timestamp
        last_ping: Instant,
        /// Current pool statistics
        pool_stats: PoolStats,
        /// Redis server info (optional)
        server_info: Option<String>,
    },
    /// Connection is unhealthy or unavailable
    Unhealthy {
        /// Error message describing the problem
        error: String,
        /// Last successful ping timestamp (if any)
        last_successful_ping: Option<Instant>,
        /// Current pool statistics
        pool_stats: PoolStats,
    },
}

impl RedisHealth {
    /// Check if the connection is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy { .. })
    }

    /// Get the error message if unhealthy
    pub fn error(&self) -> Option<&str> {
        match self {
            Self::Healthy { .. } => None,
            Self::Unhealthy { error, .. } => Some(error),
        }
    }

    /// Get the last ping timestamp (successful or last known)
    pub fn last_ping(&self) -> Option<Instant> {
        match self {
            Self::Healthy { last_ping, .. } => Some(*last_ping),
            Self::Unhealthy {
                last_successful_ping,
                ..
            } => *last_successful_ping,
        }
    }

    /// Get pool statistics
    pub fn pool_stats(&self) -> &PoolStats {
        match self {
            Self::Healthy { pool_stats, .. } => pool_stats,
            Self::Unhealthy { pool_stats, .. } => pool_stats,
        }
    }

    /// Get server info if available
    pub fn server_info(&self) -> Option<&str> {
        match self {
            Self::Healthy { server_info, .. } => server_info.as_deref(),
            Self::Unhealthy { .. } => None,
        }
    }
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
