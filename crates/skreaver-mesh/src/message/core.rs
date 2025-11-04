//! Core message structure and implementations.
//!
//! This module contains the main `Message` struct used throughout the mesh
//! for agent communication. Messages support multiple routing patterns through
//! the type-safe `Route` enum.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::types::{MessageId, MessageMetadata, MessagePayload, Route};
use crate::types::AgentId;

/// A message sent between agents in the mesh
///
/// ## Type-Safe Routing
/// Messages now use explicit `Route` enum for compile-time routing guarantees:
/// - **Route::Unicast**: Direct message from agent A to agent B
/// - **Route::Broadcast**: Broadcast from agent A to all listeners
/// - **Route::System**: System message to a specific agent
/// - **Route::Anonymous**: System broadcast (rare, for infrastructure)
///
/// ## Backward Compatibility
/// Legacy `from`/`to` fields are maintained for backward compatibility but deprecated.
/// Use the `route` field for routing information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique message identifier
    pub id: MessageId,
    /// Type-safe routing information
    pub route: Route,
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
    /// Create a new anonymous message with the given payload
    pub fn new(payload: impl Into<MessagePayload>) -> Self {
        Self {
            id: MessageId::new(),
            route: Route::Anonymous,
            payload: payload.into(),
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now(),
            correlation_id: None,
        }
    }

    /// Create a unicast message from one agent to another
    pub fn unicast(from: AgentId, to: AgentId, payload: impl Into<MessagePayload>) -> Self {
        Self {
            id: MessageId::new(),
            route: Route::Unicast { from, to },
            payload: payload.into(),
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now(),
            correlation_id: None,
        }
    }

    /// Create a broadcast message from an agent
    pub fn broadcast(from: AgentId, payload: impl Into<MessagePayload>) -> Self {
        Self {
            id: MessageId::new(),
            route: Route::Broadcast { from },
            payload: payload.into(),
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now(),
            correlation_id: None,
        }
    }

    /// Create a system message to a specific agent
    pub fn system(to: AgentId, payload: impl Into<MessagePayload>) -> Self {
        Self {
            id: MessageId::new(),
            route: Route::System { to },
            payload: payload.into(),
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now(),
            correlation_id: None,
        }
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

    // Routing pattern helpers

    /// Check if this is a unicast message (from agent to agent)
    pub fn is_unicast(&self) -> bool {
        matches!(self.route, Route::Unicast { .. })
    }

    /// Check if this is a broadcast message (from agent to all)
    pub fn is_broadcast(&self) -> bool {
        matches!(self.route, Route::Broadcast { .. })
    }

    /// Check if this is a system message (to specific agent, no sender)
    pub fn is_system(&self) -> bool {
        matches!(self.route, Route::System { .. })
    }

    /// Check if this is an anonymous message (no sender, no recipient)
    pub fn is_anonymous(&self) -> bool {
        matches!(self.route, Route::Anonymous)
    }

    /// Get the sender agent ID from the route
    pub fn sender(&self) -> Option<&AgentId> {
        self.route.sender()
    }

    /// Get the recipient agent ID from the route
    pub fn recipient(&self) -> Option<&AgentId> {
        self.route.recipient()
    }

    /// Get the route information
    pub fn route(&self) -> &Route {
        &self.route
    }
}
