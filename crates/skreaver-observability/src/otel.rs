//! OpenTelemetry Integration
//!
//! Provides OpenTelemetry export capabilities for Skreaver observability
//! with OTLP endpoint support for external monitoring systems.

use crate::ObservabilityError;
use std::collections::HashMap;
use std::time::Duration;

/// Well-known OpenTelemetry resource attribute keys
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ResourceKey {
    ServiceName,
    ServiceVersion,
    ServiceNamespace,
    ServiceInstanceId,
    DeploymentEnvironment,
    Custom(String),
}

impl ResourceKey {
    fn as_str(&self) -> &str {
        match self {
            ResourceKey::ServiceName => "service.name",
            ResourceKey::ServiceVersion => "service.version",
            ResourceKey::ServiceNamespace => "service.namespace",
            ResourceKey::ServiceInstanceId => "service.instance.id",
            ResourceKey::DeploymentEnvironment => "deployment.environment",
            ResourceKey::Custom(s) => s,
        }
    }
}

impl From<&str> for ResourceKey {
    fn from(s: &str) -> Self {
        match s {
            "service.name" => ResourceKey::ServiceName,
            "service.version" => ResourceKey::ServiceVersion,
            "service.namespace" => ResourceKey::ServiceNamespace,
            "service.instance.id" => ResourceKey::ServiceInstanceId,
            "deployment.environment" => ResourceKey::DeploymentEnvironment,
            other => ResourceKey::Custom(other.to_string()),
        }
    }
}

/// OpenTelemetry endpoint configuration
#[derive(Debug, Clone)]
pub struct OtlpEndpoint {
    url: String,
    timeout: Duration,
}

impl OtlpEndpoint {
    /// Create a new OTLP endpoint with validation
    pub fn new(url: String) -> Result<Self, OtelConfigError> {
        if url.is_empty() {
            return Err(OtelConfigError::InvalidEndpoint(
                "Endpoint URL cannot be empty".to_string(),
            ));
        }

        // Basic URL validation
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(OtelConfigError::InvalidEndpoint(
                "Endpoint URL must start with http:// or https://".to_string(),
            ));
        }

        Ok(Self {
            url,
            timeout: Duration::from_secs(30), // Default timeout
        })
    }

    /// Set timeout for OTLP exports
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

/// Service identity for OpenTelemetry resources
#[derive(Debug, Clone)]
pub struct ServiceIdentity {
    name: String,
    version: String,
    namespace: Option<String>,
    instance_id: Option<String>,
}

impl ServiceIdentity {
    /// Create new service identity with validation
    pub fn new(name: String, version: String) -> Result<Self, OtelConfigError> {
        if name.is_empty() {
            return Err(OtelConfigError::InvalidServiceName(
                "Service name cannot be empty".to_string(),
            ));
        }

        if version.is_empty() {
            return Err(OtelConfigError::InvalidServiceVersion(
                "Service version cannot be empty".to_string(),
            ));
        }

        Ok(Self {
            name,
            version,
            namespace: None,
            instance_id: None,
        })
    }

    /// Set service namespace
    pub fn with_namespace(mut self, namespace: String) -> Self {
        self.namespace = Some(namespace);
        self
    }

    /// Set service instance ID
    pub fn with_instance_id(mut self, instance_id: String) -> Self {
        self.instance_id = Some(instance_id);
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn namespace(&self) -> Option<&str> {
        self.namespace.as_deref()
    }

    pub fn instance_id(&self) -> Option<&str> {
        self.instance_id.as_deref()
    }
}

/// OpenTelemetry configuration with type safety and validation
#[derive(Debug, Clone)]
pub struct OtelConfig {
    endpoint: OtlpEndpoint,
    service: ServiceIdentity,
    resource_attributes: HashMap<ResourceKey, String>,
    deployment_environment: Option<String>,
}

/// Configuration errors that prevent invalid states
#[derive(thiserror::Error, Debug)]
pub enum OtelConfigError {
    #[error("Invalid endpoint: {0}")]
    InvalidEndpoint(String),

