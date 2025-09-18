//! PostgreSQL-based memory backend with ACID compliance, connection pooling, and advanced features
//!
//! This module provides an enterprise-grade PostgreSQL backend with:
//! - Full ACID compliance with proper transaction isolation levels
//! - Advanced connection pooling with health monitoring
//! - Schema migration support with versioning and rollback
//! - JSON support for structured data storage
//! - Comprehensive security and error handling

pub mod config;
pub mod health;
pub mod migrations;
pub mod pool;
pub mod transactions;

// Re-export public types for convenience
pub use config::PostgresConfig;
pub use health::PostgresPoolHealth;
pub use migrations::{PostgresMigration, PostgresMigrationEngine};
pub use pool::{PooledConnection, PostgresPool};
pub use transactions::PostgresTransactionalMemory;