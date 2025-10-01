//! Coordination patterns for multi-agent workflows
//!
//! This module provides high-level abstractions for common multi-agent
//! communication patterns, making it easier to build complex workflows.

pub mod broadcast_gather;
pub mod pipeline;
pub mod request_reply;
pub mod supervisor;

pub use broadcast_gather::{BroadcastGather, GatherConfig, GatherResult};
pub use pipeline::{Pipeline, PipelineStage};
pub use request_reply::{RequestReply, RequestReplyConfig};
pub use supervisor::{Supervisor, SupervisorConfig, TaskStatus, WorkerPool};