    #[error("Invalid service name: {0}")]
    InvalidServiceName(String),

    #[error("Invalid service version: {0}")]
    InvalidServiceVersion(String),

    #[error("Duplicate resource attribute: {0}")]
    DuplicateAttribute(String),
}

impl OtelConfig {
    /// Create new OpenTelemetry configuration with validation
    pub fn new(endpoint_url: String, service_name: String) -> Result<Self, OtelConfigError> {
        let endpoint = OtlpEndpoint::new(endpoint_url)?;
        let service = ServiceIdentity::new(service_name, env!("CARGO_PKG_VERSION").to_string())?;

        let mut resource_attributes = HashMap::new();
        resource_attributes.insert(ResourceKey::ServiceName, service.name().to_string());
        resource_attributes.insert(ResourceKey::ServiceVersion, service.version().to_string());

        Ok(Self {
            endpoint,
            service,
            resource_attributes,
            deployment_environment: None,
        })
    }

    /// Set deployment environment
    pub fn with_environment(mut self, environment: String) -> Self {
        self.deployment_environment = Some(environment.clone());
        self.resource_attributes
            .insert(ResourceKey::DeploymentEnvironment, environment);
        self
    }

    /// Add custom resource attribute with validation
    pub fn with_resource_attribute(
        mut self,
        key: ResourceKey,
        value: String,
    ) -> Result<Self, OtelConfigError> {
        if self.resource_attributes.contains_key(&key) {
            return Err(OtelConfigError::DuplicateAttribute(
                key.as_str().to_string(),
            ));
        }

        self.resource_attributes.insert(key, value);
        Ok(self)
    }

    /// Set service namespace
    pub fn with_service_namespace(mut self, namespace: String) -> Self {
        self.service = self.service.with_namespace(namespace.clone());
        self.resource_attributes
            .insert(ResourceKey::ServiceNamespace, namespace);
        self
    }

    /// Configure endpoint timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.endpoint = self.endpoint.with_timeout(timeout);
        self
    }

    /// Get endpoint
    pub fn endpoint(&self) -> &OtlpEndpoint {
        &self.endpoint
    }

    /// Get service identity
    pub fn service(&self) -> &ServiceIdentity {
        &self.service
    }

    /// Get all resource attributes
    pub fn resource_attributes(&self) -> &HashMap<ResourceKey, String> {
        &self.resource_attributes
    }

    /// Get deployment environment
    pub fn deployment_environment(&self) -> Option<&str> {
        self.deployment_environment.as_deref()
    }
}

/// Initialize OpenTelemetry exporter with proper lifecycle management
#[cfg(feature = "opentelemetry")]
pub fn init_otel_exporter(config: &OtelConfig) -> Result<OtelState, ObservabilityError> {
    use opentelemetry::trace::TracerProvider;
    use opentelemetry::{Key, KeyValue};
    use opentelemetry_otlp::{Protocol, WithExportConfig};
    use opentelemetry_sdk::{resource::Resource, trace::SdkTracerProvider};

    // Build resource with validation
    let mut resource_kvs = Vec::new();

    for (key, value) in config.resource_attributes() {
        let otel_key = Key::new(key.as_str().to_string());
        resource_kvs.push(KeyValue::new(otel_key, value.clone()));
    }

    // Create resource with SDK defaults and custom attributes
    // Start with SDK resource detection, then add custom attributes
    let resource = Resource::builder().with_attributes(resource_kvs).build();

    // Create OTLP exporter with proper configuration
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(config.endpoint().url())
        .with_timeout(config.endpoint().timeout())
        .with_protocol(Protocol::Grpc)
        .build()
        .map_err(|e| {
            ObservabilityError::OpenTelemetryInit(format!(
                "Failed to create OTLP exporter for endpoint {}: {}",
                config.endpoint().url(),
                e
            ))
        })?;

    // Initialize tracer provider with resource
    let tracer_provider = SdkTracerProvider::builder()
        .with_resource(resource)
        .with_batch_exporter(exporter) // In 0.30.0, batch config is set separately
        .build();

    // Set global tracer provider
    opentelemetry::global::set_tracer_provider(tracer_provider.clone());

    let tracer = tracer_provider.tracer("skreaver-observability");

    tracing::info!(
        endpoint = config.endpoint().url(),
        service.name = config.service().name(),
        service.version = config.service().version(),
        service.namespace = config.service().namespace(),
        deployment.environment = config.deployment_environment(),
        "OpenTelemetry exporter initialized successfully"
    );

    Ok(OtelState::new(tracer_provider, tracer))
}

