//! Type-safe metadata framework
//!
//! Provides compile-time safe metadata handling to replace stringly-typed
//! HashMap<String, String> usage throughout the codebase.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MetadataValue {
    /// String value
    String(String),
    /// Integer value
    Integer(i64),
    /// Unsigned integer value
    UnsignedInteger(u64),
    /// Floating point value
    Float(f64),
    /// Boolean value
    Boolean(bool),
    /// Timestamp value (ISO 8601)
    Timestamp(String),
}

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
            inner: HashMap::with_capacity(capacity),
        }
    }

    /// Insert a metadata entry
    pub fn insert<V: Into<MetadataValue>>(
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
    pub fn with_string(mut self, key: MetadataKey, value: impl Into<String>) -> Self {
        self.metadata
            .insert(key, MetadataValue::String(value.into()));
        self
    }

    /// Add an integer value
    pub fn with_i64(mut self, key: MetadataKey, value: i64) -> Self {
        self.metadata.insert(key, MetadataValue::Integer(value));
        self
    }

    /// Add an unsigned integer value
    pub fn with_u64(mut self, key: MetadataKey, value: u64) -> Self {
        self.metadata
            .insert(key, MetadataValue::UnsignedInteger(value));
        self
    }

    /// Add a float value
    pub fn with_f64(mut self, key: MetadataKey, value: f64) -> Self {
        self.metadata.insert(key, MetadataValue::Float(value));
        self
    }

    /// Add a boolean value
    pub fn with_bool(mut self, key: MetadataKey, value: bool) -> Self {
        self.metadata.insert(key, MetadataValue::Boolean(value));
        self
    }

    /// Add a timestamp value
    pub fn with_timestamp(mut self, key: MetadataKey, value: impl Into<String>) -> Self {
        self.metadata
            .insert(key, MetadataValue::Timestamp(value.into()));
        self
    }

    /// Add a generic value
    pub fn with<V: Into<MetadataValue>>(mut self, key: MetadataKey, value: V) -> Self {
        self.metadata.insert(key, value.into());
        self
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

        metadata.insert(MetadataKey::BuildVersion, "1.0.0");
        metadata.insert(MetadataKey::UptimeSeconds, 3600u64);

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
            .with_u64(MetadataKey::UptimeSeconds, 3600)
            .with_bool(MetadataKey::Custom("enabled".to_string()), true)
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
}
