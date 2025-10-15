//! # HTTP Runtime
//!
//! This module provides a production-ready HTTP server runtime for Skreaver agents,
//! enabling secure remote interaction through RESTful APIs with authentication,
//! rate limiting, and streaming capabilities. The runtime manages agent lifecycle,
//! handles observations, and provides real-time status information.

use crate::runtime::{
    Coordinator,
    agent_builders::{AdvancedAgentBuilder, AnalyticsAgentBuilder, EchoAgentBuilder},
    agent_factory::{AgentFactory, AgentFactoryError},
    agent_instance::{AgentInstance, CoordinatorTrait},
    api_types::{AgentSpec, CreateAgentResponse},
    backpressure::{BackpressureConfig, BackpressureManager},
    rate_limit::{RateLimitConfig, RateLimitState},
};
use skreaver_core::Agent;
use skreaver_core::auth::rbac::RoleManager;
use skreaver_core::security::SecurityConfig;
use skreaver_observability::{ObservabilityConfig, init_observability};
use skreaver_tools::{SecureToolRegistry, ToolRegistry};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

/// Unique identifier for an agent instance
pub type AgentId = String;

/// HTTP server state containing all running agents and security configuration
///
/// Tool registry is wrapped in `SecureToolRegistry` to enforce security policies at runtime.
#[derive(Clone)]
pub struct HttpAgentRuntime<T: ToolRegistry> {
    pub agents: Arc<RwLock<HashMap<AgentId, AgentInstance>>>,
    /// Tool registry wrapped with security policy enforcement
    pub tool_registry: Arc<SecureToolRegistry<T>>,
    pub rate_limit_state: Arc<RateLimitState>,
    pub backpressure_manager: Arc<BackpressureManager>,
    pub agent_factory: Arc<AgentFactory>,
    /// Security configuration loaded from file or defaults
    pub security_config: Arc<SecurityConfig>,
}

/// HTTP runtime configuration
#[derive(Debug, Clone)]
pub struct HttpRuntimeConfig {
    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,
    /// Backpressure and queue management configuration
    pub backpressure: BackpressureConfig,
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
            request_timeout_secs: 30,
            max_body_size: 16 * 1024 * 1024, // 16MB
            enable_cors: true,
            enable_openapi: true,
            observability: ObservabilityConfig::default(),
            security_config_path: None, // Use default config
        }
    }
}

// AgentInstance and CoordinatorTrait are now imported from agent_instance module

impl<A: Agent + Send + Sync + 'static, T: ToolRegistry + Clone> CoordinatorTrait
    for Coordinator<A, T>
where
    A::Observation: From<String> + std::fmt::Display,
    A::Action: ToString,
{
    fn step(&mut self, input: String) -> String {
        let observation = A::Observation::from(input);
        let action = self.step(observation);
        action.to_string()
    }

    fn get_agent_type(&self) -> &'static str {
        std::any::type_name::<A>()
    }
}

