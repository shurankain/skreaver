//! API documentation handlers
//!
//! This module provides OpenAPI specification and Swagger UI endpoints
//! for interactive API documentation.

use axum::{response::{Html, Json}};
use utoipa::OpenApi;

use crate::runtime::types::{
    CreateAgentRequest, CreateAgentResponse, ObserveRequest, ObserveResponse,
    AgentStatus, AgentsListResponse, ErrorResponse, CreateTokenRequest,
    CreateTokenResponse, QueueMetricsResponse
};

/// GET /docs - Swagger UI for interactive API documentation
pub async fn swagger_ui() -> Html<&'static str> {
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

/// GET /api-docs/openapi.json - OpenAPI specification endpoint
pub async fn openapi_spec() -> Json<utoipa::openapi::OpenApi> {
    #[derive(OpenApi)]
    #[openapi(
        paths(
            crate::runtime::handlers::health_check,
            crate::runtime::handlers::readiness_check,
            crate::runtime::handlers::metrics_endpoint,
            crate::runtime::handlers::create_token,
            crate::runtime::handlers::list_agents,
            crate::runtime::handlers::create_agent,
            crate::runtime::handlers::get_agent_status,
            crate::runtime::handlers::delete_agent,
            crate::runtime::handlers::get_agent_queue_metrics,
            crate::runtime::handlers::get_global_queue_metrics
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
            (name = "health", description = "Health check endpoints"),
            (name = "metrics", description = "Queue metrics endpoints")
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