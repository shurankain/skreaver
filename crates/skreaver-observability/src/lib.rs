//! Skreaver Observability Framework
//!
//! Provides comprehensive telemetry, metrics collection, and health monitoring
//! for Skreaver agent infrastructure with strict cardinality controls and
//! production-ready observability patterns.

#[cfg(feature = "metrics")]
pub mod metrics;

#[cfg(feature = "tracing")]
pub mod trace;

#[cfg(feature = "health")]
pub mod health;

#[cfg(feature = "opentelemetry")]
pub mod otel;

pub mod tags;

// Re-export core types for easy access
#[cfg(feature = "metrics")]
pub use metrics::{CoreMetrics, MetricsCollector, MetricsRegistry};

#[cfg(feature = "tracing")]
pub use trace::{SessionTracker, TraceContext};

#[cfg(feature = "health")]
pub use health::{ComponentHealth, HealthChecker, HealthStatus};

pub use tags::{AgentId, CardinalTags, ErrorKind, SessionId, ToolName};

/// Standard latency buckets as defined in development plan
/// Covers microseconds to 10+ seconds with production-focused distribution
pub const LATENCY_BUCKETS: &[f64] = &[
    0.005, // 5ms
    0.01,  // 10ms
    0.02,  // 20ms
    0.05,  // 50ms
    0.1,   // 100ms
    0.2,   // 200ms
    0.5,   // 500ms
    1.0,   // 1s
    2.5,   // 2.5s
    5.0,   // 5s
    10.0,  // 10s
];

/// Observability configuration
#[derive(Debug, Clone)]
pub struct ObservabilityConfig {
    /// Enable metrics collection
    pub metrics_enabled: bool,
    /// Enable distributed tracing
    pub tracing_enabled: bool,
    /// Enable health checks
    pub health_enabled: bool,
    /// OpenTelemetry endpoint (optional)
    pub otel_endpoint: Option<String>,
    /// Metrics namespace prefix
    pub namespace: String,
    /// Log sampling configuration
    pub log_sampling: LogSamplingConfig,
}

/// Log sampling configuration per DEVELOPMENT_PLAN.md
#[derive(Debug, Clone)]
pub struct LogSamplingConfig {
    /// Sample rate for ERROR level (1 = no sampling)
    pub error_sample_rate: u32,
    /// Sample rate for WARN level (1 = no sampling)
    pub warn_sample_rate: u32,
    /// Sample rate for INFO level (100 = 1 in 100)
    pub info_sample_rate: u32,
    /// Sample rate for DEBUG level (1000 = 1 in 1000)
    pub debug_sample_rate: u32,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            metrics_enabled: true,
            tracing_enabled: true,
            health_enabled: true,
            otel_endpoint: std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok(),
            namespace: "skreaver".to_string(),
            log_sampling: LogSamplingConfig::default(),
        }
    }
}

impl Default for LogSamplingConfig {
    /// Default sampling rates per DEVELOPMENT_PLAN.md specification
    fn default() -> Self {
        Self {
            error_sample_rate: 1,    // No sampling for errors
            warn_sample_rate: 1,     // No sampling for warnings
            info_sample_rate: 100,   // Sample 1 in 100 info logs
            debug_sample_rate: 1000, // Sample 1 in 1000 debug logs
        }
    }
}

/// Initialize observability framework
pub fn init_observability(config: ObservabilityConfig) -> Result<(), ObservabilityError> {
    #[cfg(feature = "metrics")]
    if config.metrics_enabled {
        metrics::init_metrics_registry(&config.namespace)?;
    }

    #[cfg(feature = "tracing")]
    if config.tracing_enabled {
        trace::init_tracing(&config)?;
    }

    #[cfg(feature = "health")]
    if config.health_enabled {
        health::init_health_checks()?;
    }

    Ok(())
}

/// Observability framework errors
#[derive(thiserror::Error, Debug)]
pub enum ObservabilityError {
    #[error("Metrics initialization failed: {0}")]
    MetricsInit(String),

    #[error("Tracing initialization failed: {0}")]
    TracingInit(String),

    #[error("Health check initialization failed: {0}")]
    HealthInit(String),

    #[error("OpenTelemetry setup failed: {0}")]
    OpenTelemetryInit(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[cfg(feature = "metrics")]
    #[error("Metrics error: {0}")]
    Metrics(#[from] metrics::MetricsError),
}
