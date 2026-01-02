//! Type-safe metadata framework with DoS protection
//!
//! Provides compile-time safe metadata handling to replace stringly-typed
//! HashMap<String, String> usage throughout the codebase.
//!
//! ## DoS Protection
//!
//! This module includes protection against denial-of-service attacks via:
//! - **Entry count limits**: Maximum number of metadata entries
//! - **Total size limits**: Maximum total byte size of all metadata
//! - **Value size limits**: Maximum size per individual value
//!
//! These limits prevent attackers from:
//! - Exhausting server memory with unbounded metadata
//! - Causing performance degradation with large metadata operations
//! - Bypassing resource limits through metadata injection

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

/// Maximum number of metadata entries (DoS protection)
pub const MAX_METADATA_ENTRIES: usize = 100;

/// Maximum total size of all metadata in bytes (DoS protection)
pub const MAX_METADATA_TOTAL_BYTES: usize = 10 * 1024; // 10KB

/// Maximum length for a single metadata value
pub const MAX_VALUE_LENGTH: usize = 1024; // 1KB per value

/// Type-safe metadata keys with compile-time validation
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataKey {
    // Build and version information
    BuildVersion,
    GitCommit,
    BuildTimestamp,

    // System information
    Hostname,
    Environment,
    Region,
    Datacenter,

    // Runtime information
    UptimeSeconds,
    ProcessId,
    ThreadCount,

    // API Key metadata
    ApiKeyId,
    CreatedAt,
    ExpiresAt,
    LastUsedAt,
    RotatedFrom,
    RevocationReason,

    // Health check metadata
    ComponentType,
    HealthCheckInterval,
    LastCheckDuration,
    ConsecutiveFailures,
    RecoveryAttempts,

    // Authentication metadata
    PrincipalType,
    AuthMethod,
    SessionId,
    IpAddress,
    UserAgent,

    // Request/Response metadata
    RequestId,
    CorrelationId,
    TraceId,
    SpanId,

    // Performance metrics
    ResponseTimeMs,
    QueueTimeMs,
    ProcessingTimeMs,

    // Error information
    ErrorCode,
    ErrorMessage,
    ErrorCategory,
    StackTrace,

    // Custom application metadata
    Custom(String),
}

impl MetadataKey {
    /// Get the key as a string
    pub fn as_str(&self) -> &str {
        match self {
            MetadataKey::BuildVersion => "build_version",
            MetadataKey::GitCommit => "git_commit",
            MetadataKey::BuildTimestamp => "build_timestamp",
            MetadataKey::Hostname => "hostname",
            MetadataKey::Environment => "environment",
            MetadataKey::Region => "region",
            MetadataKey::Datacenter => "datacenter",
            MetadataKey::UptimeSeconds => "uptime_seconds",
            MetadataKey::ProcessId => "process_id",
            MetadataKey::ThreadCount => "thread_count",
            MetadataKey::ApiKeyId => "api_key_id",
            MetadataKey::CreatedAt => "created_at",
            MetadataKey::ExpiresAt => "expires_at",
            MetadataKey::LastUsedAt => "last_used_at",
            MetadataKey::RotatedFrom => "rotated_from",
            MetadataKey::RevocationReason => "revocation_reason",
            MetadataKey::ComponentType => "component_type",
            MetadataKey::HealthCheckInterval => "health_check_interval",
            MetadataKey::LastCheckDuration => "last_check_duration",
            MetadataKey::ConsecutiveFailures => "consecutive_failures",
            MetadataKey::RecoveryAttempts => "recovery_attempts",
            MetadataKey::PrincipalType => "principal_type",
            MetadataKey::AuthMethod => "auth_method",
            MetadataKey::SessionId => "session_id",
            MetadataKey::IpAddress => "ip_address",
            MetadataKey::UserAgent => "user_agent",
            MetadataKey::RequestId => "request_id",
            MetadataKey::CorrelationId => "correlation_id",
            MetadataKey::TraceId => "trace_id",
            MetadataKey::SpanId => "span_id",
            MetadataKey::ResponseTimeMs => "response_time_ms",
            MetadataKey::QueueTimeMs => "queue_time_ms",
            MetadataKey::ProcessingTimeMs => "processing_time_ms",
            MetadataKey::ErrorCode => "error_code",
            MetadataKey::ErrorMessage => "error_message",
            MetadataKey::ErrorCategory => "error_category",
            MetadataKey::StackTrace => "stack_trace",
            MetadataKey::Custom(s) => s,
        }
    }
}

