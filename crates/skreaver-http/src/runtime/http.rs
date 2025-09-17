//! # HTTP Runtime
//!
//! This module provides a production-ready HTTP server runtime for Skreaver agents,
//! enabling secure remote interaction through RESTful APIs with authentication,
//! rate limiting, and streaming capabilities. The runtime manages agent lifecycle,
//! handles observations, and provides real-time status information.

use crate::runtime::{
    Coordinator, auth,
    backpressure::{BackpressureConfig, BackpressureManager, QueueMetrics, RequestPriority},
    rate_limit::{RateLimitConfig, RateLimitState},
    streaming::{self, StreamingAgentExecutor},
};
use async_trait::async_trait;
use axum::response::Html;
use axum::{
    Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Json, sse::Sse},
    routing::{get, post},
};
use futures::Stream;
use serde::{Deserialize, Serialize};
use skreaver_core::Agent;
use skreaver_observability::health::{HealthCheck, SystemHealth};
use skreaver_observability::metrics::get_metrics_registry;
use skreaver_observability::{
    AgentId as ObsAgentId, HealthChecker, ObservabilityConfig, SessionId, init_observability,
};
use skreaver_tools::ToolRegistry;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use utoipa::{OpenApi, ToSchema};

/// Unique identifier for an agent instance
pub type AgentId = String;

/// HTTP server state containing all running agents and security configuration
#[derive(Clone)]
pub struct HttpAgentRuntime<T: ToolRegistry> {
    pub agents: Arc<RwLock<HashMap<AgentId, AgentInstance>>>,
    pub tool_registry: Arc<T>,
    pub rate_limit_state: Arc<RateLimitState>,
    pub backpressure_manager: Arc<BackpressureManager>,
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
        }
    }
}

/// Container for agent and its coordinator
pub struct AgentInstance {
    pub coordinator: Box<dyn CoordinatorTrait + Send + Sync>,
}

/// Trait to allow dynamic dispatch of coordinators
pub trait CoordinatorTrait {
    fn step(&mut self, input: String) -> String;
    fn get_agent_type(&self) -> &'static str;
}

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

/// Request body for creating a new agent
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateAgentRequest {
    /// Type of agent to create
    #[schema(example = "simple_agent")]
    pub agent_type: String,
    /// Optional name for the agent
    #[schema(example = "my-agent")]
    pub name: Option<String>,
}

/// Response for agent creation
#[derive(Debug, Serialize, ToSchema)]
pub struct CreateAgentResponse {
    /// Unique identifier for the created agent
    #[schema(example = "agent-12345")]
    pub agent_id: String,
    /// Type of the created agent
    #[schema(example = "simple_agent")]
    pub agent_type: String,
    /// Current status of the agent
    #[schema(example = "running")]
    pub status: String,
}

/// Request body for sending observations to an agent
#[derive(Debug, Deserialize, ToSchema)]
pub struct ObserveRequest {
    /// Input observation for the agent
    #[schema(example = "Hello, agent!")]
    pub input: String,
    /// Whether to stream the response in real-time
    #[serde(default)]
    #[schema(default = false)]
    pub stream: bool,
}

