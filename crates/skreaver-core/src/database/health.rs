//! Shared health check types and traits for database backends
//!
//! This module provides a unified interface for health monitoring across
//! different database backends (Redis, PostgreSQL, SQLite, etc.).
//!
//! # Design Philosophy
//!
//! - **Backend-agnostic**: Common interface for all database types
//! - **Async-first**: Support for both sync and async health checks
//! - **Rich diagnostics**: Detailed health status with metrics
//! - **Type-safe**: Strong typing for health states
//!
//! # Example
//!
//! ```rust,ignore
//! use skreaver_core::database::health::{HealthCheck, HealthStatus};
//!
//! async fn monitor_health(db: impl HealthCheck) {
//!     let health = db.check_health().await.unwrap();
//!     match health.status {
//!         HealthStatus::Healthy => println!("Database is healthy"),
//!         HealthStatus::Degraded => println!("Database is degraded: {}", health.message.unwrap()),
//!         HealthStatus::Unhealthy => println!("Database is unhealthy!"),
//!     }
//! }
//! ```

use std::time::{Duration, Instant, SystemTime};

/// Health status levels for database connections
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HealthStatus {
    /// Database is fully operational
    Healthy,
    /// Database is operational but with reduced performance
    Degraded,
    /// Database is not operational
    Unhealthy,
}

impl HealthStatus {
    /// Check if the status is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self, HealthStatus::Healthy)
    }

    /// Check if the status is degraded
    pub fn is_degraded(&self) -> bool {
        matches!(self, HealthStatus::Degraded)
    }

    /// Check if the status is unhealthy
    pub fn is_unhealthy(&self) -> bool {
        matches!(self, HealthStatus::Unhealthy)
    }

    /// Check if the database is operational (healthy or degraded)
    pub fn is_operational(&self) -> bool {
        !self.is_unhealthy()
    }
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "healthy"),
            HealthStatus::Degraded => write!(f, "degraded"),
            HealthStatus::Unhealthy => write!(f, "unhealthy"),
        }
    }
}

/// Connection pool statistics
#[derive(Debug, Clone)]
pub struct PoolStatistics {
    /// Total number of connections in the pool
    pub total_connections: usize,
    /// Number of idle (available) connections
    pub idle_connections: usize,
    /// Number of active (in-use) connections
    pub active_connections: usize,
    /// When the pool was created
    pub created_at: Instant,
    /// Pool utilization percentage (0.0-1.0)
    pub utilization: f64,
}

impl PoolStatistics {
    /// Create new pool statistics
    pub fn new(total: usize, idle: usize, active: usize, created_at: Instant) -> Self {
        let utilization = if total > 0 {
            active as f64 / total as f64
        } else {
            0.0
        };

        Self {
            total_connections: total,
            idle_connections: idle,
            active_connections: active,
            created_at,
            utilization,
        }
    }

    /// Check if the pool is at capacity
    pub fn is_at_capacity(&self) -> bool {
        self.active_connections >= self.total_connections
    }

    /// Check if the pool is underutilized (< 50%)
    pub fn is_underutilized(&self) -> bool {
        self.utilization < 0.5
    }

    /// Check if the pool is overutilized (> 80%)
    pub fn is_overutilized(&self) -> bool {
        self.utilization > 0.8
    }
}

/// Performance metrics for database operations
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// Total number of operations executed
    pub total_operations: u64,
    /// Number of successful operations
    pub successful_operations: u64,
    /// Number of failed operations
    pub failed_operations: u64,
    /// Average operation latency in milliseconds
    pub avg_latency_ms: f64,
    /// Minimum operation latency in milliseconds
    pub min_latency_ms: Option<f64>,
    /// Maximum operation latency in milliseconds
    pub max_latency_ms: Option<f64>,
    /// Last recorded error
    pub last_error: Option<String>,
    /// When metrics were last reset
    pub last_reset: Instant,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            avg_latency_ms: 0.0,
            min_latency_ms: None,
            max_latency_ms: None,
            last_error: None,
            last_reset: Instant::now(),
        }
    }
}

impl PerformanceMetrics {
    /// Record a successful operation
    pub fn record_success(&mut self, latency: Duration) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.update_latency(latency);
    }

    /// Record a failed operation
    pub fn record_failure(&mut self, latency: Duration, error: String) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.update_latency(latency);
        self.last_error = Some(error);
    }

    /// Update latency statistics
    fn update_latency(&mut self, latency: Duration) {
        let latency_ms = latency.as_secs_f64() * 1000.0;

        // Update average (simple moving average)
        self.avg_latency_ms = if self.total_operations == 1 {
            latency_ms
        } else {
            (self.avg_latency_ms * (self.total_operations - 1) as f64 + latency_ms)
                / self.total_operations as f64
        };

        // Update min/max
        self.min_latency_ms = Some(
            self.min_latency_ms
                .map_or(latency_ms, |min| min.min(latency_ms)),
        );
        self.max_latency_ms = Some(
            self.max_latency_ms
                .map_or(latency_ms, |max| max.max(latency_ms)),
        );
    }

    /// Calculate success rate (0.0-1.0)
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            1.0
        } else {
            self.successful_operations as f64 / self.total_operations as f64
        }
    }

    /// Calculate failure rate (0.0-1.0)
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Reset all metrics
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Comprehensive health report for a database backend
#[derive(Debug, Clone)]
pub struct HealthReport {
    /// Overall health status
    pub status: HealthStatus,
    /// When the health check was performed
    pub checked_at: SystemTime,
    /// How long the health check took
    pub check_duration: Duration,
    /// Optional status message
    pub message: Option<String>,
    /// Connection pool statistics
    pub pool_stats: Option<PoolStatistics>,
    /// Performance metrics
    pub metrics: Option<PerformanceMetrics>,
    /// Backend-specific information
    pub backend_info: Option<String>,
    /// Last successful ping timestamp
    pub last_successful_ping: Option<Instant>,
}