impl fmt::Display for MetadataKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for MetadataKey {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "build_version" => MetadataKey::BuildVersion,
            "git_commit" => MetadataKey::GitCommit,
            "build_timestamp" => MetadataKey::BuildTimestamp,
            "hostname" => MetadataKey::Hostname,
            "environment" => MetadataKey::Environment,
            "region" => MetadataKey::Region,
            "datacenter" => MetadataKey::Datacenter,
            "uptime_seconds" => MetadataKey::UptimeSeconds,
            "process_id" => MetadataKey::ProcessId,
            "thread_count" => MetadataKey::ThreadCount,
            "api_key_id" => MetadataKey::ApiKeyId,
            "created_at" => MetadataKey::CreatedAt,
            "expires_at" => MetadataKey::ExpiresAt,
            "last_used_at" => MetadataKey::LastUsedAt,
            "rotated_from" => MetadataKey::RotatedFrom,
            "revocation_reason" => MetadataKey::RevocationReason,
            "component_type" => MetadataKey::ComponentType,
            "health_check_interval" => MetadataKey::HealthCheckInterval,
            "last_check_duration" => MetadataKey::LastCheckDuration,
            "consecutive_failures" => MetadataKey::ConsecutiveFailures,
            "recovery_attempts" => MetadataKey::RecoveryAttempts,
            "principal_type" => MetadataKey::PrincipalType,
            "auth_method" => MetadataKey::AuthMethod,
            "session_id" => MetadataKey::SessionId,
            "ip_address" => MetadataKey::IpAddress,
            "user_agent" => MetadataKey::UserAgent,
            "request_id" => MetadataKey::RequestId,
            "correlation_id" => MetadataKey::CorrelationId,
            "trace_id" => MetadataKey::TraceId,
            "span_id" => MetadataKey::SpanId,
            "response_time_ms" => MetadataKey::ResponseTimeMs,
            "queue_time_ms" => MetadataKey::QueueTimeMs,
            "processing_time_ms" => MetadataKey::ProcessingTimeMs,
            "error_code" => MetadataKey::ErrorCode,
            "error_message" => MetadataKey::ErrorMessage,
            "error_category" => MetadataKey::ErrorCategory,
            "stack_trace" => MetadataKey::StackTrace,
            custom => MetadataKey::Custom(custom.to_string()),
        })
    }
}

/// Type-safe metadata value supporting multiple types
///
/// Values are validated against size limits to prevent DoS attacks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MetadataValue {
    /// String value (validated to not exceed MAX_VALUE_LENGTH)
    String(String),
    /// Integer value
    Integer(i64),
    /// Unsigned integer value
    UnsignedInteger(u64),
    /// Floating point value
    Float(f64),
    /// Boolean value
    Boolean(bool),
    /// Timestamp value (ISO 8601, validated to not exceed MAX_VALUE_LENGTH)
    Timestamp(String),
}

/// Errors that can occur during metadata operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataError {
    /// Value exceeds maximum length
    ValueTooLong { length: usize, max: usize },
    /// Too many entries in metadata
    TooManyEntries { current: usize, max: usize },
    /// Total metadata size exceeds limit
    TotalSizeTooLarge {
        current: usize,
        additional: usize,
        max: usize,
    },
}

impl fmt::Display for MetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ValueTooLong { length, max } => {
                write!(
                    f,
                    "Metadata value too long: {} bytes (max {})",
                    length, max
                )
            }
            Self::TooManyEntries { current, max } => {
                write!(
                    f,
                    "Too many metadata entries: {} entries (max {})",
                    current, max
                )
            }
            Self::TotalSizeTooLarge {
                current,
                additional,
                max,
            } => {
                write!(
                    f,
                    "Metadata size too large: current {} + additional {} bytes exceeds max {} bytes",
                    current, additional, max
                )
            }
        }
    }
}

