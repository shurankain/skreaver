//! # Skreaver Guardrails
//!
//! Runtime guardrails for AI agent execution. This crate provides
//! configurable pre-execution checks that wrap any `UnifiedAgent`
//! with policy enforcement: tool allowlists/denylists, message size
//! limits, and composable guardrail layers.
//!
//! Built on top of `skreaver-core::security` (which handles low-level
//! input validation, resource limits, and audit logging).
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use skreaver_guardrails::{GuardedAgentBuilder, ToolFilter};
//!
//! let guarded = GuardedAgentBuilder::new(my_agent)
//!     .allow_tools(["http_get", "file_read"])
//!     .max_message_size(1024 * 1024) // 1 MB
//!     .build();
//!
//! // Use `guarded` as a normal UnifiedAgent — guardrails are transparent.
//! let task = guarded.send_message(message).await?;
//! ```

pub mod agent;
pub mod builder;
pub mod config;
pub mod error;
pub mod policy;

pub use agent::GuardedAgent;
pub use builder::GuardedAgentBuilder;
pub use config::GuardrailConfig;
pub use error::{GuardrailError, GuardrailResult};
pub use policy::{GuardrailPolicy, ToolFilter};