impl<T: ToolRegistry + Clone + Send + Sync + 'static> HttpAgentRuntime<T> {
    /// Create a new HTTP agent runtime with default configuration
    pub fn new(tool_registry: T) -> Self {
        Self::with_config(tool_registry, HttpRuntimeConfig::default())
    }

    /// Create a new HTTP agent runtime with custom configuration
    pub fn with_config(tool_registry: T, config: HttpRuntimeConfig) -> Self {
        // Initialize observability framework
        if let Err(e) = init_observability(config.observability.clone()) {
            tracing::warn!("Failed to initialize observability: {}", e);
        }

        // Load security configuration with fail-fast validation
        let security_config = if let Some(config_path) = &config.security_config_path {
            match SecurityConfig::load_from_file(config_path) {
                Ok(cfg) => {
                    tracing::info!(
                        "Loaded security configuration from: {}",
                        config_path.display()
                    );
                    // Validate configuration (fail-fast on errors)
                    cfg.validate().unwrap_or_else(|e| {
                        panic!(
                            "Security configuration validation failed: {}\n\
                            Config file: {}\n\
                            CRITICAL: Invalid security configuration prevents startup. \
                            Please fix the configuration file or remove it to use defaults.",
                            e,
                            config_path.display()
                        )
                    });
                    tracing::info!("Security configuration validated successfully");
                    cfg
                }
                Err(e) => {
                    // Check if this is a "file not found" error - warn and use defaults
                    // This allows development/testing without requiring config files
                    if e.to_string().contains("No such file or directory")
                        || e.to_string().contains("cannot find the path")
                    {
                        tracing::warn!(
                            "Security configuration file not found: {} - using defaults. \
                            Error: {}",
                            config_path.display(),
                            e
                        );
                        SecurityConfig::create_default()
                    } else {
                        // Other errors (permission denied, invalid TOML, etc.) are critical
                        panic!(
                            "Failed to load security configuration from {}: {}\n\
                            CRITICAL: Could not read security configuration file. \
                            Please ensure the file exists and is readable, or remove the \
                            security_config_path setting to use defaults.",
                            config_path.display(),
                            e
                        )
                    }
                }
            }
        } else {
            tracing::info!("No security configuration file specified, using defaults");
            let default_config = SecurityConfig::create_default();
            // Validate defaults (should always pass, but check anyway)
            default_config.validate().unwrap_or_else(|e| {
                panic!(
                    "Default security configuration is invalid: {}\n\
                    This is a bug - please report it.",
                    e
                )
            });
            default_config
        };

        // Wrap tool registry with security policy and RBAC enforcement
        let security_config_arc = Arc::new(security_config);
        let role_manager = Arc::new(RoleManager::with_defaults());
        let secure_registry = SecureToolRegistry::new(
            tool_registry,
            Arc::clone(&security_config_arc),
            role_manager,
        );
        tracing::info!("Tool registry wrapped with security policy and RBAC enforcement");

        let backpressure_manager = Arc::new(BackpressureManager::new(config.backpressure.clone()));

        // Start backpressure manager in background
        let backpressure_manager_clone = Arc::clone(&backpressure_manager);
        tokio::spawn(async move {
            if let Err(e) = backpressure_manager_clone.start().await {
                tracing::error!("Failed to start backpressure manager: {}", e);
            }
        });

        // Create and configure agent factory with standard builders
        let mut agent_factory = AgentFactory::new();
        agent_factory.register_builder(Box::new(EchoAgentBuilder));
        agent_factory.register_builder(Box::new(AdvancedAgentBuilder));
        agent_factory.register_builder(Box::new(AnalyticsAgentBuilder));

        Self {
            agents: agent_factory.agents(),
            tool_registry: Arc::new(secure_registry),
            rate_limit_state: Arc::new(RateLimitState::new(config.rate_limit)),
            backpressure_manager,
            agent_factory: Arc::new(agent_factory),
            security_config: security_config_arc,
        }
    }

    /// Create a new agent from specification using the factory pattern
    pub async fn create_agent(
        &self,
        spec: AgentSpec,
        custom_id: Option<String>,
    ) -> Result<CreateAgentResponse, AgentFactoryError> {
        self.agent_factory.create_agent(spec, custom_id).await
    }

    /// Get list of supported agent types
    pub fn supported_agent_types(&self) -> Vec<crate::runtime::api_types::AgentType> {
        self.agent_factory.supported_types()
    }

    /// Check if an agent type is supported
    pub fn supports_agent_type(&self, agent_type: &crate::runtime::api_types::AgentType) -> bool {
        self.agent_factory.supports_type(agent_type)
    }

    /// Remove an agent by ID
    pub async fn remove_agent(&self, agent_id: &str) -> Result<(), AgentFactoryError> {
        self.agent_factory.remove_agent(agent_id).await
    }

    /// Check if an agent exists by ID
    pub async fn has_agent(&self, agent_id: &str) -> bool {
        self.agent_factory.has_agent(agent_id).await
    }

    /// List all agent IDs
    pub async fn list_agent_ids(&self) -> Vec<String> {
        self.agent_factory.list_agent_ids().await
    }

    /// Get agent count
    pub async fn agent_count(&self) -> usize {
        self.agent_factory.agent_count().await
    }

    /// Get the security configuration
    pub fn security_config(&self) -> &SecurityConfig {
        &self.security_config
    }

    /// Add an agent instance to the runtime (legacy method for backward compatibility)
    pub async fn add_agent<A>(&self, agent_id: String, agent: A) -> Result<(), String>
    where
        A: Agent + Send + Sync + 'static,
        A::Observation: From<String> + std::fmt::Display,
        A::Action: ToString,
    {
        let coordinator = Coordinator::new(agent, (*self.tool_registry).clone());
        let agent_instance = crate::runtime::agent_instance::AgentInstance::new(
            crate::runtime::agent_instance::AgentId::new(agent_id.clone())
                .map_err(|e| e.to_string())?,
            std::any::type_name::<A>().to_string(),
            Box::new(coordinator),
        );

        let mut agents = self.agents.write().await;
        agents.insert(agent_id, agent_instance);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::auth::create_jwt_token;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use serde_json::{Value, json};
    use skreaver_core::InMemoryMemory;
    use skreaver_core::{
        Agent, ExecutionResult, MemoryReader, MemoryUpdate, MemoryWriter, ToolCall,
    };
    use skreaver_tools::InMemoryToolRegistry;
    use tower::ServiceExt;

    /// Simple test agent that echoes input
    struct TestAgent {
        memory: InMemoryMemory,
        last_input: Option<String>,
    }

    impl TestAgent {
        fn new(memory: InMemoryMemory) -> Self {
            Self {
                memory,
                last_input: None,
            }
        }
    }

    impl Agent for TestAgent {
        type Observation = String;
        type Action = String;

        fn observe(&mut self, input: Self::Observation) {
            self.last_input = Some(input.clone());
            if let Ok(update) = MemoryUpdate::new("input", &input) {
                let _ = self.memory_writer().store(update);
            }
        }

        fn act(&mut self) -> Self::Action {
            self.last_input
                .as_ref()
                .map(|s| format!("Test response: {}", s))
                .unwrap_or_else(|| "No input".into())
        }

        fn call_tools(&self) -> Vec<ToolCall> {
            Vec::new()
        }

        fn handle_result(&mut self, _result: ExecutionResult) {
            // No-op for test agent
        }

        fn update_context(&mut self, update: MemoryUpdate) {
            let _ = self.memory_writer().store(update);
        }

        fn memory_reader(&self) -> &dyn MemoryReader {
            &self.memory
        }

        fn memory_writer(&mut self) -> &mut dyn MemoryWriter {
            &mut self.memory
        }
    }

    /// Helper to create a test HTTP runtime
    fn create_test_runtime() -> HttpAgentRuntime<InMemoryToolRegistry> {
        let tool_registry = InMemoryToolRegistry::new();
        HttpAgentRuntime::new(tool_registry)
    }

    /// Helper to create a test agent and add it to runtime
    async fn setup_test_agent(runtime: &HttpAgentRuntime<InMemoryToolRegistry>, agent_id: &str) {
        let agent = TestAgent::new(InMemoryMemory::new());
        runtime
            .add_agent(agent_id.to_string(), agent)
            .await
            .unwrap();
    }

    /// Helper to create a valid JWT token for testing
    fn create_test_token() -> String {
        create_jwt_token(
            "test-user".to_string(),
            vec!["read".to_string(), "write".to_string()],
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let runtime = create_test_runtime();
        let app = runtime.router();

        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["status"], "healthy");
        assert_eq!(json["service"], "skreaver-http-runtime");
        assert!(json["timestamp"].is_string());
        assert_eq!(json["version"], "0.3.0");
    }

    #[tokio::test]
    async fn test_create_token_endpoint() {
        let runtime = create_test_runtime();
        let app = runtime.router();

        let request_body = json!({
            "user_id": "test-user",
            "permissions": ["read", "write"]
        });

        let request = Request::builder()
            .method("POST")
            .uri("/auth/token")
            .header("content-type", "application/json")
            .body(Body::from(request_body.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert!(json["token"].is_string());
        assert_eq!(json["expires_in"], 86400);
        assert_eq!(json["token_type"], "Bearer");
    }

    #[tokio::test]
    async fn test_list_agents() {
        let runtime = create_test_runtime();
        setup_test_agent(&runtime, "test-agent-1").await;
        setup_test_agent(&runtime, "test-agent-2").await;

        let app = runtime.router();
        let token = create_test_token();

        let request = Request::builder()
            .uri("/agents")
            .header("Authorization", format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["total"], 2);
        assert_eq!(json["agents"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_get_agent_status() {
        let runtime = create_test_runtime();
        setup_test_agent(&runtime, "status-test-agent").await;

        let app = runtime.router();
        let token = create_test_token();

        let request = Request::builder()
            .uri("/agents/status-test-agent/status")
            .header("Authorization", format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["agent_id"], "status-test-agent");
        assert_eq!(json["status"], "running");
        assert!(json["agent_type"].is_string());
    }

    #[tokio::test]
    async fn test_observe_agent() {
        let runtime = create_test_runtime();
        setup_test_agent(&runtime, "observe-test-agent").await;

        let app = runtime.router();
        let token = create_test_token();

        let request_body = json!({
            "input": "Hello, agent!"
        });

        let request = Request::builder()
            .method("POST")
            .uri("/agents/observe-test-agent/observe")
            .header("Authorization", format!("Bearer {}", token))
            .header("content-type", "application/json")
            .body(Body::from(request_body.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["agent_id"], "observe-test-agent");
        assert!(json["response"].is_string());
        assert!(json["timestamp"].is_string());
    }

    #[tokio::test]
    async fn test_nonexistent_agent_returns_404() {
        let runtime = create_test_runtime();
        let app = runtime.router();
        let token = create_test_token();

        let request = Request::builder()
            .uri("/agents/nonexistent/status")
            .header("Authorization", format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["error"], "agent_not_found");
    }

    #[tokio::test]
    async fn test_openapi_docs_endpoint() {
        let runtime = create_test_runtime();
        let app = runtime.router();

        let request = Request::builder().uri("/docs").body(Body::empty()).unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let content_type = response.headers().get("content-type");
        assert!(content_type.is_some());
        assert!(
            content_type
                .unwrap()
                .to_str()
                .unwrap()
                .contains("text/html")
        );
    }

    #[tokio::test]
    async fn test_openapi_spec_endpoint() {
        let runtime = create_test_runtime();
        let app = runtime.router();

        let request = Request::builder()
            .uri("/api-docs/openapi.json")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["openapi"], "3.1.0");
        assert_eq!(json["info"]["title"], "Skreaver HTTP Runtime API");
        assert_eq!(json["info"]["version"], "0.1.0");
        assert!(json["paths"].is_object());
    }

    #[tokio::test]
    async fn test_batch_observe_agent() {
        let runtime = create_test_runtime();
        setup_test_agent(&runtime, "batch-test-agent").await;

        let app = runtime.router();
        let token = create_test_token();

        let request_body = json!({
            "inputs": ["Hello batch 1", "Hello batch 2", "Hello batch 3"],
            "parallel_limit": 2,
            "timeout_seconds": 30
        });

        let request = Request::builder()
            .method("POST")
            .uri("/agents/batch-test-agent/batch")
            .header("Authorization", format!("Bearer {}", token))
            .header("content-type", "application/json")
            .body(Body::from(request_body.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["agent_id"], "batch-test-agent");
        assert_eq!(json["results"].as_array().unwrap().len(), 3);
        assert!(json["total_time_ms"].as_u64().is_some());

        // Check individual results
        let results = json["results"].as_array().unwrap();
        for (i, result) in results.iter().enumerate() {
            assert_eq!(result["index"], i);
            assert_eq!(result["success"], true);
            assert!(result["response"].is_string());
            assert!(result["processing_time_ms"].as_u64().is_some());
        }
    }

    #[tokio::test]
    async fn test_batch_observe_agent_empty_batch() {
        let runtime = create_test_runtime();
        setup_test_agent(&runtime, "empty-batch-agent").await;

        let app = runtime.router();
        let token = create_test_token();

        let request_body = json!({
            "inputs": [],
            "parallel_limit": 1,
            "timeout_seconds": 30
        });

        let request = Request::builder()
            .method("POST")
            .uri("/agents/empty-batch-agent/batch")
            .header("Authorization", format!("Bearer {}", token))
            .header("content-type", "application/json")
            .body(Body::from(request_body.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["error"], "empty_batch");
    }

    #[tokio::test]
    async fn test_batch_observe_agent_too_large() {
        let runtime = create_test_runtime();
        setup_test_agent(&runtime, "large-batch-agent").await;

        let app = runtime.router();
        let token = create_test_token();

        // Create a batch with 101 inputs (over the limit)
        let inputs: Vec<String> = (0..101).map(|i| format!("Input {}", i)).collect();
        let request_body = json!({
            "inputs": inputs,
            "parallel_limit": 1,
            "timeout_seconds": 30
        });

        let request = Request::builder()
            .method("POST")
            .uri("/agents/large-batch-agent/batch")
            .header("Authorization", format!("Bearer {}", token))
            .header("content-type", "application/json")
            .body(Body::from(request_body.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["error"], "batch_too_large");
    }

    #[tokio::test]
    async fn test_observe_agent_stream_endpoint() {
        let runtime = create_test_runtime();
        setup_test_agent(&runtime, "stream-test-agent").await;

        let app = runtime.router();
        let token = create_test_token();

        let request_body = json!({
            "input": "Hello, streaming agent!"
        });

        let request = Request::builder()
            .method("POST")
            .uri("/agents/stream-test-agent/observe/stream")
            .header("Authorization", format!("Bearer {}", token))
            .header("content-type", "application/json")
            .body(Body::from(request_body.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Check that we get SSE content type
        let content_type = response.headers().get("content-type");
        assert!(content_type.is_some());
        let content_type_str = content_type.unwrap().to_str().unwrap();
        assert!(content_type_str.contains("text/event-stream"));
    }

    #[tokio::test]
    async fn test_concurrent_batch_requests() {
        let runtime = create_test_runtime();
        setup_test_agent(&runtime, "concurrent-batch-agent").await;

        let app = runtime.router();
        let token = create_test_token();

        // Create multiple concurrent batch requests
        let mut handles = Vec::new();

        for batch_id in 0..3 {
            let app_clone = app.clone();
            let token_clone = token.clone();

            let handle = tokio::spawn(async move {
                let request_body = json!({
                    "inputs": [
                        format!("Batch {} input 1", batch_id),
                        format!("Batch {} input 2", batch_id)
                    ],
                    "parallel_limit": 1,
                    "timeout_seconds": 10
                });

                let request = Request::builder()
                    .method("POST")
                    .uri("/agents/concurrent-batch-agent/batch")
                    .header("Authorization", format!("Bearer {}", token_clone))
                    .header("content-type", "application/json")
                    .body(Body::from(request_body.to_string()))
                    .unwrap();

                app_clone.oneshot(request).await.unwrap()
            });

            handles.push(handle);
        }

        // Wait for all requests to complete
        let mut responses = Vec::new();
        for handle in handles {
            let response = handle.await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            responses.push(response);
        }

        // Verify all responses are valid
        for response in responses {
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            let json: Value = serde_json::from_slice(&body).unwrap();

            assert_eq!(json["agent_id"], "concurrent-batch-agent");
            assert_eq!(json["results"].as_array().unwrap().len(), 2);

            // All operations should succeed
            let results = json["results"].as_array().unwrap();
            for result in results {
                assert_eq!(result["success"], true);
            }
        }
    }

    #[tokio::test]
    async fn test_batch_with_nonexistent_agent() {
        let runtime = create_test_runtime();

        let app = runtime.router();
        let token = create_test_token();

        let request_body = json!({
            "inputs": ["Test input"],
            "parallel_limit": 1,
            "timeout_seconds": 10
        });

        let request = Request::builder()
            .method("POST")
            .uri("/agents/nonexistent-agent/batch")
            .header("Authorization", format!("Bearer {}", token))
            .header("content-type", "application/json")
            .body(Body::from(request_body.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["error"], "agent_not_found");
    }

    #[tokio::test]
    async fn test_high_concurrency_stress() {
        let runtime = create_test_runtime();
        setup_test_agent(&runtime, "stress-test-agent").await;

        let app = runtime.router();
        let token = create_test_token();

        // Create many concurrent requests of different types
        let mut handles = Vec::new();

        // Mix of batch requests, individual observations, and status checks
        for i in 0..10 {
            let app_clone = app.clone();
            let token_clone = token.clone();

            // Batch request
            let batch_handle = tokio::spawn(async move {
                let request_body = json!({
                    "inputs": [format!("Stress test batch {} item 1", i), format!("Stress test batch {} item 2", i)],
                    "parallel_limit": 2,
                    "timeout_seconds": 5
                });

                let request = Request::builder()
                    .method("POST")
                    .uri("/agents/stress-test-agent/batch")
                    .header("Authorization", format!("Bearer {}", token_clone))
                    .header("content-type", "application/json")
                    .body(Body::from(request_body.to_string()))
                    .unwrap();

                app_clone.oneshot(request).await.unwrap()
            });
            handles.push(batch_handle);

            // Individual observation
            let obs_app = app.clone();
            let obs_token = token.clone();
            let obs_handle = tokio::spawn(async move {
                let request_body = json!({
                    "input": format!("Individual observation {}", i)
                });

                let request = Request::builder()
                    .method("POST")
                    .uri("/agents/stress-test-agent/observe")
                    .header("Authorization", format!("Bearer {}", obs_token))
                    .header("content-type", "application/json")
                    .body(Body::from(request_body.to_string()))
                    .unwrap();

                obs_app.oneshot(request).await.unwrap()
            });
            handles.push(obs_handle);

            // Status check
            let status_app = app.clone();
            let status_token = token.clone();
            let status_handle = tokio::spawn(async move {
                let request = Request::builder()
                    .uri("/agents/stress-test-agent/status")
                    .header("Authorization", format!("Bearer {}", status_token))
                    .body(Body::empty())
                    .unwrap();

                status_app.oneshot(request).await.unwrap()
            });
            handles.push(status_handle);
        }

        // Wait for all requests to complete
        let mut successful_responses = 0;
        for handle in handles {
            let response = handle.await.unwrap();
            if response.status() == StatusCode::OK {
                successful_responses += 1;
            }
        }

        // All requests should succeed under high concurrency
        assert_eq!(
            successful_responses, 30,
            "All 30 concurrent requests should succeed"
        );
    }

    // ===================================================================
    // Authentication Integration Tests
    // ===================================================================

    #[tokio::test]
    async fn test_protected_endpoint_requires_auth() {
        let runtime = create_test_runtime();
        setup_test_agent(&runtime, "test-agent").await;
        let app = runtime.router();

        // Try accessing protected endpoint without auth
        let request = Request::builder()
            .uri("/agents")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "authentication_required");
    }

    #[tokio::test]
    async fn test_protected_endpoint_with_invalid_token() {
        let runtime = create_test_runtime();
        setup_test_agent(&runtime, "test-agent").await;
        let app = runtime.router();

        // Try with invalid JWT token
        let request = Request::builder()
            .uri("/agents")
            .header("Authorization", "Bearer invalid-token-12345")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "invalid_token");
    }

    #[tokio::test]
    async fn test_protected_endpoint_with_valid_token() {
        let runtime = create_test_runtime();
        setup_test_agent(&runtime, "test-agent").await;
        let app = runtime.router();
        let token = create_test_token();

        // Try with valid JWT token
        let request = Request::builder()
            .uri("/agents")
            .header("Authorization", format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_public_endpoint_no_auth_required() {
        let runtime = create_test_runtime();
        let app = runtime.router();

        // Public endpoints should work without auth
        let endpoints = vec!["/health", "/ready", "/metrics"];

        for endpoint in endpoints {
            let request = Request::builder()
                .uri(endpoint)
                .body(Body::empty())
                .unwrap();

            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "Endpoint {} should be accessible without auth",
                endpoint
            );
        }
    }

    #[tokio::test]
    async fn test_observe_endpoint_requires_auth() {
        let runtime = create_test_runtime();
        setup_test_agent(&runtime, "auth-test-agent").await;
        let app = runtime.router();

        let request_body = json!({
            "input": "Test input"
        });

        // Try without auth
        let request = Request::builder()
            .method("POST")
            .uri("/agents/auth-test-agent/observe")
            .header("content-type", "application/json")
            .body(Body::from(request_body.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_api_key_authentication() {
        let runtime = create_test_runtime();
        setup_test_agent(&runtime, "test-agent").await;
        let app = runtime.router();

        // Try with test API key (available in debug builds)
        let request = Request::builder()
            .uri("/agents")
            .header("Authorization", "Bearer sk-test-key-123")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_x_api_key_header_authentication() {
        let runtime = create_test_runtime();
        setup_test_agent(&runtime, "test-agent").await;
        let app = runtime.router();

        // Try with X-API-Key header
        let request = Request::builder()
            .uri("/agents")
            .header("X-API-Key", "sk-test-key-123")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_invalid_api_key() {
        let runtime = create_test_runtime();
        setup_test_agent(&runtime, "test-agent").await;
        let app = runtime.router();

        // Try with invalid API key
        let request = Request::builder()
            .uri("/agents")
            .header("Authorization", "Bearer sk-invalid-key-999")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "invalid_api_key");
    }

    #[tokio::test]
    async fn test_create_token_endpoint_public() {
        let runtime = create_test_runtime();
        let app = runtime.router();

        let request_body = json!({
            "user_id": "new-user",
            "permissions": ["read"]
        });

        // Token creation should not require auth
        let request = Request::builder()
            .method("POST")
            .uri("/auth/token")
            .header("content-type", "application/json")
            .body(Body::from(request_body.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert!(json["token"].is_string());
    }

    #[tokio::test]
    async fn test_all_agent_endpoints_require_auth() {
        let runtime = create_test_runtime();
        setup_test_agent(&runtime, "test-agent").await;
        let app = runtime.router();

        // Test all protected agent endpoints
        let protected_endpoints = vec![
            ("GET", "/agents"),
            ("GET", "/agents/test-agent/status"),
            ("POST", "/agents/test-agent/observe"),
            ("POST", "/agents/test-agent/observe/stream"),
            ("POST", "/agents/test-agent/batch"),
            ("GET", "/agents/test-agent/stream"),
            ("DELETE", "/agents/test-agent"),
            ("GET", "/agents/test-agent/queue/metrics"),
            ("GET", "/queue/metrics"),
        ];

        for (method, endpoint) in protected_endpoints {
            let mut request = Request::builder().uri(endpoint);

            if method == "POST" {
                request = request
                    .method(method)
                    .header("content-type", "application/json");
            } else {
                request = request.method(method);
            }

            let body = if method == "POST" {
                Body::from(json!({"input": "test"}).to_string())
            } else {
                Body::empty()
            };

            let request = request.body(body).unwrap();
            let response = app.clone().oneshot(request).await.unwrap();

            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "Endpoint {} {} should require authentication",
                method,
                endpoint
            );
        }
    }
}
