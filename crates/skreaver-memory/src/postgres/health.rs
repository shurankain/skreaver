//! PostgreSQL health monitoring
//!
//! This module provides health status structures and monitoring functionality for PostgreSQL connections.

/// Health status for PostgreSQL connection pool
#[derive(Debug, Clone)]
pub struct PostgresPoolHealth {
    pub available_connections: usize,
    pub total_connections: usize,
    pub active_connections: usize,
    pub server_version: String,
    pub last_check: std::time::Instant,
}