/// Response from agent observation
#[derive(Debug, Serialize, ToSchema)]
pub struct ObserveResponse {
    /// ID of the agent that processed the observation
    #[schema(example = "agent-12345")]
    pub agent_id: String,
    /// Agent's response to the observation
    #[schema(example = "Hello! How can I help you?")]
    pub response: String,
    /// Timestamp when the response was generated
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Agent status information
#[derive(Debug, Serialize, ToSchema)]
pub struct AgentStatus {
    /// Unique identifier of the agent
    #[schema(example = "agent-12345")]
    pub agent_id: String,
    /// Type of the agent
    #[schema(example = "simple_agent")]
    pub agent_type: String,
    /// Current status of the agent
    #[schema(example = "running")]
    pub status: String,
    /// When the agent was created
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// List of all agents
#[derive(Debug, Serialize, ToSchema)]
pub struct AgentsListResponse {
    /// List of agent status information
    pub agents: Vec<AgentStatus>,
    /// Total number of agents
    #[schema(example = 5)]
    pub total: usize,
}

/// Error response format
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    /// Error code identifier
    #[schema(example = "agent_not_found")]
    pub error: String,
    /// Human-readable error message
    #[schema(example = "Agent with ID 'agent-12345' not found")]
    pub message: String,
    /// Additional context or details about the error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

/// Request body for creating a JWT token
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTokenRequest {
    /// User identifier for the token
    #[schema(example = "test-user")]
    pub user_id: String,
    /// Permissions to grant to the user
    /// Permissions to grant to the user
    pub permissions: Vec<String>,
}

/// Response for JWT token creation
#[derive(Debug, Serialize, ToSchema)]
pub struct CreateTokenResponse {
    /// JWT access token
    #[schema(example = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...")]
    pub token: String,
    /// Token expiration time in seconds
    #[schema(example = 86400)]
    pub expires_in: u64,
    /// Token type
    #[schema(example = "Bearer")]
    pub token_type: String,
}

/// Query parameters for streaming endpoint
#[derive(Debug, Deserialize)]
pub struct StreamRequest {
    /// Optional input to send to the agent
    pub input: Option<String>,
    /// Whether to include debug information in stream
    #[serde(default)]
    pub debug: bool,
    /// Custom timeout in seconds for the operation
    pub timeout_seconds: Option<u64>,
}

/// Request for batch operations
#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchObserveRequest {
    /// List of inputs to process
    pub inputs: Vec<String>,
    /// Whether to return results as stream
    #[serde(default)]
    pub stream: bool,
    /// Maximum parallel operations
    #[serde(default = "default_parallel_limit")]
    pub parallel_limit: usize,
    /// Timeout per individual operation in seconds
    #[serde(default = "default_operation_timeout")]
    pub timeout_seconds: u64,
}

fn default_parallel_limit() -> usize {
    5
}
fn default_operation_timeout() -> u64 {
    60
}

/// Response for batch operations
#[derive(Debug, Serialize, ToSchema)]
pub struct BatchObserveResponse {
    /// Agent identifier
    pub agent_id: String,
    /// Results for each input
    pub results: Vec<BatchResult>,
    /// Total processing time
    pub total_time_ms: u64,
    /// Request timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Individual result in batch operation
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BatchResult {
    /// Input index
    pub index: usize,
    /// Whether the operation succeeded
    pub success: bool,
    /// Response content (if successful)
    pub response: Option<String>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Processing time for this individual operation
    pub time_ms: u64,
}

/// Queue metrics response
#[derive(Debug, Serialize, ToSchema)]
pub struct QueueMetricsResponse {
    /// Agent ID (if for specific agent)
    pub agent_id: Option<String>,
    /// Number of requests in queue
    pub queue_size: usize,
    /// Number of active/processing requests
    pub active_requests: usize,
    /// Total requests processed
    pub total_processed: u64,
    /// Total requests that timed out
    pub total_timeouts: u64,
    /// Total requests rejected
    pub total_rejections: u64,
    /// Average processing time in milliseconds
    pub avg_processing_time_ms: f64,
    /// Current load factor (0.0-1.0)
    pub load_factor: f64,
    /// Timestamp of metrics collection
    pub timestamp: chrono::DateTime<chrono::Utc>,
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

        let backpressure_manager = Arc::new(BackpressureManager::new(config.backpressure.clone()));

        // Start backpressure manager in background
        let backpressure_manager_clone = Arc::clone(&backpressure_manager);
        tokio::spawn(async move {
            if let Err(e) = backpressure_manager_clone.start().await {
                tracing::error!("Failed to start backpressure manager: {}", e);
            }
        });

        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            tool_registry: Arc::new(tool_registry),
            rate_limit_state: Arc::new(RateLimitState::new(config.rate_limit)),
            backpressure_manager,
        }
    }

    /// Create the Axum router with all endpoints and middleware
    pub fn router(self) -> Router {
        self.router_with_config(HttpRuntimeConfig::default())
    }

    /// Create the Axum router with custom configuration
    pub fn router_with_config(self, config: HttpRuntimeConfig) -> Router {
        let mut router = Router::new()
            // Public endpoints (no auth required)
            .route("/health", get(health_check))
            .route("/ready", get(readiness_check))
            .route("/metrics", get(metrics_endpoint))
            .route("/auth/token", post(create_token))
            // Protected endpoints (require authentication)
            .route("/agents", get(list_agents).post(create_agent))
            .route("/agents/{agent_id}/status", get(get_agent_status))
            .route("/agents/{agent_id}/observe", post(observe_agent))
            .route(
                "/agents/{agent_id}/observe/stream",
                post(observe_agent_stream),
            )
            .route("/agents/{agent_id}/batch", post(batch_observe_agent))
            .route("/agents/{agent_id}/stream", get(stream_agent))
            .route(
                "/agents/{agent_id}/queue/metrics",
                get(get_agent_queue_metrics),
            )
            .route("/agents/{agent_id}", axum::routing::delete(delete_agent))
            .route("/queue/metrics", get(get_global_queue_metrics))
            .with_state(self)
            .layer(TraceLayer::new_for_http());

        // Add CORS if enabled
        if config.enable_cors {
            router = router.layer(CorsLayer::permissive());
        }

        // Add OpenAPI documentation if enabled
        if config.enable_openapi {
            router = router.merge(create_openapi_router());
        }

        router
    }

