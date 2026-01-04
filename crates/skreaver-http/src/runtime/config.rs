//! # Environment-Based Configuration
//!
//! This module provides environment variable-based configuration loading
//! for the HTTP runtime, allowing runtime configuration changes without
//! rebuilds. This is essential for production Kubernetes deployments.
//!
//! ## Environment Variables
//!
//! ### HTTP Runtime Configuration
//! - `SKREAVER_REQUEST_TIMEOUT_SECS` - Request timeout in seconds (default: 30)
//! - `SKREAVER_MAX_BODY_SIZE` - Maximum request body size in bytes (default: 16777216 / 16MB)
//! - `SKREAVER_ENABLE_CORS` - Enable CORS (default: true)
//! - `SKREAVER_ENABLE_OPENAPI` - Enable OpenAPI docs (default: true)
//! - `SKREAVER_SECURITY_CONFIG_PATH` - Path to security configuration file
//!
//! ### Rate Limiting
//! - `SKREAVER_RATE_LIMIT_GLOBAL_RPM` - Global requests per minute (default: 1000)
//! - `SKREAVER_RATE_LIMIT_PER_IP_RPM` - Per-IP requests per minute (default: 60)
//! - `SKREAVER_RATE_LIMIT_PER_USER_RPM` - Per-user requests per minute (default: 120)
//!
//! ### Backpressure
//! - `SKREAVER_BACKPRESSURE_MAX_QUEUE_SIZE` - Max queue size per agent (default: 100)
//! - `SKREAVER_BACKPRESSURE_MAX_CONCURRENT` - Max concurrent requests per agent (default: 10)
//! - `SKREAVER_BACKPRESSURE_GLOBAL_MAX_CONCURRENT` - Global max concurrent requests (default: 500)
//! - `SKREAVER_BACKPRESSURE_QUEUE_TIMEOUT_SECS` - Queue timeout in seconds (default: 30)
//! - `SKREAVER_BACKPRESSURE_PROCESSING_TIMEOUT_SECS` - Processing timeout in seconds (default: 60)
//! - `SKREAVER_BACKPRESSURE_MODE` - Backpressure mode: "static" or "adaptive" (default: adaptive)
//! - `SKREAVER_BACKPRESSURE_ENABLE_ADAPTIVE` - [DEPRECATED] Use SKREAVER_BACKPRESSURE_MODE instead
//! - `SKREAVER_BACKPRESSURE_TARGET_PROCESSING_MS` - Target processing time in ms (default: 1000)
//! - `SKREAVER_BACKPRESSURE_LOAD_THRESHOLD` - Load threshold 0.0-1.0 (default: 0.8)
//!
//! ### Connection Limits
//! - `SKREAVER_CONNECTION_LIMIT_MAX` - Global max concurrent connections (default: 10000)
//! - `SKREAVER_CONNECTION_LIMIT_PER_IP` - Max connections per IP (default: 100)
//! - `SKREAVER_CONNECTION_LIMIT_ENABLED` - Enable connection limits (default: true)
//! - `SKREAVER_CONNECTION_LIMIT_MISSING_BEHAVIOR` - How to handle missing ConnectInfo:
//!   - `reject` - Reject requests without ConnectInfo (default, safest for production)
//!   - `disable_per_ip` - Only enforce global limit when ConnectInfo missing
//!   - `fallback:<IP>` - Use fallback IP (e.g., `fallback:127.0.0.1` for testing)
//!
//! ### Observability
//! - `SKREAVER_OBSERVABILITY_ENABLE_METRICS` - Enable Prometheus metrics (default: true)
//! - `SKREAVER_OBSERVABILITY_ENABLE_TRACING` - Enable OpenTelemetry tracing (default: false)
//! - `SKREAVER_OBSERVABILITY_ENABLE_HEALTH` - Enable health checks (default: true)
//! - `SKREAVER_OBSERVABILITY_OTEL_ENDPOINT` - OTLP endpoint for traces
//! - `SKREAVER_OBSERVABILITY_NAMESPACE` - Metrics namespace prefix (default: "skreaver")

