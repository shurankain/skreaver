//! Message types for the A2A protocol.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::{Part, TextPart};

/// A message exchanged between agents during task execution
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    /// Optional message identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// Role of the message sender
    pub role: Role,

    /// Content parts of the message
    pub parts: Vec<Part>,

    /// References to related tasks
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_task_ids: Vec<String>,

    /// When the message was sent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<DateTime<Utc>>,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Message {
    /// Create a new user message with text content
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            id: Some(Uuid::new_v4().to_string()),
            role: Role::User,
            parts: vec![Part::Text(TextPart {
                text: text.into(),
                metadata: HashMap::new(),
            })],
            reference_task_ids: Vec::new(),
            timestamp: Some(Utc::now()),
            metadata: HashMap::new(),
        }
    }

    /// Create a new agent message with text content
    pub fn agent(text: impl Into<String>) -> Self {
        Self {
            id: Some(Uuid::new_v4().to_string()),
            role: Role::Agent,
            parts: vec![Part::Text(TextPart {
                text: text.into(),
                metadata: HashMap::new(),
            })],
            reference_task_ids: Vec::new(),
            timestamp: Some(Utc::now()),
            metadata: HashMap::new(),
        }
    }

    /// Add a part to the message
    pub fn with_part(mut self, part: Part) -> Self {
        self.parts.push(part);
        self
    }

    /// Add a reference to another task
    pub fn with_reference(mut self, task_id: impl Into<String>) -> Self {
        self.reference_task_ids.push(task_id.into());
        self
    }
}

/// Role of a message sender
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Message from a user (or client agent acting on behalf of a user)
    User,

    /// Message from an agent
    Agent,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::User => write!(f, "user"),
            Role::Agent => write!(f, "agent"),
        }
    }
}
