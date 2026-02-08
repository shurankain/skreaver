//! Request and response types for the A2A protocol.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{Message, Task};

/// Request to send a message and create/continue a task
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageRequest {
    /// The message to send
    pub message: Message,

    /// Optional task ID to continue an existing task
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,

    /// Optional context ID for grouping tasks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_id: Option<String>,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Response containing the task state after sending a message
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageResponse {
    /// The updated task
    pub task: Task,
}

/// Request to get a task by ID
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTaskRequest {
    /// Task ID
    pub task_id: String,
}

/// Request to cancel a task
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelTaskRequest {
    /// Task ID
    pub task_id: String,

    /// Optional reason for cancellation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Push notification configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushNotificationConfig {
    /// Configuration ID
    pub id: String,

    /// Webhook URL to receive notifications
    pub webhook_url: String,

    /// Events to notify about
    #[serde(default)]
    pub events: Vec<String>,

    /// Optional secret for webhook verification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
}