use crate::runtime::{
    HttpRuntimeConfig, backpressure::BackpressureConfig, connection_limits::ConnectionLimitConfig,
    rate_limit::RateLimitConfig,
};
use skreaver_observability::{ObservabilityConfig, ObservabilityMode};
use std::{env, num::NonZeroU64, path::PathBuf, time::Duration};

/// Error type for configuration loading
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid environment variable '{key}': {message}")]
    InvalidEnvVar { key: String, message: String },

    #[error("Configuration validation failed: {0}")]
    ValidationError(String),
}

/// Validated request timeout (1-300 seconds)
///
/// This newtype ensures that request timeouts are always valid at construction time,
/// eliminating the need for runtime validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RequestTimeout(NonZeroU64);

impl RequestTimeout {
    /// Minimum allowed timeout in seconds
    pub const MIN_SECONDS: u64 = 1;
    /// Maximum allowed timeout in seconds (5 minutes)
    pub const MAX_SECONDS: u64 = 300;

    /// Create a new RequestTimeout from seconds
    ///
    /// # Errors
    ///
    /// Returns `ConfigError` if the timeout is 0 or greater than 300 seconds.
    pub fn from_seconds(seconds: u64) -> Result<Self, ConfigError> {
        if seconds == 0 {
            return Err(ConfigError::ValidationError(
                "request timeout must be at least 1 second".to_string(),
            ));
        }
        if seconds > Self::MAX_SECONDS {
            return Err(ConfigError::ValidationError(format!(
                "request timeout must be at most {} seconds (5 minutes)",
                Self::MAX_SECONDS
            )));
        }
        // SAFETY: We've checked that seconds is non-zero above
        Ok(Self(NonZeroU64::new(seconds).unwrap()))
    }

    /// Get the timeout value in seconds
    #[must_use]
    pub fn seconds(&self) -> u64 {
        self.0.get()
    }

    /// Convert to `Duration`
    #[must_use]
    pub fn as_duration(&self) -> Duration {
        Duration::from_secs(self.0.get())
    }
}

impl Default for RequestTimeout {
    fn default() -> Self {
        // SAFETY: 30 is always valid (1 <= 30 <= 300)
        Self(NonZeroU64::new(30).unwrap())
    }
}

impl std::fmt::Display for RequestTimeout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}s", self.seconds())
    }
}

/// Validated maximum body size (1 byte to 100MB)
///
/// This newtype ensures that body size limits are always valid at construction time,
/// eliminating the need for runtime validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MaxBodySize(usize);

impl MaxBodySize {
    /// Maximum allowed body size (100MB)
    pub const MAX_BYTES: usize = 100 * 1024 * 1024;

    /// Create a new MaxBodySize from bytes
    ///
    /// # Errors
    ///
    /// Returns `ConfigError` if the size is 0 or greater than 100MB.
    pub fn from_bytes(bytes: usize) -> Result<Self, ConfigError> {
        if bytes == 0 {
            return Err(ConfigError::ValidationError(
                "max body size must be at least 1 byte".to_string(),
            ));
        }
        if bytes > Self::MAX_BYTES {
            return Err(ConfigError::ValidationError(format!(
                "max body size must be at most {} bytes (100MB)",
                Self::MAX_BYTES
            )));
        }
        Ok(Self(bytes))
    }

    /// Create a new MaxBodySize from megabytes
    ///
    /// # Errors
    ///
    /// Returns `ConfigError` if the resulting size is 0 or greater than 100MB.
    pub fn from_megabytes(mb: usize) -> Result<Self, ConfigError> {
        Self::from_bytes(mb * 1024 * 1024)
    }

    /// Get the size value in bytes
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.0
    }
}

impl Default for MaxBodySize {
    fn default() -> Self {
        // SAFETY: 16MB is always valid (1 <= 16MB <= 100MB)
        Self(16 * 1024 * 1024)
    }
}

impl std::fmt::Display for MaxBodySize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mb = self.0 as f64 / (1024.0 * 1024.0);
        write!(f, "{:.1}MB", mb)
    }
}

