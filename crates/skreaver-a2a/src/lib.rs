//! # Skreaver A2A - Agent2Agent Protocol Integration
//!
//! This crate provides A2A (Agent2Agent) protocol support for Skreaver,
//! enabling interoperability between AI agents using Google's A2A protocol.
//!
//! ## Features
//!
//! - **Core Types**: Task, Message, Artifact, and AgentCard types
//! - **Streaming**: Support for streaming task updates
//! - **A2A Client**: Connect to external A2A agents (requires `client` feature)
//! - **A2A Server**: Expose Skreaver agents via A2A (requires `server` feature)
//!
//! ## Protocol Overview
//!
//! The A2A protocol defines how AI agents discover each other and collaborate:
//!
//! 1. **Agent Card**: A JSON document describing an agent's capabilities
//! 2. **Tasks**: Units of work with lifecycle states
//! 3. **Messages**: Communications between agents
//! 4. **Artifacts**: Outputs produced by tasks
//!
//! ## Example: Creating an Agent Card
//!
//! ```rust
//! use skreaver_a2a::{AgentCard, AgentSkill};
//!
//! let card = AgentCard::new("my-agent", "My Agent", "https://my-agent.example.com")
//!     .with_description("An AI agent that can summarize documents")
//!     .with_streaming()
//!     .with_skill(
//!         AgentSkill::new("summarize", "Summarize Text")
//!             .with_description("Summarizes documents and text content")
//!     );
//! ```
//!
//! ## Example: Working with Tasks
//!
//! ```rust
//! use skreaver_a2a::{Task, Message, TaskStatus};
//!
//! // Create a new task
//! let mut task = Task::new("task-001");
//!
//! // Add a user message
//! task.add_message(Message::user("Please summarize this document..."));
//!
//! // Task is working
//! assert_eq!(task.status, TaskStatus::Working);
//!
//! // Complete the task
//! task.set_status(TaskStatus::Completed);
//! assert!(task.is_terminal());
//! ```

pub mod error;
pub mod types;

// Client module (requires client feature)
#[cfg(feature = "client")]
pub mod client;

// Re-export core types
pub use error::{A2aError, A2aResult, ErrorResponse};
pub use types::{
    // Agent Card types
    AgentCapabilities,
    AgentCard,
    AgentCardSignature,
    AgentExtension,
    AgentInterface,
    AgentProvider,
    AgentSkill,
    ApiKeyLocation,
    // Artifact types
    Artifact,
    // Request/Response types
    CancelTaskRequest,
    // Message types
    DataPart,
    FilePart,
    GetTaskRequest,
    Message,
    OAuth2Flow,
    OAuth2Flows,
    Part,
    PushNotificationConfig,
    Role,
    SecurityScheme,
    SendMessageRequest,
    SendMessageResponse,
    // Streaming types
    StreamingEvent,
    // Task types
    Task,
    TaskArtifactUpdateEvent,
    TaskStatus,
    TaskStatusUpdateEvent,
    TextPart,
};

// Re-export client types
#[cfg(feature = "client")]
pub use client::{A2aClient, AuthConfig};
