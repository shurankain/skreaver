//! Administrative operations for memory backends
//!
//! This module provides common administrative operations for database-backed
//! memory implementations, including backup, restore, health monitoring, and
//! schema migrations.

use skreaver_core::error::MemoryError;
use std::time::SystemTime;

/// Administrative operations trait for database memory backends
///
/// This trait provides common administrative functionality for backends that
/// support persistence, health monitoring, and schema migrations.
pub trait MemoryAdmin {
    /// Create a backup handle for the memory backend
    ///
    /// Returns a `BackupHandle` containing the backup data and metadata.
    fn backup(&self) -> Result<BackupHandle, MemoryError>;

    /// Restore from a backup handle
    ///
    /// Restores the memory backend state from a previously created backup.
    fn restore_from_backup(&mut self, handle: BackupHandle) -> Result<(), MemoryError>;

    /// Run schema migrations to a specific version
    ///
    /// Migrates the database schema to the specified version. If `None` is provided,
    /// migrates to the latest available version.
    fn migrate_to_version(&mut self, version: Option<u32>) -> Result<(), MemoryError>;

    /// Get structured health status
    ///
    /// Returns the current health status of the backend including connection pool
    /// information and error counts.
    fn health_status(&self) -> Result<HealthStatus, MemoryError>;

    /// Get migration status information
    ///
    /// Returns information about current and pending migrations.
    fn migration_status(&self) -> Result<MigrationStatus, MemoryError>;
}

/// Handle for backup operations
///
/// Contains backup data and metadata for restoration purposes.
#[derive(Debug, Clone)]
pub struct BackupHandle {
    /// Unique identifier for this backup
    pub id: String,
    /// When the backup was created
    pub created_at: SystemTime,
    /// Size of the backup in bytes
    pub size_bytes: u64,
    /// Format of the backup data
    pub format: BackupFormat,
    /// The actual backup data
    pub data: Vec<u8>,
}

/// Backup format types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupFormat {
    /// JSON format (portable, text-based)
    Json,
    /// SQLite dump format (SQLite-specific)
    SqliteDump,
    /// PostgreSQL dump format (PostgreSQL-specific)
    PostgresDump,
    /// Custom binary format
    Binary,
}

/// Health status severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HealthSeverity {
    /// Backend is fully operational
    Healthy,
    /// Backend is operational but experiencing issues
    Degraded,
    /// Backend is not operational
    Unhealthy,
}

/// Structured health status for memory backends with unified structure
/// This design ensures pool_status is always consistently available when applicable
#[derive(Debug, Clone)]
pub struct HealthStatus {
    /// Severity level of the health status
    pub severity: HealthSeverity,
    /// Description of the current state (details for Healthy, reason for Degraded/Unhealthy)
    pub message: String,
    /// Connection pool health information (None if pool unavailable)
    pub pool_status: Option<PoolHealth>,
    /// Number of errors encountered (relevant for Unhealthy status)
    pub error_count: u32,
}

impl HealthStatus {
    /// Create a healthy status
    pub fn healthy(message: impl Into<String>, pool_status: PoolHealth) -> Self {
        Self {
            severity: HealthSeverity::Healthy,
            message: message.into(),
            pool_status: Some(pool_status),
            error_count: 0,
        }
    }

    /// Create a degraded status
    pub fn degraded(reason: impl Into<String>, pool_status: PoolHealth) -> Self {
        Self {
            severity: HealthSeverity::Degraded,
            message: reason.into(),
            pool_status: Some(pool_status),
            error_count: 0,
        }
    }

    /// Create an unhealthy status
    pub fn unhealthy(reason: impl Into<String>, error_count: u32) -> Self {
        Self {
            severity: HealthSeverity::Unhealthy,
            message: reason.into(),
            pool_status: None,
            error_count,
        }
    }

    /// Check if the backend is healthy
    pub fn is_healthy(&self) -> bool {
        self.severity == HealthSeverity::Healthy
    }

    /// Check if the backend is degraded
    pub fn is_degraded(&self) -> bool {
        self.severity == HealthSeverity::Degraded
    }

    /// Check if the backend is unhealthy
    pub fn is_unhealthy(&self) -> bool {
        self.severity == HealthSeverity::Unhealthy
    }
}

/// Connection pool health information
#[derive(Debug, Clone)]
pub struct PoolHealth {
    /// Number of healthy connections in the pool
    pub healthy_connections: usize,
    /// Total number of connections (healthy + unhealthy)
    pub total_connections: usize,
    /// When the last health check was performed
    pub last_check: SystemTime,
}

/// Migration status information
#[derive(Debug, Clone)]
pub struct MigrationStatus {
    /// Current schema version
    pub current_version: u32,
    /// Latest available migration version
    pub latest_version: u32,
    /// List of pending migration versions
    pub pending_migrations: Vec<u32>,
    /// List of already applied migrations
    pub applied_migrations: Vec<AppliedMigration>,
}

/// Information about an applied migration
#[derive(Debug, Clone)]
pub struct AppliedMigration {
    /// Migration version number
    pub version: u32,
    /// Human-readable description of the migration
    pub description: String,
    /// When the migration was applied
    pub applied_at: SystemTime,
}

impl BackupHandle {
    /// Create a new backup handle
    pub fn new(format: BackupFormat, data: Vec<u8>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: SystemTime::now(),
            size_bytes: data.len() as u64,
            format,
            data,
        }
    }
}

impl PoolHealth {
    /// Create a new pool health status
    pub fn new(healthy_connections: usize, total_connections: usize) -> Self {
        Self {
            healthy_connections,
            total_connections,
            last_check: SystemTime::now(),
        }
    }

    /// Check if the pool is healthy
    pub fn is_healthy(&self) -> bool {
        self.healthy_connections > 0 && self.healthy_connections == self.total_connections
    }

    /// Get the percentage of healthy connections
    pub fn health_percentage(&self) -> f64 {
        if self.total_connections == 0 {
            return 0.0;
        }
        (self.healthy_connections as f64 / self.total_connections as f64) * 100.0
    }
}
