//! Shared database configuration types with validation
//!
//! This module provides validated types for database configuration that can be
//! reused across different database backends (Redis, PostgreSQL, SQLite, etc.).
//!
//! # Design Philosophy
//!
//! - **Parse-don't-validate**: Types enforce invariants at construction time
//! - **Const validation**: Where possible, validation is done at compile time
//! - **Single source of truth**: No duplicate validation logic across backends
//!
//! # Example
//!
//! ```rust
//! use skreaver_core::database::PoolSize;
//!
//! // Valid pool size
//! let pool = PoolSize::new(20).expect("20 is valid");
//! assert_eq!(pool.get(), 20);
//!
//! // Invalid pool sizes
//! assert!(PoolSize::new(0).is_none()); // Too small
//! assert!(PoolSize::new(101).is_none()); // Too large
//! ```

pub mod health;

use serde::{Deserialize, Serialize};

/// Connection pool size constrained to valid range (1-100)
///
/// Pool sizes are limited to prevent resource exhaustion. The range 1-100 is
/// a reasonable default for most applications:
/// - Minimum of 1 ensures at least one connection is available
/// - Maximum of 100 prevents excessive resource consumption
///
/// # Examples
///
/// ```rust
/// use skreaver_core::database::PoolSize;
///
/// // Create valid pool sizes
/// let small = PoolSize::new(1).unwrap();
/// let medium = PoolSize::new(20).unwrap();
/// let large = PoolSize::new(100).unwrap();
///
/// // Invalid pool sizes return None
/// assert!(PoolSize::new(0).is_none());
/// assert!(PoolSize::new(101).is_none());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "usize", into = "usize")]
pub struct PoolSize(u8);

impl PoolSize {
    /// Minimum allowed pool size
    pub const MIN: u8 = 1;
    /// Maximum allowed pool size
    pub const MAX: u8 = 100;

    /// Create a pool size (1-100)
    ///
    /// Returns `None` if the size is 0 or greater than 100.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use skreaver_core::database::PoolSize;
    ///
    /// assert!(PoolSize::new(1).is_some());
    /// assert!(PoolSize::new(50).is_some());
    /// assert!(PoolSize::new(100).is_some());
    /// assert!(PoolSize::new(0).is_none());
    /// assert!(PoolSize::new(101).is_none());
    /// ```
    pub const fn new(size: u8) -> Option<Self> {
        if size < Self::MIN || size > Self::MAX {
            None
        } else {
            Some(Self(size))
        }
    }

    /// Create a pool size from usize (for compatibility)
    ///
    /// This is useful when working with APIs that use usize for pool sizes.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use skreaver_core::database::PoolSize;
    ///
    /// assert!(PoolSize::from_usize(20).is_some());
    /// assert!(PoolSize::from_usize(0).is_none());
    /// assert!(PoolSize::from_usize(101).is_none());
    /// assert!(PoolSize::from_usize(1000).is_none());
    /// ```
    pub fn from_usize(size: usize) -> Option<Self> {
        if size > Self::MAX as usize || size < Self::MIN as usize {
            None
        } else {
            Some(Self(size as u8))
        }
    }

    /// Get the pool size as a usize
    ///
    /// # Examples
    ///
    /// ```rust
    /// use skreaver_core::database::PoolSize;
    ///
    /// let pool = PoolSize::new(20).unwrap();
    /// assert_eq!(pool.get(), 20);
    /// ```
    pub const fn get(self) -> usize {
        self.0 as usize
    }

    /// Get the pool size as a u8
    pub const fn as_u8(self) -> u8 {
        self.0
    }

    /// Default pool size for production use (10 connections)
    pub const fn default_size() -> Self {
        // Safety: 10 is within the valid range
        Self(10)
    }

    /// Small pool size for development/testing (5 connections)
    pub const fn small() -> Self {
        // Safety: 5 is within the valid range
        Self(5)
    }

    /// Large pool size for high-load scenarios (50 connections)
    pub const fn large() -> Self {
        // Safety: 50 is within the valid range
        Self(50)
    }
}

impl Default for PoolSize {
    fn default() -> Self {
        Self::default_size()
    }
}

impl TryFrom<usize> for PoolSize {
    type Error = PoolSizeError;