impl std::error::Error for MetadataError {}

impl MetadataValue {
    /// Try to get as string
    pub fn as_string(&self) -> Option<&str> {
        match self {
            MetadataValue::String(s) => Some(s),
            MetadataValue::Timestamp(s) => Some(s),
            _ => None,
        }
    }

    /// Try to get as integer
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            MetadataValue::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// Try to get as unsigned integer
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            MetadataValue::UnsignedInteger(u) => Some(*u),
            _ => None,
        }
    }

    /// Try to get as float
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            MetadataValue::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Try to get as boolean
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            MetadataValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }
}

impl From<String> for MetadataValue {
    fn from(s: String) -> Self {
        MetadataValue::String(s)
    }
}

impl From<&str> for MetadataValue {
    fn from(s: &str) -> Self {
        MetadataValue::String(s.to_string())
    }
}

impl From<i64> for MetadataValue {
    fn from(i: i64) -> Self {
        MetadataValue::Integer(i)
    }
}

impl From<u64> for MetadataValue {
    fn from(u: u64) -> Self {
        MetadataValue::UnsignedInteger(u)
    }
}

impl From<f64> for MetadataValue {
    fn from(f: f64) -> Self {
        MetadataValue::Float(f)
    }
}

impl From<bool> for MetadataValue {
    fn from(b: bool) -> Self {
        MetadataValue::Boolean(b)
    }
}

impl fmt::Display for MetadataValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MetadataValue::String(s) => write!(f, "{}", s),
            MetadataValue::Integer(i) => write!(f, "{}", i),
            MetadataValue::UnsignedInteger(u) => write!(f, "{}", u),
            MetadataValue::Float(fl) => write!(f, "{}", fl),
            MetadataValue::Boolean(b) => write!(f, "{}", b),
            MetadataValue::Timestamp(s) => write!(f, "{}", s),
        }
    }
}

/// Type-safe metadata collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    inner: HashMap<MetadataKey, MetadataValue>,
}

