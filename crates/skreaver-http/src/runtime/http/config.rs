//! HTTP runtime configuration
//!
//! This module provides configuration structures for the HTTP runtime,
//! including rate limiting, backpressure, connection limits, and observability settings.

use crate::runtime::{backpressure::BackpressureConfig, rate_limit::RateLimitConfig};
use skreaver_observability::ObservabilityConfig;
use std::path::PathBuf;

/// CORS policy configuration
///
/// Use `Option<CorsConfig>` to enable/disable CORS:
/// - `None` = CORS disabled
/// - `Some(config)` = CORS enabled with given policy
#[derive(Debug, Clone)]
pub enum CorsPolicy {
    /// Allow all origins (development/permissive mode)
    Permissive {
        methods: Vec<String>,
        headers: Vec<String>,
    },
    /// Restrict to specific origins (production mode)
    Restrictive {
        origins: Vec<String>,
        methods: Vec<String>,
        headers: Vec<String>,
    },
}

impl CorsPolicy {
    /// Create permissive policy (allows all origins)
    pub fn permissive() -> Self {
        Self::Permissive {
            methods: vec![
                "GET".into(),
                "POST".into(),
                "PUT".into(),
                "DELETE".into(),
                "OPTIONS".into(),
            ],
            headers: vec!["*".into()],
        }
    }

    /// Create restrictive policy with specific origins
    pub fn restrictive(origins: Vec<String>) -> Self {
        Self::Restrictive {
            origins,
            methods: vec!["GET".into(), "POST".into()],
            headers: vec!["content-type".into(), "authorization".into()],
        }
    }

    /// Create custom restrictive policy with specific origins, methods, and headers
    pub fn custom(origins: Vec<String>, methods: Vec<String>, headers: Vec<String>) -> Self {
        Self::Restrictive {
            origins,
            methods,
            headers,
        }
    }

    /// Check if policy is permissive
    pub fn is_permissive(&self) -> bool {
        matches!(self, Self::Permissive { .. })
    }

    /// Get allowed origins (None for permissive mode)
    pub fn allowed_origins(&self) -> Option<&[String]> {
        match self {
            Self::Permissive { .. } => None,
            Self::Restrictive { origins, .. } => Some(origins),
        }
    }

    /// Get allowed methods
    pub fn allowed_methods(&self) -> &[String] {
        match self {
            Self::Permissive { methods, .. } => methods,
            Self::Restrictive { methods, .. } => methods,
        }
    }

    /// Get allowed headers
    pub fn allowed_headers(&self) -> &[String] {
        match self {
            Self::Permissive { headers, .. } => headers,
            Self::Restrictive { headers, .. } => headers,
        }
    }
}

impl Default for CorsPolicy {
    fn default() -> Self {
        Self::permissive()
    }
}

/// Type alias for backward compatibility
pub type CorsConfig = CorsPolicy;

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
            cors: Some(CorsPolicy::restrictive(allowed_origins)),
            openapi: Some(OpenApiConfig::default()),
            ..Default::default()
        }
    }

    /// Create configuration for development (permissive CORS, OpenAPI enabled)
    pub fn development() -> Self {
        Self {
            cors: Some(CorsPolicy::permissive()),
            openapi: Some(OpenApiConfig::default()),
            ..Default::default()
        }
    }
}
