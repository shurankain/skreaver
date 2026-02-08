//! Task types for the A2A protocol.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::{Artifact, Message};

/// A task represents a unit of work in the A2A protocol.
///
/// Tasks have a lifecycle that progresses through various states, and they
/// contain messages exchanged between agents and artifacts produced as output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    /// Unique identifier for the task
    pub id: String,

    /// Optional context ID for grouping related tasks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_id: Option<String>,

    /// Current status of the task
    pub status: TaskStatus,

    /// Messages exchanged during the task
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<Message>,

    /// Artifacts produced by the task
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<Artifact>,

    /// When the task was created
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,

    /// When the task was last updated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Task {
    /// Create a new task with the given ID
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            context_id: None,
            status: TaskStatus::Working,
            messages: Vec::new(),
            artifacts: Vec::new(),
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
            metadata: HashMap::new(),
        }
    }

    /// Create a new task with a generated UUID
    pub fn new_with_uuid() -> Self {
        Self::new(Uuid::new_v4().to_string())
    }

    /// Add a message to the task
    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
        self.updated_at = Some(Utc::now());
    }

    /// Add an artifact to the task
    pub fn add_artifact(&mut self, artifact: Artifact) {
        self.artifacts.push(artifact);
        self.updated_at = Some(Utc::now());
    }

    /// Update the task status
    pub fn set_status(&mut self, status: TaskStatus) {
        self.status = status;
        self.updated_at = Some(Utc::now());
    }

    /// Check if the task is in a terminal state
    pub fn is_terminal(&self) -> bool {
        self.status.is_terminal()
    }

    /// Check if the task requires input
    pub fn requires_input(&self) -> bool {
        self.status.is_input_required()
    }
}

/// Task status indicating the current state in the task lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskStatus {
    /// Task is actively being processed
    Working,

    /// Task completed successfully
    Completed,

    /// Task failed due to an error
    Failed,

    /// Task was cancelled by the user
    Cancelled,

    /// Task was rejected by the system
    Rejected,

    /// Task requires additional input to proceed
    InputRequired,
}

impl TaskStatus {
    /// Check if this status represents a terminal state.
    ///
    /// Terminal states are final states where no further processing will occur.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            TaskStatus::Completed
                | TaskStatus::Failed
                | TaskStatus::Cancelled
                | TaskStatus::Rejected
        )
    }

    /// Check if this status indicates the task is still in progress.
    pub fn is_in_progress(self) -> bool {
        matches!(self, TaskStatus::Working | TaskStatus::InputRequired)
    }

    /// Check if this status indicates input is required.
    pub fn is_input_required(self) -> bool {
        self == TaskStatus::InputRequired
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Working => write!(f, "working"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Failed => write!(f, "failed"),
            TaskStatus::Cancelled => write!(f, "cancelled"),
            TaskStatus::Rejected => write!(f, "rejected"),
            TaskStatus::InputRequired => write!(f, "input-required"),
        }
    }
}
