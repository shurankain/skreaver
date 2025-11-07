//! Database operation timeout enforcement
//!
//! This module provides timeout enforcement for all database operations to prevent
//! indefinite hangs during migrations, queries, and transactions.
//!
//! # Security
//!
//! Without timeouts, malicious or buggy migrations can:
//! - Cause indefinite hangs during startup
//! - Lock up the entire application
//! - Prevent graceful shutdown
//!
//! This module enforces timeouts using SQLite's built-in busy_timeout mechanism
//! combined with application-level timeout wrappers for long-running operations.

use rusqlite::Connection;
use skreaver_core::error::{MemoryBackend, MemoryError, MemoryErrorKind};
use std::thread;
use std::time::Duration;

/// Configuration for database operation timeouts
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    /// Timeout for individual SQL statements (execute, query, etc.)
    pub statement_timeout: Duration,

    /// Timeout for entire transactions
    pub transaction_timeout: Duration,

    /// Timeout for schema migrations
    pub migration_timeout: Duration,

    /// Timeout for acquiring a connection from the pool
    pub connection_timeout: Duration,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            statement_timeout: Duration::from_secs(30), // 30 seconds per statement
            transaction_timeout: Duration::from_secs(60), // 1 minute per transaction
            migration_timeout: Duration::from_secs(300), // 5 minutes for migrations
            connection_timeout: Duration::from_secs(10), // 10 seconds to get connection
        }
    }
}

impl TimeoutConfig {
    /// Create configuration for production use (stricter timeouts)
    pub fn production() -> Self {
        Self {
            statement_timeout: Duration::from_secs(15),
            transaction_timeout: Duration::from_secs(30),
            migration_timeout: Duration::from_secs(120),
            connection_timeout: Duration::from_secs(5),
        }
    }

    /// Create configuration for development (more lenient)
    pub fn development() -> Self {
        Self {
            statement_timeout: Duration::from_secs(60),
            transaction_timeout: Duration::from_secs(120),
            migration_timeout: Duration::from_secs(600),
            connection_timeout: Duration::from_secs(30),
        }
    }
}

/// Execute a database operation with a timeout
///
/// This function runs the operation in a separate thread and enforces a timeout.
/// If the timeout is exceeded, returns a TimeoutError.
///
/// # Security
///
/// This prevents indefinite hangs from:
/// - Deadlocks
/// - Long-running queries
/// - Buggy migrations
/// - Malicious SQL
pub fn with_timeout<F, T>(timeout: Duration, operation_name: &str, f: F) -> Result<T, MemoryError>
where
    F: FnOnce() -> Result<T, MemoryError> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();

    // Spawn thread to execute operation
    let operation_name = operation_name.to_string();
    thread::spawn(move || {
        let result = f();
        let _ = tx.send(result);
    });

    // Wait for result with timeout
    match rx.recv_timeout(timeout) {
        Ok(result) => result,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(MemoryError::ConnectionFailed {
            backend: MemoryBackend::Sqlite,
            kind: MemoryErrorKind::Timeout {
                operation: operation_name,
                timeout_seconds: timeout.as_secs(),
            },
        }),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            Err(MemoryError::ConnectionFailed {
                backend: MemoryBackend::Sqlite,
                kind: MemoryErrorKind::InternalError {
                    backend_error: format!("Operation '{}' thread disconnected", operation_name),
                },
            })
        }
    }
}

/// Configure a connection with appropriate timeouts
///
/// This sets SQLite's busy_timeout to prevent indefinite blocking on locked databases.
pub fn configure_connection_timeouts(
    conn: &Connection,
    config: &TimeoutConfig,
) -> Result<(), MemoryError> {
    // Set busy timeout - how long to wait for locks
    let timeout_ms = config.statement_timeout.as_millis() as i32;
    conn.busy_timeout(Duration::from_millis(timeout_ms as u64))
        .map_err(|e| MemoryError::ConnectionFailed {
            backend: MemoryBackend::Sqlite,
            kind: MemoryErrorKind::InternalError {
                backend_error: format!("Failed to set busy_timeout: {}", e),
            },
        })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeout_config_defaults() {
        let config = TimeoutConfig::default();
        assert_eq!(config.statement_timeout, Duration::from_secs(30));
        assert_eq!(config.transaction_timeout, Duration::from_secs(60));
        assert_eq!(config.migration_timeout, Duration::from_secs(300));
    }

    #[test]
    fn test_timeout_config_production() {
        let config = TimeoutConfig::production();
        assert!(config.statement_timeout < TimeoutConfig::default().statement_timeout);
        assert!(config.migration_timeout < TimeoutConfig::default().migration_timeout);
    }

    #[test]
    fn test_with_timeout_success() {
        let result = with_timeout(Duration::from_secs(1), "test_operation", || {
            Ok::<i32, MemoryError>(42)
        });
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_with_timeout_timeout() {
        let result = with_timeout(Duration::from_millis(100), "slow_operation", || {
            thread::sleep(Duration::from_secs(2));
            Ok::<(), MemoryError>(())
        });

        assert!(result.is_err());
        match result {
            Err(MemoryError::ConnectionFailed { kind, .. }) => {
                assert!(matches!(kind, MemoryErrorKind::Timeout { .. }));
            }
            _ => panic!("Expected timeout error"),
        }
    }
}