/// Builder for `HttpRuntimeConfig` with environment variable support
#[derive(Debug, Clone)]
pub struct HttpRuntimeConfigBuilder {
    rate_limit: RateLimitConfig,
    backpressure: BackpressureConfig,
    connection_limits: ConnectionLimitConfig,
    request_timeout: RequestTimeout,
    max_body_size: MaxBodySize,
    cors: Option<crate::runtime::http::CorsConfig>,
    openapi: Option<crate::runtime::http::OpenApiConfig>,
    observability: ObservabilityConfig,
    security_config_path: Option<PathBuf>,
}

impl Default for HttpRuntimeConfigBuilder {
    fn default() -> Self {
        Self {
            rate_limit: RateLimitConfig::default(),
            backpressure: BackpressureConfig::default(),
            connection_limits: ConnectionLimitConfig::default(),
            request_timeout: RequestTimeout::default(),
            max_body_size: MaxBodySize::default(),
            cors: Some(crate::runtime::http::CorsConfig::default()),
            openapi: Some(crate::runtime::http::OpenApiConfig::default()),
            observability: ObservabilityConfig::default(),
            security_config_path: None,
        }
    }
}

impl HttpRuntimeConfigBuilder {
    /// Create a new builder with default values
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Load configuration from environment variables
    ///
    /// # Errors
    ///
    /// Returns `ConfigError` if any environment variable has an invalid value
    /// or if the configuration fails validation.
    pub fn from_env() -> Result<Self, ConfigError> {
        let mut builder = Self::default();

        // HTTP Runtime Configuration
        if let Some(timeout) = get_env_u64("SKREAVER_REQUEST_TIMEOUT_SECS")? {
            builder = builder.request_timeout_secs(timeout)?;
        }
        if let Some(max_size) = get_env_usize("SKREAVER_MAX_BODY_SIZE")? {
            builder = builder.max_body_size(max_size)?;
        }
        if let Some(cors) = get_env_bool("SKREAVER_ENABLE_CORS")? {
            builder = builder.cors(if cors {
                Some(crate::runtime::http::CorsConfig::default())
            } else {
                None
            });
        }
        if let Some(openapi) = get_env_bool("SKREAVER_ENABLE_OPENAPI")? {
            builder = builder.openapi(if openapi {
                Some(crate::runtime::http::OpenApiConfig::default())
            } else {
                None
            });
        }
        if let Some(path) = get_env_string("SKREAVER_SECURITY_CONFIG_PATH") {
            builder = builder.security_config_path(PathBuf::from(path));
        }

        // Rate Limiting
        let mut rate_limit = RateLimitConfig::default();
        if let Some(rpm) = get_env_u32("SKREAVER_RATE_LIMIT_GLOBAL_RPM")?
            && let Some(non_zero) = std::num::NonZeroU32::new(rpm)
        {
            rate_limit.global_rpm = non_zero;
        }
        if let Some(rpm) = get_env_u32("SKREAVER_RATE_LIMIT_PER_IP_RPM")?
            && let Some(non_zero) = std::num::NonZeroU32::new(rpm)
        {
            rate_limit.per_ip_rpm = non_zero;
        }
        if let Some(rpm) = get_env_u32("SKREAVER_RATE_LIMIT_PER_USER_RPM")?
            && let Some(non_zero) = std::num::NonZeroU32::new(rpm)
        {
            rate_limit.per_user_rpm = non_zero;
        }
        builder = builder.rate_limit(rate_limit);

        // Backpressure
        let mut backpressure = BackpressureConfig::default();
        if let Some(size) = get_env_usize("SKREAVER_BACKPRESSURE_MAX_QUEUE_SIZE")? {
            backpressure.max_queue_size = crate::runtime::backpressure::QueueSize::new(size)?;
        }
        if let Some(concurrent) = get_env_usize("SKREAVER_BACKPRESSURE_MAX_CONCURRENT")? {
            backpressure.max_concurrent_requests =
                crate::runtime::backpressure::ConcurrencyLimit::new(concurrent)?;
        }
        if let Some(global) = get_env_usize("SKREAVER_BACKPRESSURE_GLOBAL_MAX_CONCURRENT")? {
            backpressure.global_max_concurrent =
                crate::runtime::backpressure::ConcurrencyLimit::new(global)?;
        }
        if let Some(timeout) = get_env_u64("SKREAVER_BACKPRESSURE_QUEUE_TIMEOUT_SECS")? {
            backpressure.queue_timeout = Duration::from_secs(timeout);
        }
        if let Some(timeout) = get_env_u64("SKREAVER_BACKPRESSURE_PROCESSING_TIMEOUT_SECS")? {
            backpressure.processing_timeout = Duration::from_secs(timeout);
        }

        // Parse backpressure mode directly from string (eliminates boolean blindness)
        // Supports both new format ("static"/"adaptive") and legacy boolean format for backward compatibility
        if let Some(mode) = get_env_parsed("SKREAVER_BACKPRESSURE_MODE")? {
            backpressure.mode = mode;
        } else if let Some(adaptive) = get_env_bool("SKREAVER_BACKPRESSURE_ENABLE_ADAPTIVE")? {
            // Legacy boolean format for backward compatibility (deprecated)
            backpressure.mode = if adaptive {
                crate::runtime::backpressure::BackpressureMode::Adaptive
            } else {
                crate::runtime::backpressure::BackpressureMode::Static
            };
        }

        if let Some(target_ms) = get_env_u64("SKREAVER_BACKPRESSURE_TARGET_PROCESSING_MS")? {
            backpressure.target_processing_time_ms = target_ms;
        }
        if let Some(threshold) = get_env_f64("SKREAVER_BACKPRESSURE_LOAD_THRESHOLD")? {
            backpressure.load_threshold =
                crate::runtime::backpressure::LoadThreshold::new(threshold)?;
        }
        builder = builder.backpressure(backpressure);

        // Connection Limits
        let mut connection_limits = ConnectionLimitConfig::default();
        if let Some(max) = get_env_usize("SKREAVER_CONNECTION_LIMIT_MAX")? {
            connection_limits.max_connections = max;
        }
        if let Some(max_per_ip) = get_env_usize("SKREAVER_CONNECTION_LIMIT_PER_IP")? {
            connection_limits.max_connections_per_ip = max_per_ip;
        }
        if let Some(enabled) = get_env_bool("SKREAVER_CONNECTION_LIMIT_ENABLED")? {
            connection_limits.mode = if enabled {
                crate::runtime::connection_limits::ConnectionLimitMode::Enabled
            } else {
                crate::runtime::connection_limits::ConnectionLimitMode::Disabled
            };
        }

        // Handle missing ConnectInfo behavior
        if let Ok(behavior_str) = env::var("SKREAVER_CONNECTION_LIMIT_MISSING_BEHAVIOR") {
            use crate::runtime::connection_limits::MissingConnectInfoBehavior;
            connection_limits.missing_connect_info_behavior = match behavior_str
                .to_lowercase()
                .as_str()
            {
                "reject" => MissingConnectInfoBehavior::Reject,
                "disable_per_ip" => MissingConnectInfoBehavior::DisablePerIpLimits,
                fallback_ip if fallback_ip.starts_with("fallback:") => {
                    let ip_str = &fallback_ip["fallback:".len()..];
                    let ip = ip_str.parse().map_err(|e| ConfigError::InvalidEnvVar {
                        key: "SKREAVER_CONNECTION_LIMIT_MISSING_BEHAVIOR".to_string(),
                        message: format!("Invalid IP address '{}': {}", ip_str, e),
                    })?;
                    MissingConnectInfoBehavior::UseFallback(ip)
                }
                _ => {
                    return Err(ConfigError::InvalidEnvVar {
                        key: "SKREAVER_CONNECTION_LIMIT_MISSING_BEHAVIOR".to_string(),
                        message: format!(
                            "Invalid value '{}'. Must be 'reject', 'disable_per_ip', or 'fallback:<IP>'",
                            behavior_str
                        ),
                    });
                }
            };
        }

        builder = builder.connection_limits(connection_limits);

        // Observability
        let mut observability = ObservabilityConfig::default();

        // Read individual feature flags from environment
        let metrics = get_env_bool("SKREAVER_OBSERVABILITY_ENABLE_METRICS")?.unwrap_or(true);
        let tracing = get_env_bool("SKREAVER_OBSERVABILITY_ENABLE_TRACING")?.unwrap_or(true);
        let health = get_env_bool("SKREAVER_OBSERVABILITY_ENABLE_HEALTH")?.unwrap_or(true);

        // Determine observability mode from feature flags
        observability.mode = match (metrics, tracing, health) {
            (true, true, true) => ObservabilityMode::Full,
            (true, false, true) => ObservabilityMode::MetricsOnly,
            (false, false, true) => ObservabilityMode::HealthOnly,
            (false, false, false) => ObservabilityMode::Disabled,
            // If tracing is enabled, metrics should also be enabled (tracing depends on metrics)
            (false, true, _) => {
                tracing::warn!("Tracing enabled without metrics - enabling Full mode");
                ObservabilityMode::Full
            }
            // Other combinations default to what's requested, but may not be ideal
            (true, false, false) | (true, true, false) => {
                tracing::warn!("Unusual observability configuration - health checks disabled");
                ObservabilityMode::MetricsOnly
            }
        };

        if let Some(endpoint) = get_env_string("SKREAVER_OBSERVABILITY_OTEL_ENDPOINT") {
            observability.otel_endpoint = Some(endpoint);
        }
        if let Some(namespace) = get_env_string("SKREAVER_OBSERVABILITY_NAMESPACE") {
            observability.namespace = namespace;
        }
        builder = builder.observability(observability);

        Ok(builder)
    }