    /// Add an agent instance to the runtime
    pub async fn add_agent<A>(&self, agent_id: String, agent: A) -> Result<(), String>
    where
        A: Agent + Send + Sync + 'static,
        A::Observation: From<String> + std::fmt::Display,
        A::Action: ToString,
    {
        let coordinator = Coordinator::new(agent, (*self.tool_registry).clone());
        let instance = AgentInstance {
            coordinator: Box::new(coordinator),
        };

        let mut agents = self.agents.write().await;
        agents.insert(agent_id, instance);
        Ok(())
    }
}

/// GET /agents - List all agents
#[utoipa::path(
    get,
    path = "/agents",
    responses(
        (status = 200, description = "List of all agents", body = AgentsListResponse),
        (status = 401, description = "Authentication required", body = auth::AuthError)
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    )
)]
async fn list_agents<T: ToolRegistry + Clone + Send + Sync>(
    State(runtime): State<HttpAgentRuntime<T>>,
) -> Result<Json<AgentsListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let agents = runtime.agents.read().await;

    let agent_statuses: Vec<AgentStatus> = agents
        .iter()
        .map(|(id, instance)| AgentStatus {
            agent_id: id.clone(),
            agent_type: instance.coordinator.get_agent_type().to_string(),
            status: "running".to_string(),
            created_at: chrono::Utc::now(), // TODO: Track actual creation time
        })
        .collect();

    Ok(Json(AgentsListResponse {
        total: agent_statuses.len(),
        agents: agent_statuses,
    }))
}

/// POST /agents - Create a new agent
#[utoipa::path(
    post,
    path = "/agents",
    request_body = CreateAgentRequest,
    responses(
        (status = 201, description = "Agent created successfully", body = CreateAgentResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Authentication required", body = auth::AuthError),
        (status = 501, description = "Not implemented", body = ErrorResponse)
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    )
)]
async fn create_agent<T: ToolRegistry + Clone + Send + Sync>(
    State(_runtime): State<HttpAgentRuntime<T>>,
    Json(_request): Json<CreateAgentRequest>,
) -> Result<Json<CreateAgentResponse>, (StatusCode, Json<ErrorResponse>)> {
    // For now, we'll return an error since we need a factory pattern to create agents dynamically
    Err((
        StatusCode::NOT_IMPLEMENTED,
        Json(ErrorResponse {
            error: "not_implemented".to_string(),
            message:
                "Dynamic agent creation not yet implemented. Use runtime.add_agent() directly."
                    .to_string(),
            details: None,
        }),
    ))
}

/// GET /agents/{agent_id}/status - Get agent status
#[utoipa::path(
    get,
    path = "/agents/{agent_id}/status",
    params(
        ("agent_id" = String, Path, description = "Agent identifier")
    ),
    responses(
        (status = 200, description = "Agent status information", body = AgentStatus),
        (status = 404, description = "Agent not found", body = ErrorResponse),
        (status = 401, description = "Authentication required", body = auth::AuthError)
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    )
)]
async fn get_agent_status<T: ToolRegistry + Clone + Send + Sync>(
    State(runtime): State<HttpAgentRuntime<T>>,
    Path(agent_id): Path<String>,
) -> Result<Json<AgentStatus>, (StatusCode, Json<ErrorResponse>)> {
    let agents = runtime.agents.read().await;

    match agents.get(&agent_id) {
        Some(instance) => Ok(Json(AgentStatus {
            agent_id: agent_id.clone(),
            agent_type: instance.coordinator.get_agent_type().to_string(),
            status: "running".to_string(),
            created_at: chrono::Utc::now(), // TODO: Track actual creation time
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "agent_not_found".to_string(),
                message: format!("Agent with ID '{}' not found", agent_id),
                details: None,
            }),
        )),
    }
}

