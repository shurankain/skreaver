//! SQLite backend modules
//!
//! This module contains the modular SQLite backend implementation with
//! separate concerns for connection pooling, migrations, backups, and health monitoring.

pub mod backup;
pub mod health;
pub mod migrations;
pub mod pool;

// Re-export commonly used types
pub use backup::{BackupFormat, BackupHandle};
pub use health::HealthStatus;
pub use migrations::{AppliedMigration, Migration, MigrationEngine, MigrationStatus};
pub use pool::{PoolHealth, PooledConnection, SqlitePool};