    /// Set rate limiting configuration
    #[must_use]
    pub fn rate_limit(mut self, rate_limit: RateLimitConfig) -> Self {
        self.rate_limit = rate_limit;
        self
    }

    /// Set backpressure configuration
    #[must_use]
    pub fn backpressure(mut self, backpressure: BackpressureConfig) -> Self {
        self.backpressure = backpressure;
        self
    }

    /// Set connection limits configuration
    #[must_use]
    pub fn connection_limits(mut self, connection_limits: ConnectionLimitConfig) -> Self {
        self.connection_limits = connection_limits;
        self
    }

    /// Set request timeout using validated type
    #[must_use]
    pub fn request_timeout(mut self, timeout: RequestTimeout) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Set request timeout in seconds (convenience method with validation)
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::ValidationError` if the timeout is invalid (must be 1-300 seconds).
    pub fn request_timeout_secs(mut self, timeout: u64) -> Result<Self, ConfigError> {
        self.request_timeout = RequestTimeout::from_seconds(timeout)?;
        Ok(self)
    }

    /// Set maximum body size using validated type
    #[must_use]
    pub fn max_body_size_validated(mut self, size: MaxBodySize) -> Self {
        self.max_body_size = size;
        self
    }

    /// Set maximum request body size in bytes (convenience method with validation)
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::ValidationError` if the size is invalid (must be 1 byte - 100MB).
    pub fn max_body_size(mut self, size: usize) -> Result<Self, ConfigError> {
        self.max_body_size = MaxBodySize::from_bytes(size)?;
        Ok(self)
    }

