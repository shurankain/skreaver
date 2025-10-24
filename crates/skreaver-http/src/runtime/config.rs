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
//! - `SKREAVER_BACKPRESSURE_ENABLE_ADAPTIVE` - Enable adaptive backpressure (default: true)
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
use skreaver_observability::ObservabilityConfig;
use std::{env, path::PathBuf, time::Duration};

/// Error type for configuration loading
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid environment variable '{key}': {message}")]
    InvalidEnvVar { key: String, message: String },

    #[error("Configuration validation failed: {0}")]
    ValidationError(String),
}

/// Builder for `HttpRuntimeConfig` with environment variable support
#[derive(Debug, Clone)]
pub struct HttpRuntimeConfigBuilder {
    rate_limit: RateLimitConfig,
    backpressure: BackpressureConfig,
    connection_limits: ConnectionLimitConfig,
    request_timeout_secs: u64,
    max_body_size: usize,
    enable_cors: bool,
    enable_openapi: bool,
    observability: ObservabilityConfig,
    security_config_path: Option<PathBuf>,
}

impl Default for HttpRuntimeConfigBuilder {
    fn default() -> Self {
        Self {
            rate_limit: RateLimitConfig::default(),
            backpressure: BackpressureConfig::default(),
            connection_limits: ConnectionLimitConfig::default(),
            request_timeout_secs: 30,
            max_body_size: 16 * 1024 * 1024, // 16MB
            enable_cors: true,
            enable_openapi: true,
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
            builder = builder.request_timeout_secs(timeout);
        }
        if let Some(max_size) = get_env_usize("SKREAVER_MAX_BODY_SIZE")? {
            builder = builder.max_body_size(max_size);
        }
        if let Some(cors) = get_env_bool("SKREAVER_ENABLE_CORS")? {
            builder = builder.enable_cors(cors);
        }
        if let Some(openapi) = get_env_bool("SKREAVER_ENABLE_OPENAPI")? {
            builder = builder.enable_openapi(openapi);
        }
        if let Some(path) = get_env_string("SKREAVER_SECURITY_CONFIG_PATH") {
            builder = builder.security_config_path(PathBuf::from(path));
        }

        // Rate Limiting
        let mut rate_limit = RateLimitConfig::default();
        if let Some(rpm) = get_env_u32("SKREAVER_RATE_LIMIT_GLOBAL_RPM")? {
            rate_limit.global_rpm = rpm;
        }
        if let Some(rpm) = get_env_u32("SKREAVER_RATE_LIMIT_PER_IP_RPM")? {
            rate_limit.per_ip_rpm = rpm;
        }
        if let Some(rpm) = get_env_u32("SKREAVER_RATE_LIMIT_PER_USER_RPM")? {
            rate_limit.per_user_rpm = rpm;
        }
        builder = builder.rate_limit(rate_limit);

        // Backpressure
        let mut backpressure = BackpressureConfig::default();
        if let Some(size) = get_env_usize("SKREAVER_BACKPRESSURE_MAX_QUEUE_SIZE")? {
            backpressure.max_queue_size = size;
        }
        if let Some(concurrent) = get_env_usize("SKREAVER_BACKPRESSURE_MAX_CONCURRENT")? {
            backpressure.max_concurrent_requests = concurrent;
        }
        if let Some(global) = get_env_usize("SKREAVER_BACKPRESSURE_GLOBAL_MAX_CONCURRENT")? {
            backpressure.global_max_concurrent = global;
        }
        if let Some(timeout) = get_env_u64("SKREAVER_BACKPRESSURE_QUEUE_TIMEOUT_SECS")? {
            backpressure.queue_timeout = Duration::from_secs(timeout);
        }
        if let Some(timeout) = get_env_u64("SKREAVER_BACKPRESSURE_PROCESSING_TIMEOUT_SECS")? {
            backpressure.processing_timeout = Duration::from_secs(timeout);
        }
        if let Some(adaptive) = get_env_bool("SKREAVER_BACKPRESSURE_ENABLE_ADAPTIVE")? {
            backpressure.enable_adaptive_backpressure = adaptive;
        }
        if let Some(target_ms) = get_env_u64("SKREAVER_BACKPRESSURE_TARGET_PROCESSING_MS")? {
            backpressure.target_processing_time_ms = target_ms;
        }
        if let Some(threshold) = get_env_f64("SKREAVER_BACKPRESSURE_LOAD_THRESHOLD")? {
            backpressure.load_threshold = threshold;
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
            connection_limits.enabled = enabled;
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
                    let ip_str = fallback_ip.strip_prefix("fallback:").unwrap();
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
        if let Some(enable) = get_env_bool("SKREAVER_OBSERVABILITY_ENABLE_METRICS")? {
            observability.metrics_enabled = enable;
        }
        if let Some(enable) = get_env_bool("SKREAVER_OBSERVABILITY_ENABLE_TRACING")? {
            observability.tracing_enabled = enable;
        }
        if let Some(enable) = get_env_bool("SKREAVER_OBSERVABILITY_ENABLE_HEALTH")? {
            observability.health_enabled = enable;
        }
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

    /// Set request timeout in seconds
    #[must_use]
    pub fn request_timeout_secs(mut self, timeout: u64) -> Self {
        self.request_timeout_secs = timeout;
        self
    }

    /// Set maximum request body size in bytes
    #[must_use]
    pub fn max_body_size(mut self, size: usize) -> Self {
        self.max_body_size = size;
        self
    }

    /// Enable or disable CORS
    #[must_use]
    pub fn enable_cors(mut self, enable: bool) -> Self {
        self.enable_cors = enable;
        self
    }

    /// Enable or disable `OpenAPI` documentation
    #[must_use]
    pub fn enable_openapi(mut self, enable: bool) -> Self {
        self.enable_openapi = enable;
        self
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

    /// Validate configuration and build `HttpRuntimeConfig`
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::ValidationError` if the configuration is invalid.
    pub fn build(self) -> Result<HttpRuntimeConfig, ConfigError> {
        // Validate configuration
        self.validate()?;

        Ok(HttpRuntimeConfig {
            rate_limit: self.rate_limit,
            backpressure: self.backpressure,
            connection_limits: self.connection_limits,
            request_timeout_secs: self.request_timeout_secs,
            max_body_size: self.max_body_size,
            enable_cors: self.enable_cors,
            enable_openapi: self.enable_openapi,
            observability: self.observability,
            security_config_path: self.security_config_path,
        })
    }

    /// Validate the configuration
    fn validate(&self) -> Result<(), ConfigError> {
        // Request timeout validation
        if self.request_timeout_secs == 0 {
            return Err(ConfigError::ValidationError(
                "request_timeout_secs must be greater than 0".to_string(),
            ));
        }
        if self.request_timeout_secs > 300 {
            return Err(ConfigError::ValidationError(
                "request_timeout_secs must be <= 300 (5 minutes)".to_string(),
            ));
        }

        // Max body size validation
        if self.max_body_size == 0 {
            return Err(ConfigError::ValidationError(
                "max_body_size must be greater than 0".to_string(),
            ));
        }
        if self.max_body_size > 100 * 1024 * 1024 {
            return Err(ConfigError::ValidationError(
                "max_body_size must be <= 100MB".to_string(),
            ));
        }

        // Rate limit validation
        if self.rate_limit.global_rpm == 0 {
            return Err(ConfigError::ValidationError(
                "rate_limit.global_rpm must be greater than 0".to_string(),
            ));
        }
        if self.rate_limit.per_ip_rpm == 0 {
            return Err(ConfigError::ValidationError(
                "rate_limit.per_ip_rpm must be greater than 0".to_string(),
            ));
        }
        if self.rate_limit.per_user_rpm == 0 {
            return Err(ConfigError::ValidationError(
                "rate_limit.per_user_rpm must be greater than 0".to_string(),
            ));
        }

        // Backpressure validation
        if self.backpressure.max_queue_size == 0 {
            return Err(ConfigError::ValidationError(
                "backpressure.max_queue_size must be greater than 0".to_string(),
            ));
        }
        if self.backpressure.max_concurrent_requests == 0 {
            return Err(ConfigError::ValidationError(
                "backpressure.max_concurrent_requests must be greater than 0".to_string(),
            ));
        }
        if self.backpressure.global_max_concurrent == 0 {
            return Err(ConfigError::ValidationError(
                "backpressure.global_max_concurrent must be greater than 0".to_string(),
            ));
        }
        if self.backpressure.load_threshold < 0.0 || self.backpressure.load_threshold > 1.0 {
            return Err(ConfigError::ValidationError(
                "backpressure.load_threshold must be between 0.0 and 1.0".to_string(),
            ));
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_builder() {
        let config = HttpRuntimeConfigBuilder::new().build().unwrap();
        assert_eq!(config.request_timeout_secs, 30);
        assert_eq!(config.max_body_size, 16 * 1024 * 1024);
        assert!(config.enable_cors);
        assert!(config.enable_openapi);
    }

    #[test]
    fn test_builder_validation_timeout() {
        let result = HttpRuntimeConfigBuilder::new()
            .request_timeout_secs(0)
            .build();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("request_timeout_secs must be greater than 0")
        );
    }

    #[test]
    fn test_builder_validation_timeout_max() {
        let result = HttpRuntimeConfigBuilder::new()
            .request_timeout_secs(301)
            .build();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("request_timeout_secs must be <= 300")
        );
    }

    #[test]
    fn test_builder_validation_max_body_size() {
        let result = HttpRuntimeConfigBuilder::new().max_body_size(0).build();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("max_body_size must be greater than 0")
        );
    }

    #[test]
    fn test_builder_validation_load_threshold() {
        let backpressure = BackpressureConfig {
            load_threshold: 1.5,
            ..Default::default()
        };
        let result = HttpRuntimeConfigBuilder::new()
            .backpressure(backpressure)
            .build();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("load_threshold must be between 0.0 and 1.0")
        );
    }

    #[test]
    fn test_env_bool_parsing() {
        assert_eq!(get_env_bool("NONEXISTENT").unwrap(), None);
    }

    #[test]
    fn test_builder_custom_values() {
        let config = HttpRuntimeConfigBuilder::new()
            .request_timeout_secs(60)
            .max_body_size(32 * 1024 * 1024)
            .enable_cors(false)
            .enable_openapi(false)
            .build()
            .unwrap();

        assert_eq!(config.request_timeout_secs, 60);
        assert_eq!(config.max_body_size, 32 * 1024 * 1024);
        assert!(!config.enable_cors);
        assert!(!config.enable_openapi);
    }
}
