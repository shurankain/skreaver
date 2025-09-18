//! HTTP request handlers
//!
//! This module contains all the HTTP endpoint handlers organized by functionality.

pub mod agents;
pub mod auth;
pub mod health;
pub mod metrics;
pub mod observations;

// Re-export handlers for convenience
pub use agents::*;
pub use auth::*;
pub use health::*;
pub use metrics::*;
pub use observations::{batch_observe_agent, observe_agent, observe_agent_stream, stream_agent};
