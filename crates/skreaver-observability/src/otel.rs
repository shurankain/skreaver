//! OpenTelemetry Integration
//!
//! Provides OpenTelemetry export capabilities for Skreaver observability
//! with OTLP endpoint support for external monitoring systems.

use crate::ObservabilityError;

/// OpenTelemetry configuration
#[derive(Debug, Clone)]
pub struct OtelConfig {
    /// OTLP endpoint URL
    pub endpoint: String,
    /// Service name for telemetry
    pub service_name: String,
    /// Service version
    pub service_version: String,
    /// Additional resource attributes
    pub resource_attributes: Vec<(String, String)>,
}

impl OtelConfig {
    /// Create new OpenTelemetry configuration
    pub fn new(endpoint: String, service_name: String) -> Self {
        Self {
            endpoint,
            service_name: service_name.clone(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            resource_attributes: vec![
                ("service.name".to_string(), service_name),
                (
                    "service.version".to_string(),
                    env!("CARGO_PKG_VERSION").to_string(),
                ),
            ],
        }
    }

    /// Add resource attribute
    pub fn with_attribute(mut self, key: String, value: String) -> Self {
        self.resource_attributes.push((key, value));
        self
    }
}

/// Initialize OpenTelemetry exporter
#[cfg(feature = "opentelemetry")]
pub fn init_otel_exporter(config: &OtelConfig) -> Result<(), ObservabilityError> {
    use opentelemetry::KeyValue;
    use opentelemetry_otlp::WithExportConfig;
    use tracing_opentelemetry::OpenTelemetryLayer;
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    // Create resource with service attributes
    let resource = opentelemetry::Resource::new(
        config
            .resource_attributes
            .iter()
            .map(|(k, v)| KeyValue::new(k.clone(), v.clone()))
            .collect::<Vec<_>>(),
    );

    // Initialize tracer provider
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(&config.endpoint),
        )
        .with_trace_config(opentelemetry::trace::Config::default().with_resource(resource.clone()))
        .install_batch(opentelemetry::runtime::Tokio)
        .map_err(|e| {
            ObservabilityError::OpenTelemetryInit(format!("Failed to initialize tracer: {}", e))
        })?;

    // Set up tracing subscriber with OpenTelemetry layer
    let telemetry_layer = OpenTelemetryLayer::new(tracer);

    tracing_subscriber::registry().with(telemetry_layer).init();

    tracing::info!(
        endpoint = config.endpoint,
        service = config.service_name,
        version = config.service_version,
        "OpenTelemetry exporter initialized"
    );

    Ok(())
}

#[cfg(not(feature = "opentelemetry"))]
pub fn init_otel_exporter(_config: &OtelConfig) -> Result<(), ObservabilityError> {
    Err(ObservabilityError::OpenTelemetryInit(
        "OpenTelemetry feature not enabled".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_otel_config_creation() {
        let config = OtelConfig::new(
            "http://localhost:4317".to_string(),
            "skreaver-test".to_string(),
        );

        assert_eq!(config.endpoint, "http://localhost:4317");
        assert_eq!(config.service_name, "skreaver-test");
        assert!(!config.resource_attributes.is_empty());
    }

    #[test]
    fn test_otel_config_with_attributes() {
        let config = OtelConfig::new(
            "http://localhost:4317".to_string(),
            "skreaver-test".to_string(),
        )
        .with_attribute("deployment.environment".to_string(), "test".to_string());

        assert!(
            config
                .resource_attributes
                .iter()
                .any(|(k, v)| k == "deployment.environment" && v == "test")
        );
    }
}