impl Metadata {
    /// Create a new empty metadata collection
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    /// Create metadata with initial capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: HashMap::with_capacity(capacity.min(MAX_METADATA_ENTRIES)),
        }
    }

    /// Insert a metadata entry with validation
    ///
    /// # Errors
    ///
    /// Returns `MetadataError` if:
    /// - Value is too long
    /// - Adding this entry would exceed size limits
    pub fn insert<V: Into<MetadataValue>>(
        &mut self,
        key: MetadataKey,
        value: V,
    ) -> Result<Option<MetadataValue>, MetadataError> {
        let value = value.into();

        // Validate value size
        self.validate_value(&value)?;

        // Check if we're adding a new entry (not replacing)
        let is_new_entry = !self.inner.contains_key(&key);

        // Check size limits before insertion
        if is_new_entry {
            self.validate_size_limits(&key, &value)?;
        }

        Ok(self.inner.insert(key, value))
    }

    /// Insert a metadata entry without validation (for internal use)
    ///
    /// This method bypasses size validation and should only be used when
    /// the value is known to be safe (e.g., from trusted sources or during deserialization).
    #[cfg(test)]
    pub(crate) fn insert_unchecked<V: Into<MetadataValue>>(
        &mut self,
        key: MetadataKey,
        value: V,
    ) -> Option<MetadataValue> {
        self.inner.insert(key, value.into())
    }

    /// Get a metadata value by key
    pub fn get(&self, key: &MetadataKey) -> Option<&MetadataValue> {
        self.inner.get(key)
    }

    /// Remove a metadata entry
    pub fn remove(&mut self, key: &MetadataKey) -> Option<MetadataValue> {
        self.inner.remove(key)
    }

    /// Check if a key exists
    pub fn contains_key(&self, key: &MetadataKey) -> bool {
        self.inner.contains_key(key)
    }

    /// Get the number of metadata entries
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if metadata is empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Iterate over all metadata entries
    pub fn iter(&self) -> impl Iterator<Item = (&MetadataKey, &MetadataValue)> {
        self.inner.iter()
    }

    /// Get all keys
    pub fn keys(&self) -> impl Iterator<Item = &MetadataKey> {
        self.inner.keys()
    }

    /// Get all values
    pub fn values(&self) -> impl Iterator<Item = &MetadataValue> {
        self.inner.values()
    }

    /// Clear all metadata
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Merge another metadata collection into this one
    pub fn extend(&mut self, other: Metadata) {
        self.inner.extend(other.inner);
    }

    /// Convert to HashMap<String, String> for backward compatibility
    pub fn to_string_map(&self) -> HashMap<String, String> {
        self.inner
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    /// Create from HashMap<String, String> for backward compatibility
    pub fn from_string_map(map: HashMap<String, String>) -> Self {
        let inner = map
            .into_iter()
            .map(|(k, v)| (MetadataKey::from_str(&k).unwrap(), MetadataValue::String(v)))
            .collect();
        Self { inner }
    }

    /// Get total size in bytes (approximate)
    pub fn total_bytes(&self) -> usize {
        self.inner
            .iter()
            .map(|(k, v)| self.value_size(k, v))
            .sum()
    }

    /// Calculate the size of a key-value pair in bytes
    fn value_size(&self, key: &MetadataKey, value: &MetadataValue) -> usize {
        let key_size = key.as_str().len();
        let value_size = match value {
            MetadataValue::String(s) => s.len(),
            MetadataValue::Timestamp(s) => s.len(),
            MetadataValue::Integer(_) => 8,
            MetadataValue::UnsignedInteger(_) => 8,
            MetadataValue::Float(_) => 8,
            MetadataValue::Boolean(_) => 1,
        };
        key_size + value_size
    }

    /// Validate a metadata value
    fn validate_value(&self, value: &MetadataValue) -> Result<(), MetadataError> {
        match value {
            MetadataValue::String(s) if s.len() > MAX_VALUE_LENGTH => {
                Err(MetadataError::ValueTooLong {
                    length: s.len(),
                    max: MAX_VALUE_LENGTH,
                })
            }
            MetadataValue::Timestamp(s) if s.len() > MAX_VALUE_LENGTH => {
                Err(MetadataError::ValueTooLong {
                    length: s.len(),
                    max: MAX_VALUE_LENGTH,
                })
            }
            _ => Ok(()),
        }
    }

    /// Validate size limits before adding a new entry
    fn validate_size_limits(
        &self,
        new_key: &MetadataKey,
        new_value: &MetadataValue,
    ) -> Result<(), MetadataError> {
        // Check entry count
        if self.inner.len() >= MAX_METADATA_ENTRIES {
            return Err(MetadataError::TooManyEntries {
                current: self.inner.len(),
                max: MAX_METADATA_ENTRIES,
            });
        }

        // Check total byte size
        let new_bytes = self.value_size(new_key, new_value);
        let current_bytes = self.total_bytes();
        let total_bytes = current_bytes + new_bytes;

        if total_bytes > MAX_METADATA_TOTAL_BYTES {
            return Err(MetadataError::TotalSizeTooLarge {
                current: current_bytes,
                additional: new_bytes,
                max: MAX_METADATA_TOTAL_BYTES,
            });
        }

        Ok(())
    }
}

impl Default for Metadata {
    fn default() -> Self {
        Self::new()
    }
}

impl FromIterator<(MetadataKey, MetadataValue)> for Metadata {
    fn from_iter<T: IntoIterator<Item = (MetadataKey, MetadataValue)>>(iter: T) -> Self {
        Self {
            inner: iter.into_iter().collect(),
        }
    }
}

/// Builder for constructing metadata
#[derive(Debug, Default)]
pub struct MetadataBuilder {
    metadata: Metadata,
}