/// POST /agents/{agent_id}/observe - Send observation to agent
#[utoipa::path(
    post,
    path = "/agents/{agent_id}/observe",
    params(
        ("agent_id" = String, Path, description = "Agent identifier")
    ),
    request_body = ObserveRequest,
    responses(
        (status = 200, description = "Agent response to observation", body = ObserveResponse),
        (status = 404, description = "Agent not found", body = ErrorResponse),
        (status = 401, description = "Authentication required", body = auth::AuthError)
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    )
)]
async fn observe_agent<T: ToolRegistry + Clone + Send + Sync + 'static>(
    State(runtime): State<HttpAgentRuntime<T>>,
    Path(agent_id): Path<String>,
    Json(request): Json<ObserveRequest>,
) -> Result<Json<ObserveResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    // Record HTTP request metrics
    if let Some(registry) = get_metrics_registry() {
        let route = format!("/agents/{}/observe", "{agent_id}");
        let _ = registry.record_http_request(&route, "POST", start_time.elapsed());
    }

    // Check if agent exists first
    {
        let agents = runtime.agents.read().await;
        if !agents.contains_key(&agent_id) {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "agent_not_found".to_string(),
                    message: format!("Agent with ID '{}' not found", agent_id),
                    details: None,
                }),
            ));
        }
    }

    // Use backpressure manager for request processing
    let priority = RequestPriority::Normal; // TODO: Allow setting priority in request
    let timeout = Some(std::time::Duration::from_secs(30)); // TODO: Make configurable

    let (_request_id, rx) = runtime
        .backpressure_manager
        .queue_request_with_input(agent_id.clone(), request.input.clone(), priority, timeout)
        .await
        .map_err(|e| {
            let status = match e {
                crate::runtime::backpressure::BackpressureError::QueueFull { .. } => {
                    StatusCode::TOO_MANY_REQUESTS
                }
                crate::runtime::backpressure::BackpressureError::SystemOverloaded { .. } => {
                    StatusCode::SERVICE_UNAVAILABLE
                }
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (
                status,
                Json(ErrorResponse {
                    error: "backpressure_error".to_string(),
                    message: e.to_string(),
                    details: None,
                }),
            )
        })?;

    // Start processing the queued request
    let runtime_clone = runtime.clone();
    let agent_id_clone = agent_id.clone();
    tokio::spawn(async move {
        let agent_id_for_processing = agent_id_clone.clone();
        let runtime_for_closure = runtime_clone.clone();
        if let Some(_) = runtime_clone
            .backpressure_manager
            .process_next_queued_request(&agent_id_clone, move |input| {
                let runtime_inner = runtime_for_closure.clone();
                let agent_id_for_closure = agent_id_for_processing.clone();
                async move {
                    // Process the request within backpressure constraints
                    let mut agents = runtime_inner.agents.write().await;
                    if let Some(instance) = agents.get_mut(&agent_id_for_closure) {
                        // Create agent session for observability
                        let session_id = SessionId::generate();

                        // Record agent session start
                        if let Some(registry) = get_metrics_registry() {
                            let obs_agent_id = ObsAgentId::new(agent_id_for_closure.clone())
                                .unwrap_or_else(|_| ObsAgentId::new("invalid-agent").unwrap());
                            let tags = skreaver_observability::CardinalTags::for_agent_session(
                                obs_agent_id.clone(),
                                session_id.clone(),
                            );
                            let _ = registry.record_agent_session_start(&tags);
                        }

                        let response = instance.coordinator.step(input);

                        // Record agent session end
                        if let Some(registry) = get_metrics_registry() {
                            let obs_agent_id = ObsAgentId::new(agent_id_for_closure.clone())
                                .unwrap_or_else(|_| ObsAgentId::new("invalid-agent").unwrap());
                            let tags = skreaver_observability::CardinalTags::for_agent_session(
                                obs_agent_id,
                                session_id,
                            );
                            let _ = registry.record_agent_session_end(&tags);
                        }

                        response
                    } else {
                        "Agent not found".to_string()
                    }
                }
            })
            .await
        {
            // Processing started successfully
        }
    });

    // Wait for response from backpressure manager
    let response = rx.await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "request_cancelled".to_string(),
                message: "Request was cancelled".to_string(),
                details: None,
            }),
        )
    })?;

    let response = response.map_err(|e| {
        let status = match e {
            crate::runtime::backpressure::BackpressureError::QueueTimeout { .. } => {
                StatusCode::REQUEST_TIMEOUT
            }
            crate::runtime::backpressure::BackpressureError::ProcessingTimeout { .. } => {
                StatusCode::REQUEST_TIMEOUT
            }
            crate::runtime::backpressure::BackpressureError::AgentNotFound { .. } => {
                StatusCode::NOT_FOUND
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (
            status,
            Json(ErrorResponse {
                error: "processing_error".to_string(),
                message: e.to_string(),
                details: None,
            }),
        )
    })?;

    Ok(Json(ObserveResponse {
        agent_id: agent_id.clone(),
        response,
        timestamp: chrono::Utc::now(),
    }))
}

/// DELETE /agents/{agent_id} - Remove an agent
#[utoipa::path(
    delete,
    path = "/agents/{agent_id}",
    params(
        ("agent_id" = String, Path, description = "Agent identifier")
    ),
    responses(
        (status = 204, description = "Agent deleted successfully"),
        (status = 404, description = "Agent not found", body = ErrorResponse),
        (status = 401, description = "Authentication required", body = auth::AuthError)
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    )
)]
async fn delete_agent<T: ToolRegistry + Clone + Send + Sync>(
    State(runtime): State<HttpAgentRuntime<T>>,
    Path(agent_id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let mut agents = runtime.agents.write().await;

    match agents.remove(&agent_id) {
        Some(_) => Ok(StatusCode::NO_CONTENT),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "agent_not_found".to_string(),
                message: format!("Agent with ID '{}' not found", agent_id),
                details: None,
            }),
        )),
    }
}