    fn try_from(size: usize) -> Result<Self, Self::Error> {
        Self::from_usize(size).ok_or(PoolSizeError::OutOfRange { size })
    }
}

impl From<PoolSize> for usize {
    fn from(pool: PoolSize) -> Self {
        pool.get()
    }
}

impl std::fmt::Display for PoolSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Errors that can occur when creating a PoolSize
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PoolSizeError {
    /// Pool size is out of the valid range (1-100)
    #[error("Pool size {size} is out of range (must be {}-{})", PoolSize::MIN, PoolSize::MAX)]
    OutOfRange { size: usize },
}

/// Database name with validation
///
/// Database names are validated to:
/// - Not be empty
/// - Not exceed maximum length (63 chars for PostgreSQL compatibility)
/// - Contain only alphanumeric characters, underscores, and hyphens
/// - Not start with a hyphen (for PostgreSQL compatibility)
///
/// # Examples
///
/// ```rust
/// use skreaver_core::database::DatabaseName;
///
/// let db = DatabaseName::new("my_database").unwrap();
/// assert_eq!(db.as_str(), "my_database");
///
/// // Invalid names
/// assert!(DatabaseName::new("").is_none());
/// assert!(DatabaseName::new("-invalid").is_none());
/// assert!(DatabaseName::new("has spaces").is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DatabaseName(String);

impl DatabaseName {
    /// Maximum length for database names (PostgreSQL limit)
    pub const MAX_LENGTH: usize = 63;

    /// Create a database name with validation
    ///
    /// # Validation Rules
    ///
    /// - Non-empty
    /// - Maximum 63 characters (PostgreSQL limit)
    /// - Only alphanumeric, underscore, hyphen
    /// - Cannot start with hyphen
    ///
    /// # Examples
    ///
    /// ```rust
    /// use skreaver_core::database::DatabaseName;
    ///
    /// assert!(DatabaseName::new("my_db").is_some());
    /// assert!(DatabaseName::new("db-123").is_some());
    /// assert!(DatabaseName::new("").is_none());
    /// assert!(DatabaseName::new("-invalid").is_none());
    /// ```
    pub fn new(name: impl Into<String>) -> Option<Self> {
        let name = name.into();

        if name.is_empty() || name.len() > Self::MAX_LENGTH {
            return None;
        }

        // Cannot start with hyphen
        if name.starts_with('-') {
            return None;
        }

        // Must contain only alphanumeric, underscore, hyphen
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return None;
        }

        Some(Self(name))
    }

    /// Get the database name as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert into owned String
    pub fn into_string(self) -> String {
        self.0
    }
}

impl TryFrom<String> for DatabaseName {
    type Error = DatabaseNameError;

    fn try_from(name: String) -> Result<Self, Self::Error> {
        Self::new(name.clone()).ok_or(DatabaseNameError::Invalid { name })
    }
}

impl From<DatabaseName> for String {
    fn from(name: DatabaseName) -> Self {
        name.0
    }
}

impl std::fmt::Display for DatabaseName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for DatabaseName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Errors that can occur when creating a DatabaseName
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DatabaseNameError {
    /// Database name is invalid
    #[error("Invalid database name: {name}")]
    Invalid { name: String },
}

/// Host address with validation
///
/// Host addresses can be:
/// - IPv4 addresses (e.g., "192.168.1.1")
/// - IPv6 addresses (e.g., "::1")
/// - Hostnames (e.g., "localhost", "db.example.com")
/// - Unix socket paths (validated separately)
///
/// # Security
///
/// Host addresses are validated to prevent:
/// - Path traversal attacks (`../` sequences)
/// - Empty hostnames
/// - Excessively long hostnames
///
/// # Examples
///
/// ```rust
/// use skreaver_core::database::HostAddress;
///
/// let localhost = HostAddress::new("localhost").unwrap();
/// let ip = HostAddress::new("127.0.0.1").unwrap();
///
/// assert!(HostAddress::new("").is_none());
/// assert!(HostAddress::new("../etc").is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct HostAddress(String);

impl HostAddress {
    /// Maximum length for host addresses
    pub const MAX_LENGTH: usize = 253; // DNS hostname limit