/// Represents the initialized OpenTelemetry state with proper lifecycle management
#[derive(Debug)]
pub struct OtelState {
    _tracer_provider: opentelemetry_sdk::trace::SdkTracerProvider,
    _tracer: opentelemetry_sdk::trace::Tracer,
}

impl OtelState {
    fn new(
        tracer_provider: opentelemetry_sdk::trace::SdkTracerProvider,
        tracer: opentelemetry_sdk::trace::Tracer,
    ) -> Self {
        Self {
            _tracer_provider: tracer_provider,
            _tracer: tracer,
        }
    }

    /// Shutdown OpenTelemetry gracefully
    pub fn shutdown(self) -> Result<(), ObservabilityError> {
        // The tracer provider will be dropped and shutdown automatically
        // when this struct is dropped, ensuring proper cleanup
        tracing::info!("OpenTelemetry exporter shutdown initiated");
        Ok(())
    }
}

impl Drop for OtelState {
    fn drop(&mut self) {
        // Ensure graceful shutdown when the state is dropped
        if let Err(e) = self._tracer_provider.shutdown() {
            tracing::error!(error = %e, "Failed to shutdown OpenTelemetry tracer provider");
        }
    }
}

#[cfg(not(feature = "opentelemetry"))]
pub fn init_otel_exporter(_config: &OtelConfig) -> Result<OtelState, ObservabilityError> {
    Err(ObservabilityError::OpenTelemetryInit(
        "OpenTelemetry feature not enabled".to_string(),
    ))
}

#[cfg(not(feature = "opentelemetry"))]
/// Stub state for when OpenTelemetry is not enabled
#[derive(Debug)]
pub struct OtelState;

#[cfg(not(feature = "opentelemetry"))]
impl OtelState {
    pub fn shutdown(self) -> Result<(), ObservabilityError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_otel_config_creation() {
        let config = OtelConfig::new(
            "http://localhost:4317".to_string(),
            "skreaver-test".to_string(),
        )
        .expect("Config creation should succeed");

        assert_eq!(config.endpoint().url(), "http://localhost:4317");
        assert_eq!(config.service().name(), "skreaver-test");
        assert!(!config.resource_attributes().is_empty());

        // Should have default service attributes
        assert!(
            config
                .resource_attributes()
                .contains_key(&ResourceKey::ServiceName)
        );
        assert!(
            config
                .resource_attributes()
                .contains_key(&ResourceKey::ServiceVersion)
        );
    }

    #[test]
    fn test_otel_config_with_environment() {
        let config = OtelConfig::new(
            "https://localhost:4317".to_string(),
            "skreaver-test".to_string(),
        )
        .expect("Config creation should succeed")
        .with_environment("test".to_string());

        assert_eq!(config.deployment_environment(), Some("test"));
        assert_eq!(
            config
                .resource_attributes()
                .get(&ResourceKey::DeploymentEnvironment),
            Some(&"test".to_string())
        );
    }

