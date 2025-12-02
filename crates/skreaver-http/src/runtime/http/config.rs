//! HTTP runtime configuration
//!
//! This module provides configuration structures for the HTTP runtime,
//! including rate limiting, backpressure, connection limits, and observability settings.

use crate::runtime::{backpressure::BackpressureConfig, rate_limit::RateLimitConfig};
use skreaver_observability::ObservabilityConfig;
use std::path::PathBuf;

/// CORS configuration
///
/// Use `Option<CorsConfig>` to enable/disable CORS:
/// - `None` = CORS disabled
/// - `Some(config)` = CORS enabled with given configuration
#[derive(Debug, Clone)]
pub struct CorsConfig {
    /// Allow all origins (permissive mode)
    pub permissive: bool,
    /// Allowed origins (if not permissive)
    pub allowed_origins: Vec<String>,
    /// Allowed methods
    pub allowed_methods: Vec<String>,
    /// Allowed headers
    pub allowed_headers: Vec<String>,
}

impl CorsConfig {
    /// Create a permissive CORS configuration (allows all origins)
    pub fn permissive() -> Self {
        Self {
            permissive: true,
            allowed_origins: vec![],
            allowed_methods: vec![
                "GET".into(),
                "POST".into(),
                "PUT".into(),
                "DELETE".into(),
                "OPTIONS".into(),
            ],
            allowed_headers: vec!["*".into()],
        }
    }

    /// Create a restrictive CORS configuration with specific origins
    pub fn restrictive(origins: Vec<String>) -> Self {
        Self {
            permissive: false,
            allowed_origins: origins,
            allowed_methods: vec!["GET".into(), "POST".into()],
            allowed_headers: vec!["content-type".into(), "authorization".into()],
        }
    }

    /// Check if CORS is in permissive mode
    pub fn is_permissive(&self) -> bool {
        self.permissive
    }
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self::permissive()
    }
}

/// OpenAPI documentation configuration
///
/// Use `Option<OpenApiConfig>` to enable/disable OpenAPI:
/// - `None` = OpenAPI disabled
/// - `Some(config)` = OpenAPI enabled with given configuration
#[derive(Debug, Clone)]
pub struct OpenApiConfig {
    /// Documentation endpoint path
    pub docs_path: String,
    /// OpenAPI spec endpoint path
    pub spec_path: String,
}

impl OpenApiConfig {
    /// Create OpenAPI configuration with default paths
    pub fn new() -> Self {
        Self::default()
    }

    /// Create OpenAPI configuration with custom paths
    pub fn with_paths(docs_path: impl Into<String>, spec_path: impl Into<String>) -> Self {
        Self {
            docs_path: docs_path.into(),
            spec_path: spec_path.into(),
        }
    }
}

impl Default for OpenApiConfig {
    fn default() -> Self {
        Self {
            docs_path: "/docs".to_string(),
            spec_path: "/api-docs/openapi.json".to_string(),
        }
    }
}

/// HTTP runtime configuration
///
/// Uses Option pattern for optional features (CORS, OpenAPI) to eliminate
/// boolean blindness and provide better configuration control.
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
    /// CORS configuration (None = disabled, Some = enabled)
    pub cors: Option<CorsConfig>,
    /// OpenAPI documentation configuration (None = disabled, Some = enabled)
    pub openapi: Option<OpenApiConfig>,
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
            cors: Some(CorsConfig::default()),
            openapi: Some(OpenApiConfig::default()),
            observability: ObservabilityConfig::default(),
            security_config_path: None, // Use default config
        }
    }
}

impl HttpRuntimeConfig {
    /// Create configuration with CORS and OpenAPI disabled
    pub fn minimal() -> Self {
        Self {
            cors: None,
            openapi: None,
            ..Default::default()
        }
    }

    /// Create configuration for production (restrictive CORS, OpenAPI enabled)
    pub fn production(allowed_origins: Vec<String>) -> Self {
        Self {
            cors: Some(CorsConfig::restrictive(allowed_origins)),
            openapi: Some(OpenApiConfig::default()),
            ..Default::default()
        }
    }

    /// Create configuration for development (permissive CORS, OpenAPI enabled)
    pub fn development() -> Self {
        Self {
            cors: Some(CorsConfig::permissive()),
            openapi: Some(OpenApiConfig::default()),
            ..Default::default()
        }
    }
}
