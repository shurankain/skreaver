//! Streaming event types for the A2A protocol.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{Artifact, Message, TaskStatus};

/// Event for task status updates during streaming
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatusUpdateEvent {
    /// Task ID
    pub task_id: String,

    /// New status
    pub status: TaskStatus,

    /// Optional message with the update
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,

    /// Timestamp of the event
    pub timestamp: DateTime<Utc>,
}

/// Event for artifact updates during streaming
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskArtifactUpdateEvent {
    /// Task ID
    pub task_id: String,

    /// The artifact being added or updated
    pub artifact: Artifact,

    /// Whether this is the final update for this artifact
    #[serde(default)]
    pub is_final: bool,

    /// Timestamp of the event
    pub timestamp: DateTime<Utc>,
}

/// Unified streaming event type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum StreamingEvent {
    /// Task status update
    #[serde(rename = "taskStatusUpdate")]
    TaskStatusUpdate(TaskStatusUpdateEvent),

    /// Artifact update
    #[serde(rename = "taskArtifactUpdate")]
    TaskArtifactUpdate(TaskArtifactUpdateEvent),
}
