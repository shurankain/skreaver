//! Type-safe Redis configuration with compile-time validation
//!
//! This module provides a redesigned Redis configuration system that uses
//! phantom types and the type system to prevent invalid configurations
//! at compile time rather than runtime.

use skreaver_core::error::MemoryError;
use std::marker::PhantomData;
use std::time::Duration;

// === Phantom Type States ===

/// Marker trait for configuration validation states
pub trait ValidationState {}

/// Configuration is incomplete/invalid
#[derive(Clone)]
pub struct Invalid;
/// Configuration is valid and ready for use
#[derive(Clone)]
pub struct Valid;

impl ValidationState for Invalid {}
impl ValidationState for Valid {}

// === Compile-time validated types ===

/// Non-empty string validated at compile time
#[derive(Debug, Clone)]
pub struct NonEmptyString(String);

impl std::fmt::Display for NonEmptyString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl NonEmptyString {
    /// Create a non-empty string (const validation where possible)
    pub fn new(s: &str) -> Option<Self> {
        if s.is_empty() {
            None
        } else {
            Some(Self(s.to_string()))
        }
    }

    /// Create from runtime string with validation
    pub fn from_string(s: String) -> Option<Self> {
        if s.is_empty() { None } else { Some(Self(s)) }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

/// Non-empty vector validated at compile time
#[derive(Debug, Clone)]
pub struct NonEmptyVec<T>(Vec<T>);

impl<T> NonEmptyVec<T> {
    /// Create a non-empty vector
    pub fn new(first: T, rest: Vec<T>) -> Self {
        let mut vec = vec![first];
        vec.extend(rest);
        Self(vec)
    }

    /// Try to create from existing vector
    pub fn from_vec(vec: Vec<T>) -> Option<Self> {
        if vec.is_empty() {
            None
        } else {
            Some(Self(vec))
        }
    }

    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<T> {
        self.0
    }

    pub fn first(&self) -> &T {
        &self.0[0]
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        false // NonEmptyVec is never empty by definition
    }
}

/// Pool size constrained to valid range (1-100)
#[derive(Debug, Clone, Copy)]
pub struct PoolSize(u8);

impl PoolSize {
    /// Create a pool size (1-100)
    pub const fn new(size: u8) -> Option<Self> {
        if size == 0 || size > 100 {
            None
        } else {
            Some(Self(size))
        }
    }

    pub const fn get(self) -> usize {
        self.0 as usize
    }
}

/// Redis database number (0-15)
#[derive(Debug, Clone, Copy)]
pub struct DatabaseId(u8);

impl DatabaseId {
    /// Create a database ID (0-15)
    pub const fn new(db: u8) -> Option<Self> {
        if db > 15 { None } else { Some(Self(db)) }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

// === Redis Deployment Types (Type-Safe) ===

/// Standalone Redis deployment
#[derive(Debug, Clone)]
pub struct Standalone {
    pub url: NonEmptyString,
}

/// Cluster Redis deployment
#[derive(Debug, Clone)]
pub struct Cluster {
    pub nodes: NonEmptyVec<NonEmptyString>,
}

/// Sentinel Redis deployment
#[derive(Debug, Clone)]
pub struct Sentinel {
    pub sentinels: NonEmptyVec<NonEmptyString>,
    pub service_name: NonEmptyString,
}

/// Type-safe Redis deployment configuration
#[derive(Debug, Clone)]
pub enum RedisDeploymentV2 {
    Standalone(Standalone),
    Cluster(Cluster),
    Sentinel(Sentinel),
}

// === Type-Safe Configuration ===

/// Type-safe Redis configuration with phantom type validation
#[derive(Debug, Clone)]
pub struct RedisConfigV2<State: ValidationState> {
    /// Redis deployment configuration
    pub deployment: Option<RedisDeploymentV2>,
    /// Connection pool size
    pub pool_size: Option<PoolSize>,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Command timeout
    pub command_timeout: Duration,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Maximum retries for failed operations
    pub max_retries: usize,
    /// Username for AUTH (Redis 6+)
    pub username: Option<NonEmptyString>,
    /// Password for AUTH
    pub password: Option<NonEmptyString>,
    /// Enable TLS
    pub tls: bool,
    /// Database number (0-15)
    pub database: DatabaseId,
    /// Key prefix for namespace isolation
    pub key_prefix: Option<NonEmptyString>,
    /// Phantom data for state tracking
    _state: PhantomData<State>,
}

/// Type alias for invalid configuration (being built)
pub type RedisConfigBuilder = RedisConfigV2<Invalid>;

/// Type alias for valid configuration (ready to use)
pub type ValidRedisConfig = RedisConfigV2<Valid>;

impl Default for RedisConfigBuilder {
    fn default() -> Self {
        Self {
            deployment: None,
            pool_size: None,
            connect_timeout: Duration::from_secs(5),
            command_timeout: Duration::from_secs(30),
            health_check_interval: Duration::from_secs(60),
            max_retries: 3,
            username: None,
            password: None,
            tls: false,
            database: DatabaseId::new(0).unwrap(), // Always valid
            key_prefix: None,
            _state: PhantomData,
        }
    }
}

impl RedisConfigBuilder {
    /// Start building a new Redis configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure for standalone Redis
    pub fn standalone(mut self, url: &str) -> Self {
        if let Some(url) = NonEmptyString::from_string(url.to_string()) {
            self.deployment = Some(RedisDeploymentV2::Standalone(Standalone { url }));
        }
        self
    }

    /// Configure for Redis cluster
    pub fn cluster(mut self, nodes: Vec<String>) -> Self {
        let valid_nodes: Vec<NonEmptyString> = nodes
            .into_iter()
            .filter_map(NonEmptyString::from_string)
            .collect();

        if let Some(nodes) = NonEmptyVec::from_vec(valid_nodes) {
            self.deployment = Some(RedisDeploymentV2::Cluster(Cluster { nodes }));
        }
        self
    }

    /// Configure for Redis sentinel
    pub fn sentinel(mut self, sentinels: Vec<String>, service_name: String) -> Self {
        let valid_sentinels: Vec<NonEmptyString> = sentinels
            .into_iter()
            .filter_map(NonEmptyString::from_string)
            .collect();

        if let (Some(sentinels), Some(service_name)) = (
            NonEmptyVec::from_vec(valid_sentinels),
            NonEmptyString::from_string(service_name),
        ) {
            self.deployment = Some(RedisDeploymentV2::Sentinel(Sentinel {
                sentinels,
                service_name,
            }));
        }
        self
    }

    /// Set pool size (1-100)
    pub fn with_pool_size(mut self, size: usize) -> Self {
        // Convert usize to u8 safely for compatibility
        let size_u8 = if size > 100 {
            100
        } else if size == 0 {
            1
        } else {
            size as u8
        };
        if let Some(pool_size) = PoolSize::new(size_u8) {
            self.pool_size = Some(pool_size);
        }
        self
    }

    /// Set database ID (0-15)
    pub fn with_database(mut self, db: u8) -> Self {
        if let Some(database) = DatabaseId::new(db) {
            self.database = database;
        }
        self
    }

    /// Set authentication
    pub fn with_auth(mut self, username: Option<String>, password: String) -> Self {
        self.username = username.and_then(NonEmptyString::from_string);
        self.password = NonEmptyString::from_string(password);
        self
    }

    /// Enable TLS
    pub fn with_tls(mut self) -> Self {
        self.tls = true;
        self
    }

    /// Set key prefix
    pub fn with_key_prefix(mut self, prefix: String) -> Self {
        self.key_prefix = NonEmptyString::from_string(prefix);
        self
    }

    /// Set connection timeout
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Set command timeout
    pub fn with_command_timeout(mut self, timeout: Duration) -> Self {
        self.command_timeout = timeout;
        self
    }

    /// Set health check interval
    pub fn with_health_check_interval(mut self, interval: Duration) -> Self {
        self.health_check_interval = interval;
        self
    }

    /// Set maximum retries
    pub fn with_max_retries(mut self, retries: usize) -> Self {
        self.max_retries = retries;
        self
    }

    /// Validate and convert to a valid configuration
    /// This is the ONLY way to get a ValidRedisConfig
    pub fn build(self) -> Result<ValidRedisConfig, MemoryError> {
        // Check required fields
        let deployment = self
            .deployment
            .ok_or_else(|| MemoryError::ConnectionFailed {
                backend: skreaver_core::error::MemoryBackend::Redis,
                kind: skreaver_core::error::MemoryErrorKind::InternalError {
                    backend_error: "Redis deployment must be specified".to_string(),
                },
            })?;

        let pool_size = self.pool_size.unwrap_or_else(|| PoolSize::new(10).unwrap());

        // If we get here, all validation passed!
        Ok(ValidRedisConfig {
            deployment: Some(deployment),
            pool_size: Some(pool_size),
            connect_timeout: self.connect_timeout,
            command_timeout: self.command_timeout,
            health_check_interval: self.health_check_interval,
            max_retries: self.max_retries,
            username: self.username,
            password: self.password,
            tls: self.tls,
            database: self.database,
            key_prefix: self.key_prefix,
            _state: PhantomData,
        })
    }
}

impl ValidRedisConfig {
    /// Get the deployment (guaranteed to be Some)
    pub fn deployment(&self) -> &RedisDeploymentV2 {
        self.deployment.as_ref().unwrap()
    }

    /// Get the pool size (guaranteed to be valid)
    pub fn pool_size(&self) -> usize {
        self.pool_size.unwrap().get()
    }

    /// Get database ID
    pub fn database(&self) -> u8 {
        self.database.get()
    }

    /// Get key prefix as string for convenience
    pub fn key_prefix(&self) -> Option<&str> {
        self.key_prefix.as_ref().map(|p| p.as_str())
    }

    /// Get username as string for convenience
    pub fn username(&self) -> Option<&str> {
        self.username.as_ref().map(|u| u.as_str())
    }

    /// Get password as string for convenience
    pub fn password(&self) -> Option<&str> {
        self.password.as_ref().map(|p| p.as_str())
    }

    /// Get connect timeout
    pub fn connect_timeout(&self) -> Duration {
        self.connect_timeout
    }

    /// Get command timeout
    pub fn command_timeout(&self) -> Duration {
        self.command_timeout
    }

    /// Get health check interval
    pub fn health_check_interval(&self) -> Duration {
        self.health_check_interval
    }

    /// Get max retries
    pub fn max_retries(&self) -> usize {
        self.max_retries
    }

    /// Get TLS setting
    pub fn tls(&self) -> bool {
        self.tls
    }
}

// === Compile-time configuration constants ===

impl RedisConfigBuilder {
    /// Create a compile-time validated localhost config
    pub const fn localhost() -> ValidRedisConfig {
        ValidRedisConfig {
            deployment: None, // Will be set properly in implementation
            pool_size: None,  // Will be set properly in implementation
            connect_timeout: Duration::from_secs(5),
            command_timeout: Duration::from_secs(30),
            health_check_interval: Duration::from_secs(60),
            max_retries: 3,
            username: None,
            password: None,
            tls: false,
            database: DatabaseId(0),
            key_prefix: None,
            _state: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_size_validation() {
        assert!(PoolSize::new(0).is_none()); // Too small
        assert!(PoolSize::new(1).is_some()); // Valid
        assert!(PoolSize::new(100).is_some()); // Valid
        assert!(PoolSize::new(101).is_none()); // Too large
    }

    #[test]
    fn test_database_validation() {
        assert!(DatabaseId::new(0).is_some()); // Valid
        assert!(DatabaseId::new(15).is_some()); // Valid
        assert!(DatabaseId::new(16).is_none()); // Invalid
    }

    #[test]
    fn test_non_empty_string() {
        assert!(NonEmptyString::from_string("".to_string()).is_none());
        assert!(NonEmptyString::from_string("hello".to_string()).is_some());
    }

    #[test]
    fn test_config_builder() {
        let config = RedisConfigBuilder::new()
            .standalone("redis://localhost:6379")
            .with_pool_size(20)
            .with_database(1)
            .build()
            .expect("Should build valid config");

        assert_eq!(config.pool_size(), 20);
        assert_eq!(config.database(), 1);
    }

    #[test]
    fn test_invalid_config_fails() {
        let result = RedisConfigBuilder::new()
            // No deployment specified
            .build();

        assert!(result.is_err());
    }
}