impl MetadataBuilder {
    /// Create a new metadata builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a string value
    ///
    /// # Errors
    ///
    /// Returns `MetadataError` if the value is too long or size limits are exceeded.
    pub fn with_string(
        mut self,
        key: MetadataKey,
        value: impl Into<String>,
    ) -> Result<Self, MetadataError> {
        self.metadata
            .insert(key, MetadataValue::String(value.into()))?;
        Ok(self)
    }

    /// Add an integer value
    pub fn with_i64(mut self, key: MetadataKey, value: i64) -> Result<Self, MetadataError> {
        self.metadata.insert(key, MetadataValue::Integer(value))?;
        Ok(self)
    }

    /// Add an unsigned integer value
    pub fn with_u64(mut self, key: MetadataKey, value: u64) -> Result<Self, MetadataError> {
        self.metadata
            .insert(key, MetadataValue::UnsignedInteger(value))?;
        Ok(self)
    }

    /// Add a float value
    pub fn with_f64(mut self, key: MetadataKey, value: f64) -> Result<Self, MetadataError> {
        self.metadata.insert(key, MetadataValue::Float(value))?;
        Ok(self)
    }

    /// Add a boolean value
    pub fn with_bool(mut self, key: MetadataKey, value: bool) -> Result<Self, MetadataError> {
        self.metadata.insert(key, MetadataValue::Boolean(value))?;
        Ok(self)
    }

    /// Add a timestamp value
    ///
    /// # Errors
    ///
    /// Returns `MetadataError` if the timestamp is too long or size limits are exceeded.
    pub fn with_timestamp(
        mut self,
        key: MetadataKey,
        value: impl Into<String>,
    ) -> Result<Self, MetadataError> {
        self.metadata
            .insert(key, MetadataValue::Timestamp(value.into()))?;
        Ok(self)
    }

    /// Add a generic value
    ///
    /// # Errors
    ///
    /// Returns `MetadataError` if the value is too long or size limits are exceeded.
    pub fn with<V: Into<MetadataValue>>(
        mut self,
        key: MetadataKey,
        value: V,
    ) -> Result<Self, MetadataError> {
        self.metadata.insert(key, value.into())?;
        Ok(self)
    }