/// GET /health - Basic health check endpoint
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy", body = serde_json::Value)
    )
)]
async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "skreaver-http-runtime",
        "timestamp": chrono::Utc::now(),
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// GET /ready - Kubernetes readiness check with detailed component health
#[utoipa::path(
    get,
    path = "/ready",
    responses(
        (status = 200, description = "Service is ready", body = SystemHealth),
        (status = 503, description = "Service not ready", body = SystemHealth)
    )
)]
async fn readiness_check<T: ToolRegistry + Clone + Send + Sync>(
    State(_runtime): State<HttpAgentRuntime<T>>,
) -> Result<Json<SystemHealth>, (StatusCode, Json<SystemHealth>)> {
    // Create health checker with basic components
    let mut health_checker = HealthChecker::new();

    // Add basic health checks
    health_checker.register("metrics_registry".to_string(), MetricsHealthCheck::new());

    // Perform all health checks
    let system_health = health_checker.check_all().await;

    // Return appropriate HTTP status based on health
    let status_code = StatusCode::from_u16(system_health.status.as_http_status())
        .unwrap_or(StatusCode::SERVICE_UNAVAILABLE);

    if system_health.status.is_healthy() {
        Ok(Json(system_health))
    } else {
        Err((status_code, Json(system_health)))
    }
}

/// GET /metrics - Prometheus metrics endpoint
#[utoipa::path(
    get,
    path = "/metrics",
    responses(
        (status = 200, description = "Prometheus metrics", content_type = "text/plain"),
        (status = 500, description = "Metrics collection failed")
    )
)]
async fn metrics_endpoint() -> Result<String, (StatusCode, String)> {
    match get_metrics_registry() {
        Some(registry) => {
            let prometheus_registry = registry.prometheus_registry();
            let encoder = prometheus::TextEncoder::new();
            let metric_families = prometheus_registry.gather();

            match encoder.encode_to_string(&metric_families) {
                Ok(metrics) => Ok(metrics),
                Err(e) => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to encode metrics: {}", e),
                )),
            }
        }
        None => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Metrics registry not initialized".to_string(),
        )),
    }
}

/// Health check for metrics registry
struct MetricsHealthCheck;

impl MetricsHealthCheck {
    fn new() -> Self {
        Self
    }
}

#[async_trait]
impl HealthCheck for MetricsHealthCheck {
    async fn check(&self) -> Result<(), String> {
        match get_metrics_registry() {
            Some(_) => Ok(()),
            None => Err("Metrics registry not initialized".to_string()),
        }
    }
}

/// POST /auth/token - Create JWT token for testing
#[utoipa::path(
    post,
    path = "/auth/token",
    request_body = CreateTokenRequest,
    responses(
        (status = 200, description = "JWT token created successfully", body = CreateTokenResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse)
    )
)]
async fn create_token(
    Json(request): Json<CreateTokenRequest>,
) -> Result<Json<CreateTokenResponse>, (StatusCode, Json<ErrorResponse>)> {
    match auth::create_jwt_token(request.user_id, request.permissions) {
        Ok(token) => Ok(Json(CreateTokenResponse {
            token,
            expires_in: 86400, // 24 hours
            token_type: "Bearer".to_string(),
        })),
        Err(_) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "token_creation_failed".to_string(),
                message: "Failed to create JWT token".to_string(),
                details: None,
            }),
        )),
    }
}

/// GET /agents/{agent_id}/stream - Stream agent execution in real-time
#[utoipa::path(
    get,
    path = "/agents/{agent_id}/stream",
    params(
        ("agent_id" = String, Path, description = "Agent identifier"),
        ("input" = Option<String>, Query, description = "Optional input to send to agent"),
        ("debug" = Option<bool>, Query, description = "Include debug information in stream"),
        ("timeout_seconds" = Option<u64>, Query, description = "Custom timeout in seconds")
    ),
    responses(
        (status = 200, description = "Server-Sent Events stream of agent updates"),
        (status = 404, description = "Agent not found", body = ErrorResponse),
        (status = 401, description = "Authentication required", body = auth::AuthError)
    )
)]
async fn stream_agent<T: ToolRegistry + Clone + Send + Sync>(
    State(runtime): State<HttpAgentRuntime<T>>,
    Path(agent_id): Path<String>,
    Query(params): Query<StreamRequest>,
) -> Result<
    Sse<impl Stream<Item = Result<axum::response::sse::Event, axum::BoxError>>>,
    (StatusCode, Json<ErrorResponse>),
