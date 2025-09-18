//! Redis-based memory backend with clustering, connection pooling, and advanced features
//!
//! This module provides an enterprise-grade Redis backend with:
//! - Support for standalone, cluster, and sentinel Redis deployments
//! - Advanced connection pooling with health monitoring
//! - Enhanced transactions with proper MULTI/EXEC
//! - Comprehensive security and error handling
//! - Runtime utilities for sync/async bridge
//! - Performance monitoring and metrics

pub mod config;
pub mod health;
pub mod pool;
pub mod runtime;
pub mod transactions;

// Re-export public types for convenience
pub use config::RedisConfig;
pub use health::{ConnectionMetrics, PoolStats, RedisHealth};
pub use pool::RedisPoolUtils;
pub use runtime::{REDIS_RUNTIME, with_redis_runtime};
pub use transactions::{ConfigProvider, RedisConnectionProvider, RedisTransactionExecutor};