    #[test]
    fn test_otel_config_with_custom_attribute() {
        let config = OtelConfig::new(
            "https://localhost:4317".to_string(),
            "skreaver-test".to_string(),
        )
        .expect("Config creation should succeed")
        .with_resource_attribute(
            ResourceKey::Custom("custom.key".to_string()),
            "custom_value".to_string(),
        )
        .expect("Adding custom attribute should succeed");

        assert_eq!(
            config
                .resource_attributes()
                .get(&ResourceKey::Custom("custom.key".to_string())),
            Some(&"custom_value".to_string())
        );
    }

    #[test]
    fn test_otel_config_validation() {
        // Empty endpoint should fail
        assert!(OtelConfig::new("".to_string(), "test".to_string()).is_err());

        // Invalid endpoint should fail
        assert!(OtelConfig::new("invalid-url".to_string(), "test".to_string()).is_err());

        // Empty service name should fail
        assert!(OtelConfig::new("http://localhost:4317".to_string(), "".to_string()).is_err());
    }

    #[test]
    fn test_duplicate_attribute_prevention() {
        let config = OtelConfig::new(
            "https://localhost:4317".to_string(),
            "skreaver-test".to_string(),
        )
        .expect("Config creation should succeed");

        // Try to add service name again - should fail
        let result =
            config.with_resource_attribute(ResourceKey::ServiceName, "new-name".to_string());
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            OtelConfigError::DuplicateAttribute(_)
        ));
    }

    #[test]
    fn test_endpoint_validation() {
        // Valid HTTP endpoint
        assert!(OtlpEndpoint::new("http://localhost:4317".to_string()).is_ok());

        // Valid HTTPS endpoint
        assert!(OtlpEndpoint::new("https://otel.example.com:4317".to_string()).is_ok());

        // Invalid endpoints
        assert!(OtlpEndpoint::new("".to_string()).is_err());
        assert!(OtlpEndpoint::new("ftp://localhost".to_string()).is_err());
        assert!(OtlpEndpoint::new("localhost:4317".to_string()).is_err());
    }

    #[test]
    fn test_endpoint_timeout_configuration() {
        let endpoint = OtlpEndpoint::new("https://localhost:4317".to_string())
            .expect("Endpoint creation should succeed")
            .with_timeout(Duration::from_secs(60));

        assert_eq!(endpoint.timeout(), Duration::from_secs(60));
    }

    #[test]
    fn test_resource_key_conversion() {
        assert_eq!(ResourceKey::ServiceName.as_str(), "service.name");
        assert_eq!(ResourceKey::ServiceVersion.as_str(), "service.version");
        assert_eq!(
            ResourceKey::DeploymentEnvironment.as_str(),
            "deployment.environment"
        );

        let custom_key = ResourceKey::Custom("custom.key".to_string());
        assert_eq!(custom_key.as_str(), "custom.key");

        // Test from string conversion
        assert_eq!(ResourceKey::from("service.name"), ResourceKey::ServiceName);
        assert_eq!(
            ResourceKey::from("unknown.key"),
            ResourceKey::Custom("unknown.key".to_string())
        );
    }

    #[test]
    fn test_service_identity_validation() {
        // Valid service identity
        let identity = ServiceIdentity::new("test-service".to_string(), "1.0.0".to_string());
        assert!(identity.is_ok());

        let identity = identity.unwrap();
        assert_eq!(identity.name(), "test-service");
        assert_eq!(identity.version(), "1.0.0");
        assert_eq!(identity.namespace(), None);
        assert_eq!(identity.instance_id(), None);

        // Test with namespace and instance ID
        let identity = identity
            .with_namespace("production".to_string())
            .with_instance_id("instance-123".to_string());

        assert_eq!(identity.namespace(), Some("production"));
        assert_eq!(identity.instance_id(), Some("instance-123"));

        // Invalid service identity
        assert!(ServiceIdentity::new("".to_string(), "1.0.0".to_string()).is_err());
        assert!(ServiceIdentity::new("test".to_string(), "".to_string()).is_err());
    }
}
