//! Health monitoring for SQLite backends
//!
//! This module provides health status tracking and monitoring capabilities.

use super::pool::PoolHealth;

/// Structured health status for SQLite backend
#[derive(Debug, Clone)]
pub enum HealthStatus {
    Healthy {
        details: String,
        pool_status: PoolHealth,
    },
    Degraded {
        reason: String,
        pool_status: PoolHealth,
    },
    Unhealthy {
        reason: String,
        error_count: u32,
    },
}
