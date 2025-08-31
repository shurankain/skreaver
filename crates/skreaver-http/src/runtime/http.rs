//! # HTTP Runtime
//!
//! This module provides a production-ready HTTP server runtime for Skreaver agents,
//! enabling secure remote interaction through RESTful APIs with authentication,
//! rate limiting, and streaming capabilities. The runtime manages agent lifecycle,
//! handles observations, and provides real-time status information.

use crate::runtime::{
    Coordinator, auth,
    rate_limit::{RateLimitConfig, RateLimitState},
    streaming::{self, StreamingAgentExecutor},
};
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
}

/// HTTP runtime configuration
#[derive(Debug, Clone)]
pub struct HttpRuntimeConfig {
    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,
    /// Request timeout in seconds
    pub request_timeout_secs: u64,
    /// Maximum request body size in bytes
    pub max_body_size: usize,
    /// Enable CORS for cross-origin requests
    pub enable_cors: bool,
    /// Enable OpenAPI documentation endpoint
    pub enable_openapi: bool,
}

impl Default for HttpRuntimeConfig {
    fn default() -> Self {
        Self {
            rate_limit: RateLimitConfig::default(),
            request_timeout_secs: 30,
            max_body_size: 16 * 1024 * 1024, // 16MB
            enable_cors: true,
            enable_openapi: true,
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
}

impl<T: ToolRegistry + Clone + Send + Sync + 'static> HttpAgentRuntime<T> {
    /// Create a new HTTP agent runtime with default configuration
    pub fn new(tool_registry: T) -> Self {
        Self::with_config(tool_registry, HttpRuntimeConfig::default())
    }

    /// Create a new HTTP agent runtime with custom configuration
    pub fn with_config(tool_registry: T, config: HttpRuntimeConfig) -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            tool_registry: Arc::new(tool_registry),
            rate_limit_state: Arc::new(RateLimitState::new(config.rate_limit)),
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
            .route("/auth/token", post(create_token))
            // Protected endpoints (require authentication)
            .route("/agents", get(list_agents).post(create_agent))
            .route("/agents/:agent_id/status", get(get_agent_status))
            .route("/agents/:agent_id/observe", post(observe_agent))
            .route("/agents/:agent_id/stream", get(stream_agent))
            .route("/agents/:agent_id", axum::routing::delete(delete_agent))
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
async fn observe_agent<T: ToolRegistry + Clone + Send + Sync>(
    State(runtime): State<HttpAgentRuntime<T>>,
    Path(agent_id): Path<String>,
    Json(request): Json<ObserveRequest>,
) -> Result<Json<ObserveResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mut agents = runtime.agents.write().await;

    match agents.get_mut(&agent_id) {
        Some(instance) => {
            let response = instance.coordinator.step(request.input);

            Ok(Json(ObserveResponse {
                agent_id: agent_id.clone(),
                response,
                timestamp: chrono::Utc::now(),
            }))
        }
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

/// GET /health - Health check endpoint
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
        ("agent_id" = String, Path, description = "Agent identifier")
    ),
    responses(
        (status = 200, description = "Agent execution stream"),
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
        tokio::spawn(async move {
            let mut agents = runtime_clone.agents.write().await;
            if let Some(instance) = agents.get_mut(&agent_id_clone) {
                let _result = executor
                    .execute_with_streaming(agent_id_clone.clone(), |exec| async move {
                        exec.thinking(&agent_id_clone, "Processing observation")
                            .await;
                        let response = instance.coordinator.step(input);
                        exec.partial(&agent_id_clone, &response).await;
                        Ok(response)
                    })
                    .await;
            }
        });
    }

    // Return SSE stream
    Ok(streaming::create_sse_stream(receiver))
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
            create_token,
            list_agents,
            create_agent,
            get_agent_status,
            observe_agent,
            stream_agent,
            delete_agent
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
                CreateTokenResponse
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
    use skreaver_core::{
        Agent, ExecutionResult, InMemoryMemory, MemoryReader, MemoryUpdate, MemoryWriter, ToolCall,
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
        assert_eq!(json["version"], "0.1.0");
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

        assert_eq!(json["openapi"], "3.0.3");
        assert_eq!(json["info"]["title"], "Skreaver HTTP Runtime API");
        assert_eq!(json["info"]["version"], "0.1.0");
        assert!(json["paths"].is_object());
    }
}