impl HealthReport {
    /// Create a healthy report
    pub fn healthy(message: impl Into<String>) -> Self {
        Self {
            status: HealthStatus::Healthy,
            checked_at: SystemTime::now(),
            check_duration: Duration::from_millis(0),
            message: Some(message.into()),
            pool_stats: None,
            metrics: None,
            backend_info: None,
            last_successful_ping: Some(Instant::now()),
        }
    }

    /// Create a degraded report
    pub fn degraded(reason: impl Into<String>) -> Self {
        Self {
            status: HealthStatus::Degraded,
            checked_at: SystemTime::now(),
            check_duration: Duration::from_millis(0),
            message: Some(reason.into()),
            pool_stats: None,
            metrics: None,
            backend_info: None,
            last_successful_ping: None,
        }
    }

    /// Create an unhealthy report
    pub fn unhealthy(reason: impl Into<String>) -> Self {
        Self {
            status: HealthStatus::Unhealthy,
            checked_at: SystemTime::now(),
            check_duration: Duration::from_millis(0),
            message: Some(reason.into()),
            pool_stats: None,
            metrics: None,
            backend_info: None,
            last_successful_ping: None,
        }
    }

    /// Add pool statistics to the report
    pub fn with_pool_stats(mut self, stats: PoolStatistics) -> Self {
        self.pool_stats = Some(stats);
        self
    }

    /// Add performance metrics to the report
    pub fn with_metrics(mut self, metrics: PerformanceMetrics) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Add backend information to the report
    pub fn with_backend_info(mut self, info: impl Into<String>) -> Self {
        self.backend_info = Some(info.into());
        self
    }

    /// Set the check duration
    pub fn with_check_duration(mut self, duration: Duration) -> Self {
        self.check_duration = duration;
        self
    }

    /// Check if the report indicates a healthy database
    pub fn is_healthy(&self) -> bool {
        self.status.is_healthy()
    }

    /// Check if the report indicates an operational database
    pub fn is_operational(&self) -> bool {
        self.status.is_operational()
    }
}

/// Trait for database health checks
///
/// Implement this trait for database backends to provide unified health monitoring.
#[async_trait::async_trait]
pub trait HealthCheck {
    /// Perform a health check on the database
    ///
    /// This should:
    /// - Test connectivity (e.g., PING for Redis, SELECT 1 for SQL)
    /// - Gather pool statistics
    /// - Measure response time
    /// - Return comprehensive health report
    async fn check_health(&self) -> Result<HealthReport, crate::error::MemoryError>;

    /// Quick health check (just connectivity, no detailed stats)
    ///
    /// Default implementation uses full health check but backends can
    /// optimize this for faster checks.
    async fn ping(&self) -> Result<bool, crate::error::MemoryError> {
        Ok(self.check_health().await?.is_operational())
    }

    /// Get current pool statistics without performing a full health check
    ///
    /// Default implementation returns None. Backends with pool support should override.
    async fn pool_stats(&self) -> Option<PoolStatistics> {
        None
    }

    /// Get current performance metrics
    ///
    /// Default implementation returns None. Backends tracking metrics should override.
    async fn metrics(&self) -> Option<PerformanceMetrics> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status() {
        assert!(HealthStatus::Healthy.is_healthy());
        assert!(HealthStatus::Degraded.is_degraded());
        assert!(HealthStatus::Unhealthy.is_unhealthy());

        assert!(HealthStatus::Healthy.is_operational());
        assert!(HealthStatus::Degraded.is_operational());
        assert!(!HealthStatus::Unhealthy.is_operational());
    }

    #[test]
    fn test_pool_statistics() {
        let stats = PoolStatistics::new(10, 5, 5, Instant::now());
        assert_eq!(stats.total_connections, 10);
        assert_eq!(stats.idle_connections, 5);
        assert_eq!(stats.active_connections, 5);
        assert_eq!(stats.utilization, 0.5);
        assert!(!stats.is_at_capacity());
        assert!(!stats.is_underutilized());
        assert!(!stats.is_overutilized());
    }

    #[test]
    fn test_performance_metrics() {
        let mut metrics = PerformanceMetrics::default();

        metrics.record_success(Duration::from_millis(10));
        assert_eq!(metrics.total_operations, 1);
        assert_eq!(metrics.successful_operations, 1);
        assert_eq!(metrics.success_rate(), 1.0);

        metrics.record_failure(Duration::from_millis(20), "error".to_string());
        assert_eq!(metrics.total_operations, 2);
        assert_eq!(metrics.failed_operations, 1);
        assert_eq!(metrics.success_rate(), 0.5);
    }

    #[test]
    fn test_health_report() {
        let report = HealthReport::healthy("All systems operational");
        assert!(report.is_healthy());
        assert!(report.is_operational());

        let report = HealthReport::degraded("High latency");
        assert!(!report.is_healthy());
        assert!(report.is_operational());

        let report = HealthReport::unhealthy("Connection failed");
        assert!(!report.is_healthy());
        assert!(!report.is_operational());
    }
}
