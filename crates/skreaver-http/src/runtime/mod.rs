//! # Runtime Module
//!
//! This module provides the execution runtime for Skreaver agents. The runtime
//! orchestrates the interaction between agents, tools, and memory systems,
//! managing the complete lifecycle of agent operations.
//!
//! ## Core Component
//!
//! - **[Coordinator]**: Central runtime that manages agent execution, tool dispatch,
//!   and memory operations in a coordinated manner
//!
//! ## Responsibilities
//!
//! - **Agent Execution**: Drives the agent observation-action cycle
//! - **Tool Orchestration**: Routes tool calls to appropriate implementations  
//! - **Memory Management**: Ensures state persistence across interactions
//! - **Error Handling**: Manages failures in tool execution and memory operations
//!
//! ## Usage Pattern
//!
//! ```rust
//! use skreaver_core::{Agent, MemoryUpdate};
//! use skreaver_core::InMemoryMemory;
//! use skreaver_core::memory::{MemoryReader, MemoryWriter};
//! use skreaver_tools::{InMemoryToolRegistry, ExecutionResult, ToolCall};
//! use skreaver_http::runtime::Coordinator;
//!
//! // Example agent implementation
//! struct SimpleAgent {
//!     memory: InMemoryMemory,
//! }
//!
//! impl Agent for SimpleAgent {
//!     type Observation = String;
//!     type Action = String;
//!     
//!     fn memory_reader(&self) -> &dyn MemoryReader { &self.memory }
//!     fn memory_writer(&mut self) -> &mut dyn MemoryWriter { &mut self.memory }
//!     fn observe(&mut self, _input: String) {}
//!     fn act(&mut self) -> String { "response".to_string() }
//!     fn call_tools(&self) -> Vec<ToolCall> { Vec::new() }
//!     fn handle_result(&mut self, _result: ExecutionResult) {}
//!     fn update_context(&mut self, update: MemoryUpdate) { let _ = self.memory_writer().store(update); }
//! }
//!
//! // Create agent and coordinate execution
//! let agent = SimpleAgent { memory: InMemoryMemory::new() };
//! let registry = InMemoryToolRegistry::new();
//! let mut coordinator = Coordinator::new(agent, registry);
//! let result = coordinator.step("user input".to_string());
//! ```

/// Concrete agent builders for standard agent types.
pub mod agent_builders;
/// Agent factory pattern for dynamic agent creation.
pub mod agent_factory;
/// Agent instance management with state tracking.
pub mod agent_instance;
/// Type-safe agent status management.
pub mod agent_status;
/// Improved API types with type safety and validation.
pub mod api_types;
/// Authentication middleware for HTTP runtime.
pub mod auth;
/// Backpressure and request queue management.
pub mod backpressure;
/// Central coordinator for agent execution and tool dispatch.
pub mod coordinator;
/// API documentation endpoints.
pub mod docs;
/// Unified error handling system.
pub mod errors;
/// HTTP request handlers organized by functionality.
pub mod handlers;
/// HTTP runtime for serving agents over REST API.
pub mod http;
/// Rate limiting middleware for HTTP runtime.
pub mod rate_limit;
/// HTTP router configuration and route registration.
pub mod router;
/// Security management and input validation.
pub mod security;
/// Streaming responses for long-running operations.
pub mod streaming;
/// Type definitions for HTTP runtime (requests, responses, etc.).
pub mod types;

pub use agent_builders::{AdvancedAgentBuilder, AnalyticsAgentBuilder, EchoAgentBuilder};
pub use agent_factory::{AgentBuilder, AgentFactory, AgentFactoryError};
pub use agent_instance::{AgentId, AgentInstance, CoordinatorTrait};
pub use agent_status::{AgentStatus, AgentStatusEnum, AgentStatusManager};
pub use api_types::{
    AgentObservation, AgentResponse, AgentSpec, AgentType, DeliveryError, ResponseDelivery,
};
pub use backpressure::{BackpressureConfig, BackpressureManager, QueueMetrics, RequestPriority};
pub use coordinator::Coordinator;
pub use errors::{ErrorResponse, RequestId, RuntimeError, RuntimeResult};
pub use http::{HttpAgentRuntime, HttpRuntimeConfig};
pub use security::{ApiKeyData, SecretKey, SecurityConfig};