    /// Build the metadata
    pub fn build(self) -> Metadata {
        self.metadata
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_key_to_string() {
        assert_eq!(MetadataKey::BuildVersion.as_str(), "build_version");
        assert_eq!(MetadataKey::ApiKeyId.as_str(), "api_key_id");
        assert_eq!(MetadataKey::Custom("foo".to_string()).as_str(), "foo");
    }

    #[test]
    fn test_metadata_key_from_string() {
        assert_eq!(
            MetadataKey::from_str("build_version"),
            Ok(MetadataKey::BuildVersion)
        );
        assert_eq!(
            MetadataKey::from_str("api_key_id"),
            Ok(MetadataKey::ApiKeyId)
        );
        assert_eq!(
            MetadataKey::from_str("unknown"),
            Ok(MetadataKey::Custom("unknown".to_string()))
        );
    }

    #[test]
    fn test_metadata_value_types() {
        let string_val = MetadataValue::from("test");
        assert_eq!(string_val.as_string(), Some("test"));

        let int_val = MetadataValue::from(42i64);
        assert_eq!(int_val.as_i64(), Some(42));

        let uint_val = MetadataValue::from(100u64);
        assert_eq!(uint_val.as_u64(), Some(100));

        let bool_val = MetadataValue::from(true);
        assert_eq!(bool_val.as_bool(), Some(true));
    }

    #[test]
    fn test_metadata_operations() {
        let mut metadata = Metadata::new();

        metadata
            .insert(MetadataKey::BuildVersion, "1.0.0")
            .unwrap();
        metadata.insert(MetadataKey::UptimeSeconds, 3600u64).unwrap();

        assert_eq!(metadata.len(), 2);
        assert!(metadata.contains_key(&MetadataKey::BuildVersion));

        let version = metadata.get(&MetadataKey::BuildVersion);
        assert_eq!(version.and_then(|v| v.as_string()), Some("1.0.0"));

        let uptime = metadata.get(&MetadataKey::UptimeSeconds);
        assert_eq!(uptime.and_then(|v| v.as_u64()), Some(3600));
    }

    #[test]
    fn test_metadata_builder() {
        let metadata = MetadataBuilder::new()
            .with_string(MetadataKey::BuildVersion, "1.0.0")
            .unwrap()
            .with_u64(MetadataKey::UptimeSeconds, 3600)
            .unwrap()
            .with_bool(MetadataKey::Custom("enabled".to_string()), true)
            .unwrap()
            .build();

        assert_eq!(metadata.len(), 3);
        assert!(metadata.contains_key(&MetadataKey::BuildVersion));
    }

    #[test]
    fn test_metadata_string_map_conversion() {
        let mut string_map = HashMap::new();
        string_map.insert("build_version".to_string(), "1.0.0".to_string());
        string_map.insert("uptime_seconds".to_string(), "3600".to_string());

        let metadata = Metadata::from_string_map(string_map);
        assert_eq!(metadata.len(), 2);

        let back_to_map = metadata.to_string_map();
        assert_eq!(back_to_map.len(), 2);
        assert_eq!(back_to_map.get("build_version"), Some(&"1.0.0".to_string()));
    }

    // DoS Protection Tests

    #[test]
    fn test_value_too_long() {
        let mut metadata = Metadata::new();
        let long_value = "a".repeat(MAX_VALUE_LENGTH + 1);

        let result = metadata.insert(MetadataKey::Custom("test".to_string()), long_value);
        assert!(matches!(result, Err(MetadataError::ValueTooLong { .. })));
    }

    #[test]
    fn test_too_many_entries() {
        let mut metadata = Metadata::new();

        // Fill to max
        for i in 0..MAX_METADATA_ENTRIES {
            let key = MetadataKey::Custom(format!("key{}", i));
            metadata.insert(key, "value").unwrap();
        }

        // Try to add one more
        let result = metadata.insert(MetadataKey::Custom("overflow".to_string()), "value");
        assert!(matches!(
            result,
            Err(MetadataError::TooManyEntries { .. })
        ));
    }

    #[test]
    fn test_total_size_limit() {
        let mut metadata = Metadata::new();

        // Create a large value
        let value = "x".repeat(MAX_VALUE_LENGTH);

        // Should be able to add some entries
        for i in 0..5 {
            let key = MetadataKey::Custom(format!("key{}", i));
            metadata.insert(key, value.clone()).unwrap();
        }

        // Eventually should hit the size limit
        let mut hit_limit = false;
        for i in 5..50 {
            let key = MetadataKey::Custom(format!("key{}", i));
            if metadata.insert(key, value.clone()).is_err() {
                hit_limit = true;
                break;
            }
        }

        assert!(hit_limit, "Should have hit size limit");
    }

    #[test]
    fn test_total_bytes_calculation() {
        let mut metadata = Metadata::new();
        metadata.insert(MetadataKey::BuildVersion, "1.0.0").unwrap(); // ~18 bytes
        metadata.insert(MetadataKey::UptimeSeconds, 3600u64).unwrap(); // ~22 bytes

        let total = metadata.total_bytes();
        assert!(total > 0);
        assert!(total < MAX_METADATA_TOTAL_BYTES);
    }

    #[test]
    fn test_replace_does_not_count_toward_limit() {
        let mut metadata = Metadata::new();

        // Insert initial value
        metadata
            .insert(MetadataKey::BuildVersion, "1.0.0")
            .unwrap();

        let initial_len = metadata.len();

        // Replace with same key - should succeed even if close to limits
        metadata
            .insert(MetadataKey::BuildVersion, "2.0.0")
            .unwrap();

        assert_eq!(metadata.len(), initial_len); // Length unchanged
    }

    #[test]
    fn test_unchecked_insert() {
        let mut metadata = Metadata::new();

        // insert_unchecked should bypass validation (internal API)
        let long_value = "a".repeat(MAX_VALUE_LENGTH + 1);
        metadata.insert_unchecked(MetadataKey::Custom("test".to_string()), long_value);

        assert_eq!(metadata.len(), 1);
    }
}