> {
    // Verify agent exists
    {
        let agents = runtime.agents.read().await;
        if !agents.contains_key(&agent_id) {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "agent_not_found".to_string(),
                    message: format!("Agent with ID '{}' not found", agent_id),
                    details: None,
                }),
            ));
        }
    }

    // Create streaming executor
    let (executor, receiver) = StreamingAgentExecutor::new();

    // Start background task to execute agent with input if provided
    if let Some(input) = params.input {
        let runtime_clone = runtime.clone();
        let agent_id_clone = agent_id.clone();
        let debug = params.debug;
        let timeout = params.timeout_seconds.unwrap_or(300); // Default 5 minutes

        tokio::spawn(async move {
            let agent_id_for_timeout = agent_id_clone.clone();

            // Apply timeout to the entire operation
            let execution_result =
                tokio::time::timeout(std::time::Duration::from_secs(timeout), async {
                    executor
                        .execute_with_streaming(agent_id_clone.clone(), |exec| async move {
                            if debug {
                                exec.thinking(&agent_id_clone, "Starting agent processing")
                                    .await;
                                exec.partial(
                                    &agent_id_clone,
                                    &format!(
                                        "Debug: Processing input of {} characters",
                                        input.len()
                                    ),
                                )
                                .await;
                            }

                            exec.thinking(&agent_id_clone, "Processing observation")
                                .await;

                            // Access the agent instance here
                            let response = {
                                let mut agents = runtime_clone.agents.write().await;
                                if let Some(instance) = agents.get_mut(&agent_id_clone) {
                                    let response = instance.coordinator.step(input);
                                    drop(agents); // Release lock immediately
                                    Ok(response)
                                } else {
                                    Err("Agent not found".to_string())
                                }
                            }?;

                            if debug {
                                exec.partial(
                                    &agent_id_clone,
                                    &format!(
                                        "Debug: Generated response of {} characters",
                                        response.len()
                                    ),
                                )
                                .await;
                            }

                            exec.partial(&agent_id_clone, &response).await;
                            Ok(response)
                        })
                        .await
                })
                .await;

            // Handle timeout
            if execution_result.is_err() {
                let _ = executor
                    .send_update(streaming::AgentUpdate::Error {
                        agent_id: agent_id_for_timeout,
                        error: format!("Operation timed out after {} seconds", timeout),
                        timestamp: chrono::Utc::now(),
                    })
                    .await;
            }
        });
    } else {
        // Send a status ping for connection health
        let agent_id_clone = agent_id.clone();
        tokio::spawn(async move {
            let _ = executor
                .send_update(streaming::AgentUpdate::Ping {
                    timestamp: chrono::Utc::now(),
                })
                .await;

            // Send initial status for this agent
            let _ = executor
                .send_update(streaming::AgentUpdate::Started {
                    agent_id: agent_id_clone,
                    timestamp: chrono::Utc::now(),
                })
                .await;
        });
    }

    // Return SSE stream
    Ok(streaming::create_sse_stream(receiver))
}

/// POST /agents/{agent_id}/observe/stream - Stream agent observation in real-time
#[utoipa::path(
    post,
    path = "/agents/{agent_id}/observe/stream",
    params(
        ("agent_id" = String, Path, description = "Agent identifier")
    ),
    request_body = ObserveRequest,
    responses(
        (status = 200, description = "Server-Sent Events stream of agent processing"),
        (status = 404, description = "Agent not found", body = ErrorResponse),
        (status = 401, description = "Authentication required", body = auth::AuthError)
    )
)]
async fn observe_agent_stream<T: ToolRegistry + Clone + Send + Sync>(
    State(runtime): State<HttpAgentRuntime<T>>,
    Path(agent_id): Path<String>,
    Json(request): Json<ObserveRequest>,
) -> Result<
    Sse<impl Stream<Item = Result<axum::response::sse::Event, axum::BoxError>>>,
    (StatusCode, Json<ErrorResponse>),
> {
    // Verify agent exists
    {
        let agents = runtime.agents.read().await;
        if !agents.contains_key(&agent_id) {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "agent_not_found".to_string(),
                    message: format!("Agent with ID '{}' not found", agent_id),
                    details: None,
                }),
            ));
        }
    }

    // Create streaming executor
    let (executor, receiver) = streaming::StreamingAgentExecutor::new();

    // Start background task to process observation
    let runtime_clone = runtime.clone();
    let agent_id_clone = agent_id.clone();
    let input = request.input;

    tokio::spawn(async move {
        let mut agents = runtime_clone.agents.write().await;
        if let Some(instance) = agents.get_mut(&agent_id_clone) {
            let _result = executor
                .execute_with_streaming(agent_id_clone.clone(), |exec| async move {
                    exec.thinking(&agent_id_clone, "Analyzing input").await;
                    let response = instance.coordinator.step(input);
                    exec.partial(&agent_id_clone, &response).await;
                    Ok(response)
                })
                .await;
        }
    });

    // Return SSE stream
    Ok(streaming::create_sse_stream(receiver))
}

