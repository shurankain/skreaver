//! SQLite backend modules
//!
//! This module contains the modular SQLite backend implementation with
//! separate concerns for connection pooling, migrations, backups, and health monitoring.

pub mod backup;
pub mod health;
pub mod migrations;
pub mod pool;

// Re-export commonly used types
pub use health::HealthStatus;
pub use migrations::{Migration, MigrationEngine};
pub use pool::SqlitePool;
