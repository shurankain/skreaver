//! # Skreaver HTTP Runtime
//!
//! This crate provides HTTP runtime capabilities for the Skreaver agent infrastructure,
//! including web server functionality, authentication, streaming, and OpenAPI documentation.
//!
//! ## Features
//!
//! - **Core HTTP Runtime**: Basic HTTP server functionality using Axum
//! - **Authentication** (`auth`): JWT-based authentication support
//! - **OpenAPI Documentation** (`openapi`): API documentation generation
//! - **OpenAPI UI** (`openapi-ui`): Swagger UI for API documentation
//! - **Compression** (`compression`): HTTP compression middleware
//! - **Streaming** (`streaming`): Server-sent events and streaming support
//! - **WebSocket** (`unstable-websocket`): WebSocket support (unstable)

pub mod runtime;

#[cfg(feature = "openapi")]
pub mod openapi;

#[cfg(feature = "unstable-websocket")]
pub mod websocket;

// Re-export main types for public API
pub use runtime::*;
pub use skreaver_tools::*;

#[cfg(feature = "openapi")]
pub use openapi::*;

// Note: websocket module has its own 'handlers' module that conflicts with runtime::handlers
// We selectively re-export websocket types to avoid ambiguity
#[cfg(feature = "unstable-websocket")]
pub use websocket::{
    ConnectionInfo, WebSocketConfig, WebSocketManager, WsError, WsMessage, handlers as ws_handlers,
    protocol,
};