/// POST /agents/{agent_id}/batch - Process multiple observations in batch
#[utoipa::path(
    post,
    path = "/agents/{agent_id}/batch",
    params(
        ("agent_id" = String, Path, description = "Agent identifier")
    ),
    request_body = BatchObserveRequest,
    responses(
        (status = 200, description = "Batch processing results", body = BatchObserveResponse),
        (status = 404, description = "Agent not found", body = ErrorResponse),
        (status = 401, description = "Authentication required", body = auth::AuthError)
    )
)]
async fn batch_observe_agent<T: ToolRegistry + Clone + Send + Sync>(
    State(runtime): State<HttpAgentRuntime<T>>,
    Path(agent_id): Path<String>,
    Json(request): Json<BatchObserveRequest>,
) -> Result<Json<BatchObserveResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    // Verify agent exists
    {
        let agents = runtime.agents.read().await;
        if !agents.contains_key(&agent_id) {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "agent_not_found".to_string(),
                    message: format!("Agent with ID '{}' not found", agent_id),
                    details: None,
                }),
            ));
        }
    }

    // Validate batch size
    if request.inputs.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "empty_batch".to_string(),
                message: "Batch request must contain at least one input".to_string(),
                details: None,
            }),
        ));
    }

    if request.inputs.len() > 100 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "batch_too_large".to_string(),
                message: "Batch size cannot exceed 100 inputs".to_string(),
                details: None,
            }),
        ));
    }

    // Process inputs with semaphore for concurrency control
    let semaphore = Arc::new(tokio::sync::Semaphore::new(request.parallel_limit));
    let results = Arc::new(tokio::sync::Mutex::new(vec![
        BatchResult {
            index: 0,
            success: false,
            response: None,
            error: None,
            time_ms: 0,
        };
        request.inputs.len()
    ]));

    let mut handles = Vec::new();

    for (index, input) in request.inputs.into_iter().enumerate() {
        let permit = semaphore.clone().acquire_owned().await.map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "semaphore_error".to_string(),
                    message: "Failed to acquire processing permit".to_string(),
                    details: None,
                }),
            )
        })?;
        let runtime_clone = runtime.clone();
        let agent_id_clone = agent_id.clone();
        let results_clone = Arc::clone(&results);
        let timeout_duration = std::time::Duration::from_secs(request.timeout_seconds);

        let handle = tokio::spawn(async move {
            let _permit = permit; // Hold the permit for the duration of this task
            let op_start = std::time::Instant::now();

            let result = tokio::time::timeout(timeout_duration, async {
                // Minimize lock scope within timeout
                let mut agents = runtime_clone.agents.write().await;
                if let Some(instance) = agents.get_mut(&agent_id_clone) {
                    let response = instance.coordinator.step(input);
                    drop(agents); // Release lock immediately after step
                    Ok(response)
                } else {
                    drop(agents); // Release lock even when agent not found
                    Err("Agent not found".to_string())
                }
            })
            .await;

            let batch_result = match result {
                Ok(Ok(response)) => BatchResult {
                    index,
                    success: true,
                    response: Some(response),
                    error: None,
                    time_ms: op_start.elapsed().as_millis() as u64,
                },
                Ok(Err(error)) => BatchResult {
                    index,
                    success: false,
                    response: None,
                    error: Some(error),
                    time_ms: op_start.elapsed().as_millis() as u64,
                },
                Err(_) => BatchResult {
                    index,
                    success: false,
                    response: None,
                    error: Some("Operation timed out".to_string()),
                    time_ms: timeout_duration.as_millis() as u64,
                },
            };

            let mut results_guard = results_clone.lock().await;
            results_guard[index] = batch_result;
        });

        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        let _ = handle.await;
    }

    let results = Arc::try_unwrap(results)
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "internal_error".to_string(),
                    message: "Failed to collect batch results".to_string(),
                    details: None,
                }),
            )
        })?
        .into_inner();
    let total_time = start_time.elapsed().as_millis() as u64;

    Ok(Json(BatchObserveResponse {
        agent_id,
        results,
        total_time_ms: total_time,
        timestamp: chrono::Utc::now(),
    }))
}