    /// Set maximum request body size in megabytes (convenience method with validation)
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::ValidationError` if the size is invalid (must be 1 byte - 100MB).
    pub fn max_body_size_mb(mut self, size_mb: usize) -> Result<Self, ConfigError> {
        self.max_body_size = MaxBodySize::from_megabytes(size_mb)?;
        Ok(self)
    }

    /// Set CORS configuration (None = disabled, Some = enabled)
    #[must_use]
    pub fn cors(mut self, cors: Option<crate::runtime::http::CorsConfig>) -> Self {
        self.cors = cors;
        self
    }

    /// Set OpenAPI configuration (None = disabled, Some = enabled)
    #[must_use]
    pub fn openapi(mut self, openapi: Option<crate::runtime::http::OpenApiConfig>) -> Self {
        self.openapi = openapi;
        self
    }

    /// Enable or disable CORS (backward compatibility)
    #[must_use]
    #[deprecated(
        since = "0.5.1",
        note = "Use `cors()` method with Option pattern instead"
    )]
    pub fn enable_cors(self, enable: bool) -> Self {
        self.cors(if enable {
            Some(crate::runtime::http::CorsConfig::default())
        } else {
            None
        })
    }

    /// Enable or disable OpenAPI documentation (backward compatibility)
    #[must_use]
    #[deprecated(
        since = "0.5.1",
        note = "Use `openapi()` method with Option pattern instead"
    )]
    pub fn enable_openapi(self, enable: bool) -> Self {
        self.openapi(if enable {
            Some(crate::runtime::http::OpenApiConfig::default())
        } else {
            None
        })
    }

    /// Set observability configuration
    #[must_use]
    pub fn observability(mut self, observability: ObservabilityConfig) -> Self {
        self.observability = observability;
        self
    }

    /// Set security configuration file path
    #[must_use]
    pub fn security_config_path(mut self, path: PathBuf) -> Self {
        self.security_config_path = Some(path);
        self
    }

    /// Build `HttpRuntimeConfig`
    ///
    /// This method is infallible because all validated values use newtypes
    /// that enforce constraints at construction time.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::ValidationError` if the backpressure or connection
    /// limit configuration is invalid (these still use runtime validation).
    pub fn build(self) -> Result<HttpRuntimeConfig, ConfigError> {
        // Validate remaining configuration (backpressure, connection limits)
        self.validate()?;

        Ok(HttpRuntimeConfig {
            rate_limit: self.rate_limit,
            backpressure: self.backpressure,
            connection_limits: self.connection_limits,
            request_timeout: self.request_timeout,
            max_body_size: self.max_body_size,
            cors: self.cors,
            openapi: self.openapi,
            observability: self.observability,
            security_config_path: self.security_config_path,
        })
    }

