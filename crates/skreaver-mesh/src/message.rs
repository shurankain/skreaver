//! Message types and builders for agent communication

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::types::AgentId;

/// Unique identifier for a message
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(String);

impl MessageId {
    /// Create a new random message ID
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create a message ID from a string
    pub fn from_string(id: String) -> Self {
        Self(id)
    }

    /// Get the message ID as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Message payload - can be any serializable type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessagePayload {
    /// String payload
    Text(String),
    /// JSON payload
    Json(serde_json::Value),
    /// Binary payload (base64 encoded in JSON)
    #[serde(with = "base64_serde")]
    Binary(Vec<u8>),
}

mod base64_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            bytes,
        ))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, s)
            .map_err(serde::de::Error::custom)
    }
}

impl From<String> for MessagePayload {
    fn from(s: String) -> Self {
        MessagePayload::Text(s)
    }
}

impl From<&str> for MessagePayload {
    fn from(s: &str) -> Self {
        MessagePayload::Text(s.to_string())
    }
}

impl From<serde_json::Value> for MessagePayload {
    fn from(v: serde_json::Value) -> Self {
        MessagePayload::Json(v)
    }
}

impl From<Vec<u8>> for MessagePayload {
    fn from(v: Vec<u8>) -> Self {
        MessagePayload::Binary(v)
    }
}

/// Message metadata
pub type MessageMetadata = HashMap<String, String>;

/// A message sent between agents in the mesh
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique message identifier
    pub id: MessageId,
    /// Sender agent ID (optional for broadcasts)
    pub from: Option<AgentId>,
    /// Recipient agent ID (None for broadcasts)
    pub to: Option<AgentId>,
    /// Message payload
    pub payload: MessagePayload,
    /// Message metadata (arbitrary key-value pairs)
    #[serde(default)]
    pub metadata: MessageMetadata,
    /// Timestamp when message was created
    pub timestamp: DateTime<Utc>,
    /// Optional correlation ID for request/reply patterns
    pub correlation_id: Option<String>,
}

impl Message {
    /// Create a new message with the given payload
    pub fn new(payload: impl Into<MessagePayload>) -> Self {
        Self {
            id: MessageId::new(),
            from: None,
            to: None,
            payload: payload.into(),
            metadata: HashMap::new(),
            timestamp: Utc::now(),
            correlation_id: None,
        }
    }

    /// Set the sender agent ID
    pub fn from(mut self, agent_id: impl Into<AgentId>) -> Self {
        self.from = Some(agent_id.into());
        self
    }

    /// Set the recipient agent ID
    pub fn to(mut self, agent_id: impl Into<AgentId>) -> Self {
        self.to = Some(agent_id.into());
        self
    }

    /// Add metadata to the message
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Set correlation ID for request/reply pattern
    pub fn with_correlation_id(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }

    /// Get metadata value by key
    pub fn get_metadata(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }

    /// Serialize message to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Deserialize message from JSON
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Builder for creating messages with a fluent API
pub struct MessageBuilder {
    message: Message,
}

impl MessageBuilder {
    /// Create a new message builder
    pub fn new(payload: impl Into<MessagePayload>) -> Self {
        Self {
            message: Message::new(payload),
        }
    }

    /// Set the sender agent ID
    pub fn from(mut self, agent_id: impl Into<AgentId>) -> Self {
        self.message = self.message.from(agent_id);
        self
    }

    /// Set the recipient agent ID
    pub fn to(mut self, agent_id: impl Into<AgentId>) -> Self {
        self.message = self.message.to(agent_id);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.message = self.message.with_metadata(key, value);
        self
    }

    /// Set correlation ID
    pub fn with_correlation_id(mut self, correlation_id: impl Into<String>) -> Self {
        self.message = self.message.with_correlation_id(correlation_id);
        self
    }

    /// Build the message
    pub fn build(self) -> Message {
        self.message
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let msg = Message::new("hello");
        assert!(matches!(msg.payload, MessagePayload::Text(_)));
        assert!(msg.from.is_none());
        assert!(msg.to.is_none());
    }

    #[test]
    fn test_message_builder() {
        let msg = MessageBuilder::new("test")
            .from("agent-1")
            .to("agent-2")
            .with_metadata("priority", "high")
            .with_correlation_id("req-123")
            .build();

        assert_eq!(msg.from.as_ref().unwrap().as_str(), "agent-1");
        assert_eq!(msg.to.as_ref().unwrap().as_str(), "agent-2");
        assert_eq!(msg.get_metadata("priority"), Some("high"));
        assert_eq!(msg.correlation_id.as_deref(), Some("req-123"));
    }

    #[test]
    fn test_message_serialization() {
        let msg = Message::new("test payload").from("agent-1").to("agent-2");

        let json = msg.to_json().unwrap();
        let deserialized = Message::from_json(&json).unwrap();

        assert_eq!(msg.id, deserialized.id);
        assert_eq!(msg.from, deserialized.from);
        assert_eq!(msg.to, deserialized.to);
    }

    #[test]
    fn test_message_payload_types() {
        let text_msg = Message::new("text");
        assert!(matches!(text_msg.payload, MessagePayload::Text(_)));

        let json_msg = Message::new(serde_json::json!({"key": "value"}));
        assert!(matches!(json_msg.payload, MessagePayload::Json(_)));

        let binary_msg = Message::new(vec![1u8, 2, 3]);
        assert!(matches!(binary_msg.payload, MessagePayload::Binary(_)));
    }
}