/// GET /agents/{agent_id}/queue/metrics - Get queue metrics for specific agent
#[utoipa::path(
    get,
    path = "/agents/{agent_id}/queue/metrics",
    params(
        ("agent_id" = String, Path, description = "Agent identifier")
    ),
    responses(
        (status = 200, description = "Agent queue metrics", body = QueueMetricsResponse),
        (status = 404, description = "Agent not found", body = ErrorResponse),
        (status = 401, description = "Authentication required", body = auth::AuthError)
    )
)]
async fn get_agent_queue_metrics<T: ToolRegistry + Clone + Send + Sync>(
    State(runtime): State<HttpAgentRuntime<T>>,
    Path(agent_id): Path<String>,
) -> Result<Json<QueueMetricsResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Verify agent exists
    {
        let agents = runtime.agents.read().await;
        if !agents.contains_key(&agent_id) {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "agent_not_found".to_string(),
                    message: format!("Agent with ID '{}' not found", agent_id),
                    details: None,
                }),
            ));
        }
    }

    let metrics = runtime
        .backpressure_manager
        .get_agent_metrics(&agent_id)
        .await
        .unwrap_or_else(|| QueueMetrics {
            queue_size: 0,
            active_requests: 0,
            total_processed: 0,
            total_timeouts: 0,
            total_rejections: 0,
            avg_processing_time_ms: 0.0,
            load_factor: 0.0,
        });

    Ok(Json(QueueMetricsResponse {
        agent_id: Some(agent_id),
        queue_size: metrics.queue_size,
        active_requests: metrics.active_requests,
        total_processed: metrics.total_processed,
        total_timeouts: metrics.total_timeouts,
        total_rejections: metrics.total_rejections,
        avg_processing_time_ms: metrics.avg_processing_time_ms,
        load_factor: metrics.load_factor,
        timestamp: chrono::Utc::now(),
    }))
}

/// GET /queue/metrics - Get global queue metrics
#[utoipa::path(
    get,
    path = "/queue/metrics",
    responses(
        (status = 200, description = "Global queue metrics", body = QueueMetricsResponse),
        (status = 401, description = "Authentication required", body = auth::AuthError)
    )
)]
async fn get_global_queue_metrics<T: ToolRegistry + Clone + Send + Sync>(
    State(runtime): State<HttpAgentRuntime<T>>,
) -> Json<QueueMetricsResponse> {
    let metrics = runtime.backpressure_manager.get_global_metrics().await;

    Json(QueueMetricsResponse {
        agent_id: None,
        queue_size: metrics.queue_size,
        active_requests: metrics.active_requests,
        total_processed: metrics.total_processed,
        total_timeouts: metrics.total_timeouts,
        total_rejections: metrics.total_rejections,
        avg_processing_time_ms: metrics.avg_processing_time_ms,
        load_factor: metrics.load_factor,
        timestamp: chrono::Utc::now(),
    })
}

/// Create OpenAPI documentation router
fn create_openapi_router() -> Router {
    Router::new()
        .route("/docs", get(swagger_ui))
        .route("/api-docs/openapi.json", get(openapi_spec))
}

/// Swagger UI handler
async fn swagger_ui() -> Html<&'static str> {
    Html(
        r#"
<!DOCTYPE html>
<html>
<head>
    <title>Skreaver API Documentation</title>
    <link rel="stylesheet" type="text/css" href="https://unpkg.com/swagger-ui-dist@3.25.0/swagger-ui.css" />
</head>
<body>
    <div id="swagger-ui"></div>
    <script src="https://unpkg.com/swagger-ui-dist@3.25.0/swagger-ui-bundle.js"></script>
    <script>
        SwaggerUIBundle({
            url: '/api-docs/openapi.json',
            dom_id: '#swagger-ui',
            presets: [
                SwaggerUIBundle.presets.apis,
                SwaggerUIBundle.presets.standalone
            ]
        });
    </script>
</body>
</html>
        "#,
    )
}

/// OpenAPI specification endpoint
async fn openapi_spec() -> Json<utoipa::openapi::OpenApi> {
    use utoipa::OpenApi;

    #[derive(OpenApi)]
    #[openapi(
        paths(
            health_check,
            readiness_check,
            metrics_endpoint,
            create_token,
            list_agents,
            create_agent,
            get_agent_status,
            observe_agent,
            stream_agent,
            delete_agent,
            get_agent_queue_metrics,
            get_global_queue_metrics
        ),
        components(
            schemas(
                CreateAgentRequest,
                CreateAgentResponse,
                ObserveRequest,
                ObserveResponse,
                AgentStatus,
                AgentsListResponse,
                ErrorResponse,
                CreateTokenRequest,
                CreateTokenResponse,
                QueueMetricsResponse
            )
        ),
        tags(
            (name = "agents", description = "Agent management endpoints"),
            (name = "auth", description = "Authentication endpoints"),
            (name = "health", description = "Health check endpoints")
        ),
        info(
            title = "Skreaver HTTP Runtime API",
            version = "0.1.0",
            description = "Production-ready HTTP API for Skreaver agent framework with authentication, rate limiting, and streaming capabilities"
        ),
        servers(
            (url = "http://localhost:3000", description = "Local development server")
        )
    )]
    struct ApiDoc;

    Json(ApiDoc::openapi())
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
            assert!(result["time_ms"].as_u64().is_some());
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
}
