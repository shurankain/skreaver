//! HTTP runtime configuration
//!
//! This module provides configuration structures for the HTTP runtime,
//! including rate limiting, backpressure, connection limits, and observability settings.

use crate::runtime::{
    backpressure::BackpressureConfig,
    rate_limit::RateLimitConfig,
};
use skreaver_observability::ObservabilityConfig;
use std::path::PathBuf;

/// HTTP runtime configuration
#[derive(Debug, Clone)]
pub struct HttpRuntimeConfig {
    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,
    /// Backpressure and queue management configuration
    pub backpressure: BackpressureConfig,
    /// Connection limits configuration
    pub connection_limits: crate::runtime::connection_limits::ConnectionLimitConfig,
    /// Request timeout in seconds
    pub request_timeout_secs: u64,
    /// Maximum request body size in bytes
    pub max_body_size: usize,
    /// Enable CORS for cross-origin requests
    pub enable_cors: bool,
    /// Enable OpenAPI documentation endpoint
    pub enable_openapi: bool,
    /// Observability configuration
    pub observability: ObservabilityConfig,
    /// Path to security configuration file (skreaver-security.toml)
    /// If None, uses default security configuration
    pub security_config_path: Option<PathBuf>,
}

impl Default for HttpRuntimeConfig {
    fn default() -> Self {
        Self {
            rate_limit: RateLimitConfig::default(),
            backpressure: BackpressureConfig::default(),
            connection_limits: crate::runtime::connection_limits::ConnectionLimitConfig::default(),
            request_timeout_secs: 30,
            max_body_size: 16 * 1024 * 1024, // 16MB
            enable_cors: true,
            enable_openapi: true,
            observability: ObservabilityConfig::default(),
            security_config_path: None, // Use default config
        }
    }
}
