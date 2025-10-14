//! # Skreaver Mesh
//!
//! Multi-agent communication layer for Skreaver agent systems.
//!
//! This crate provides reliable messaging infrastructure for agent-to-agent
//! communication, enabling complex multi-agent workflows and coordination patterns.
//!
//! ## Features
//!
//! - **Typed Messages**: Strongly-typed message schemas with automatic serialization
//! - **Pub/Sub Patterns**: Point-to-point, broadcast, and topic-based messaging
//! - **Backpressure**: Queue depth monitoring and flow control
//! - **Reliability**: Dead letter queues and retry mechanisms
//! - **Observability**: Built-in metrics and tracing
//!
//! ## Example
//!
//! ```rust,no_run
//! use skreaver_mesh::{AgentMesh, Message, AgentId, Topic};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create mesh (Redis backend)
//!     # #[cfg(feature = "redis")]
//!     # {
//!     let mesh = skreaver_mesh::RedisMesh::new("redis://localhost:6379").await?;
//!
//!     // Send point-to-point message
//!     let msg = Message::new("hello".to_string())
//!         .with_metadata("priority", "high");
//!     mesh.send(&AgentId::from("agent-2"), msg).await?;
//!
//!     // Broadcast to all agents
//!     let broadcast_msg = Message::new("shutdown".to_string());
//!     mesh.broadcast(broadcast_msg).await?;
//!
//!     // Subscribe to topic
//!     let mut stream = mesh.subscribe(&Topic::from("notifications")).await?;
//!     # }
//!     Ok(())
//! }
//! ```

pub mod backpressure;
pub mod dlq;
pub mod error;
pub mod mesh;
pub mod message;
pub mod metrics;
pub mod patterns;
pub mod types;

#[cfg(feature = "redis")]
pub mod redis;

pub use backpressure::{
    BackpressureConfig, BackpressureMonitor, BackpressureQueue, BackpressureSignal,
    BackpressureStats,
};
pub use dlq::{DeadLetterQueue, DlqConfig, DlqEntry, DlqStats};
pub use error::{MeshError, MeshResult};
pub use mesh::AgentMesh;
pub use message::{
    AnonymousRoute, BroadcastRoute, Message, MessageBuilder, MessageId, MessageMetadata,
    MessagePayload, Route, SystemRoute, TypedMessage, UnicastRoute, Unrouted,
};
pub use metrics::{MeshMetrics, MeshMetricsCollector};
pub use patterns::{
    BroadcastGather, GatherConfig, GatherResult, Pipeline, PipelineStage, RequestReply,
    RequestReplyConfig, Supervisor, SupervisorConfig, TaskStatus, WorkerPool,
};
pub use types::{AgentId, IdValidationError, Topic};

#[cfg(feature = "redis")]
pub use redis::RedisMesh;