    /// Validate the configuration
    ///
    /// Only validates fields that don't use validated newtypes yet.
    /// RequestTimeout and MaxBodySize are validated at construction time.
    fn validate(&self) -> Result<(), ConfigError> {
        // Request timeout validation - ELIMINATED
        // Now validated at construction time via RequestTimeout newtype

        // Max body size validation - ELIMINATED
        // Now validated at construction time via MaxBodySize newtype

        // Rate limit validation
        // No validation needed - NonZeroU32 guarantees non-zero values at compile time

        // Backpressure validation - ELIMINATED
        // Now validated at construction time via QueueSize, ConcurrencyLimit, and LoadThreshold newtypes

        // Observability validation
        if self.observability.namespace.is_empty() {
            return Err(ConfigError::ValidationError(
                "observability.namespace cannot be empty".to_string(),
            ));
        }

        Ok(())
    }
}

// Environment variable helper functions

fn get_env_string(key: &str) -> Option<String> {
    env::var(key).ok()
}

fn get_env_bool(key: &str) -> Result<Option<bool>, ConfigError> {
    match env::var(key) {
        Ok(val) => match val.to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Ok(Some(true)),
            "false" | "0" | "no" | "off" => Ok(Some(false)),
            _ => Err(ConfigError::InvalidEnvVar {
                key: key.to_string(),
                message: format!(
                    "invalid boolean value '{val}', expected true/false/1/0/yes/no/on/off"
                ),
            }),
        },
        Err(_) => Ok(None),
    }
}

fn get_env_u64(key: &str) -> Result<Option<u64>, ConfigError> {
    match env::var(key) {
        Ok(val) => val
            .parse::<u64>()
            .map(Some)
            .map_err(|e| ConfigError::InvalidEnvVar {
                key: key.to_string(),
                message: format!("invalid u64 value '{val}': {e}"),
            }),
        Err(_) => Ok(None),
    }
}

