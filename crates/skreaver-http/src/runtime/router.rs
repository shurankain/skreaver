//! HTTP router configuration
//!
//! This module provides router setup and route registration for the HTTP runtime.

use axum::{
    Router, middleware,
    routing::{get, post},
};
use skreaver_tools::ToolRegistry;
use std::sync::Arc;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::runtime::{
    HttpAgentRuntime, HttpRuntimeConfig,
    auth::{inject_api_key_manager, require_auth},
    connection_limits::connection_limit_middleware,
    docs::{openapi_spec, swagger_ui},
    error::request_id_middleware,
    handlers::{
        batch_observe_agent,
        create_agent,
        // Authentication
        create_token,
        delete_agent,
        // Queue metrics
        get_agent_queue_metrics,
        get_agent_status,
        get_global_queue_metrics,
        // Health and metrics
        health_check,
        // Agents
        list_agents,
        metrics_endpoint,
        // Observations
        observe_agent,
        observe_agent_stream,
        readiness_check,
        stream_agent,
    },
};

impl<T: ToolRegistry + Clone + Send + Sync + 'static> HttpAgentRuntime<T> {
    /// Create the Axum router with all endpoints and middleware
    pub fn router(self) -> Router {
        self.router_with_config(HttpRuntimeConfig::default())
    }

    /// Create the Axum router with custom configuration
    pub fn router_with_config(self, config: HttpRuntimeConfig) -> Router {
        // Clone connection tracker and API key manager for middleware
        let connection_tracker = Arc::clone(&self.connection_tracker);
        let api_key_manager = Arc::clone(&self.api_key_manager);

        // Protected routes - require authentication
        // Use route_layer to apply middleware to specific routes before merging
        let protected_routes = Router::new()
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
            .route_layer(middleware::from_fn(require_auth)); // Apply auth to these routes only

        // Public routes - no authentication required
        let public_routes = Router::new()
            .route("/health", get(health_check))
            .route("/ready", get(readiness_check))
            .route("/metrics", get(metrics_endpoint))
            .route("/auth/token", post(create_token));

        // Combine public and protected routes
        let mut router = Router::new()
            .merge(public_routes)
            .merge(protected_routes)
            .with_state(self)
            .layer(TraceLayer::new_for_http());

        // Add request ID middleware (applies to all routes, should be early in the stack)
        router = router.layer(middleware::from_fn(request_id_middleware));

        // Add API key manager injection middleware (applies to all routes)
        router = router.layer(middleware::from_fn_with_state(
            api_key_manager,
            inject_api_key_manager,
        ));

        // Add connection limit middleware (applies to all routes)
        // Use from_fn_with_state to pass the tracker as state
        router = router.layer(middleware::from_fn_with_state(
            connection_tracker,
            connection_limit_middleware,
        ));

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
}

/// Create OpenAPI documentation router
fn create_openapi_router() -> Router {
    Router::new()
        .route("/docs", get(swagger_ui))
        .route("/api-docs/openapi.json", get(openapi_spec))
}