    /// Create a host address with validation
    ///
    /// # Validation Rules
    ///
    /// - Non-empty
    /// - Maximum 253 characters (DNS limit)
    /// - No path traversal sequences
    /// - No consecutive slashes (except for IPv6 or protocol prefixes)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use skreaver_core::database::HostAddress;
    ///
    /// assert!(HostAddress::new("localhost").is_some());
    /// assert!(HostAddress::new("192.168.1.1").is_some());
    /// assert!(HostAddress::new("db.example.com").is_some());
    /// assert!(HostAddress::new("").is_none());
    /// assert!(HostAddress::new("../etc").is_none());
    /// ```
    pub fn new(host: impl Into<String>) -> Option<Self> {
        let host = host.into();

        if host.is_empty() || host.len() > Self::MAX_LENGTH {
            return None;
        }

        // Check for path traversal
        if host.contains("..") {
            return None;
        }

        // Check for suspicious patterns (but allow valid IPv6 and URLs)
        if host.contains("//") && !host.starts_with("::") && !host.contains("://") {
            return None;
        }

        Some(Self(host))
    }

    /// Get the host address as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert into owned String
    pub fn into_string(self) -> String {
        self.0
    }

    /// Create localhost host address
    pub fn localhost() -> Self {
        // Safety: "localhost" is always valid
        Self("localhost".to_string())
    }
}

impl TryFrom<String> for HostAddress {
    type Error = HostAddressError;

    fn try_from(host: String) -> Result<Self, Self::Error> {
        let host_clone = host.clone();
        Self::new(host).ok_or(HostAddressError::Invalid { host: host_clone })
    }
}

impl From<HostAddress> for String {
    fn from(host: HostAddress) -> Self {
        host.0
    }
}

impl std::fmt::Display for HostAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for HostAddress {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Errors that can occur when creating a HostAddress
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HostAddressError {
    /// Host address is invalid
    #[error("Invalid host address: {host}")]
    Invalid { host: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_size_validation() {
        assert!(PoolSize::new(0).is_none());
        assert!(PoolSize::new(1).is_some());
        assert!(PoolSize::new(50).is_some());
        assert!(PoolSize::new(100).is_some());
        assert!(PoolSize::new(101).is_none());
    }

    #[test]
    fn test_pool_size_from_usize() {
        assert!(PoolSize::from_usize(0).is_none());
        assert!(PoolSize::from_usize(20).is_some());
        assert!(PoolSize::from_usize(1000).is_none());
    }

    #[test]
    fn test_pool_size_constants() {
        assert_eq!(PoolSize::default_size().get(), 10);
        assert_eq!(PoolSize::small().get(), 5);
        assert_eq!(PoolSize::large().get(), 50);
    }

    #[test]
    fn test_database_name_validation() {
        assert!(DatabaseName::new("my_database").is_some());
        assert!(DatabaseName::new("db-123").is_some());
        assert!(DatabaseName::new("test_db_01").is_some());

        // Invalid names
        assert!(DatabaseName::new("").is_none());
        assert!(DatabaseName::new("-invalid").is_none());
        assert!(DatabaseName::new("has spaces").is_none());
        assert!(DatabaseName::new("has@symbol").is_none());
        assert!(DatabaseName::new("a".repeat(64)).is_none());
    }

    #[test]
    fn test_host_address_validation() {
        assert!(HostAddress::new("localhost").is_some());
        assert!(HostAddress::new("127.0.0.1").is_some());
        assert!(HostAddress::new("db.example.com").is_some());
        assert!(HostAddress::new("::1").is_some());

        // Invalid addresses
        assert!(HostAddress::new("").is_none());
        assert!(HostAddress::new("../etc").is_none());
        assert!(HostAddress::new("host/../path").is_none());
    }

    #[test]
    fn test_pool_size_serde() {
        let pool = PoolSize::new(20).unwrap();
        let json = serde_json::to_string(&pool).unwrap();
        assert_eq!(json, "20");

        let deserialized: PoolSize = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, pool);
    }

    #[test]
    fn test_database_name_serde() {
        let name = DatabaseName::new("my_db").unwrap();
        let json = serde_json::to_string(&name).unwrap();
        assert_eq!(json, "\"my_db\"");

        let deserialized: DatabaseName = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, name);
    }
}