fn get_env_u32(key: &str) -> Result<Option<u32>, ConfigError> {
    match env::var(key) {
        Ok(val) => val
            .parse::<u32>()
            .map(Some)
            .map_err(|e| ConfigError::InvalidEnvVar {
                key: key.to_string(),
                message: format!("invalid u32 value '{val}': {e}"),
            }),
        Err(_) => Ok(None),
    }
}

fn get_env_usize(key: &str) -> Result<Option<usize>, ConfigError> {
    match env::var(key) {
        Ok(val) => val
            .parse::<usize>()
            .map(Some)
            .map_err(|e| ConfigError::InvalidEnvVar {
                key: key.to_string(),
                message: format!("invalid usize value '{val}': {e}"),
            }),
        Err(_) => Ok(None),
    }
}

fn get_env_f64(key: &str) -> Result<Option<f64>, ConfigError> {
    match env::var(key) {
        Ok(val) => val
            .parse::<f64>()
            .map(Some)
            .map_err(|e| ConfigError::InvalidEnvVar {
                key: key.to_string(),
                message: format!("invalid f64 value '{val}': {e}"),
            }),
        Err(_) => Ok(None),
    }
}

/// Generic helper to parse environment variables using FromStr
fn get_env_parsed<T>(key: &str) -> Result<Option<T>, ConfigError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match env::var(key) {
        Ok(val) => val
            .parse::<T>()
            .map(Some)
            .map_err(|e| ConfigError::InvalidEnvVar {
                key: key.to_string(),
                message: format!("{}", e),
            }),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_builder() {
        let config = HttpRuntimeConfigBuilder::new().build().unwrap();
        assert_eq!(config.request_timeout.seconds(), 30);
        assert_eq!(config.max_body_size.bytes(), 16 * 1024 * 1024);
        assert!(config.cors.is_some());
        assert!(config.openapi.is_some());
    }

    #[test]
    fn test_builder_validation_timeout() {
        // Validation now happens at builder method level, not build() time
        let result = HttpRuntimeConfigBuilder::new().request_timeout_secs(0);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("request timeout must be at least 1 second")
        );
    }

    #[test]
    fn test_builder_validation_timeout_max() {
        let result = HttpRuntimeConfigBuilder::new().request_timeout_secs(301);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("request timeout must be at most 300 seconds")
        );
    }

    #[test]
    fn test_builder_validation_max_body_size() {
        let result = HttpRuntimeConfigBuilder::new().max_body_size(0);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("max body size must be at least 1 byte")
        );
    }

    #[test]
    fn test_load_threshold_validation() {
        // LoadThreshold construction should fail for invalid values
        let result = crate::runtime::backpressure::LoadThreshold::new(1.5);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("0.0 and 1.0"));

        // Valid load thresholds should succeed
        assert!(crate::runtime::backpressure::LoadThreshold::new(0.0).is_ok());
        assert!(crate::runtime::backpressure::LoadThreshold::new(0.8).is_ok());
        assert!(crate::runtime::backpressure::LoadThreshold::new(1.0).is_ok());

        // Negative values should fail
        let result = crate::runtime::backpressure::LoadThreshold::new(-0.1);
        assert!(result.is_err());
    }

    #[test]
    fn test_env_bool_parsing() {
        assert_eq!(get_env_bool("NONEXISTENT").unwrap(), None);
    }

    #[test]
    fn test_builder_custom_values() {
        let config = HttpRuntimeConfigBuilder::new()
            .request_timeout_secs(60)
            .unwrap()
            .max_body_size(32 * 1024 * 1024)
            .unwrap()
            .cors(None)
            .openapi(None)
            .build()
            .unwrap();

        assert_eq!(config.request_timeout.seconds(), 60);
        assert_eq!(config.max_body_size.bytes(), 32 * 1024 * 1024);
        assert!(config.cors.is_none());
        assert!(config.openapi.is_none());
    }

    #[test]
    fn test_builder_deprecated_methods() {
        #[allow(deprecated)]
        let config = HttpRuntimeConfigBuilder::new()
            .enable_cors(false)
            .enable_openapi(false)
            .build()
            .unwrap();

        assert!(config.cors.is_none());
        assert!(config.openapi.is_none());
    }
}
