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

// Re-export main types for public API
pub use runtime::*;
pub use skreaver_tools::*;
