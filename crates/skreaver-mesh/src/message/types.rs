//! Core message types and routing primitives.
//!
//! This module contains the fundamental types used throughout the message system:
//! - Route enum for type-safe message routing
//! - MessageId newtype with UUID validation
//! - MessagePayload enum for different content types
//! - Typestate markers for compile-time routing guarantees

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::types::AgentId;

// ============================================================================
// Typestate Markers
// ============================================================================

/// Typestate marker: Message routing is not yet determined
pub struct Unrouted;

/// Typestate marker: Message has unicast routing
pub struct UnicastRoute;

/// Typestate marker: Message has broadcast routing
pub struct BroadcastRoute;

/// Typestate marker: Message has system routing
pub struct SystemRoute;

/// Typestate marker: Message has anonymous routing
pub struct AnonymousRoute;

// ============================================================================
// Route Type
// ============================================================================

/// Type-safe message routing pattern
///
/// Replaces runtime boolean checks with compile-time routing patterns.
/// Each variant explicitly encodes the routing semantics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    /// Direct message from one agent to another
    /// Guarantees: Both sender and recipient are known
    Unicast { from: AgentId, to: AgentId },
    /// Broadcast from an agent to all listeners
    /// Guarantees: Sender is known, no specific recipient
    Broadcast { from: AgentId },
    /// System message to a specific agent
    /// Guarantees: Recipient is known, sender is the system
    System { to: AgentId },
    /// System-wide broadcast (for infrastructure messages)
    /// Guarantees: No specific sender or recipient
    Anonymous,
}

impl Route {
    /// Create a unicast route from one agent to another
    pub fn unicast(from: impl Into<AgentId>, to: impl Into<AgentId>) -> Self {
        Route::Unicast {
            from: from.into(),
            to: to.into(),
        }
    }

    /// Create a broadcast route from an agent
    pub fn broadcast(from: impl Into<AgentId>) -> Self {
        Route::Broadcast { from: from.into() }
    }

    /// Create a system route to a specific agent
    pub fn system(to: impl Into<AgentId>) -> Self {
        Route::System { to: to.into() }
    }

    /// Create an anonymous route (system-wide)
    pub fn anonymous() -> Self {
        Route::Anonymous
    }

    /// Get the sender agent ID if present
    pub fn sender(&self) -> Option<&AgentId> {
        match self {
            Route::Unicast { from, .. } | Route::Broadcast { from } => Some(from),
            Route::System { .. } | Route::Anonymous => None,
        }
    }

    /// Get the recipient agent ID if present
    pub fn recipient(&self) -> Option<&AgentId> {
        match self {
            Route::Unicast { to, .. } | Route::System { to } => Some(to),
            Route::Broadcast { .. } | Route::Anonymous => None,
        }
    }

    /// Check if this route targets a specific recipient
    pub fn has_recipient(&self) -> bool {
        matches!(self, Route::Unicast { .. } | Route::System { .. })
    }

    /// Check if this route has a known sender
    pub fn has_sender(&self) -> bool {
        matches!(self, Route::Unicast { .. } | Route::Broadcast { .. })
    }
}

// ============================================================================
// MessageId Type
// ============================================================================

/// Error type for MessageId validation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageIdError {
    /// The provided string is not a valid UUID
    InvalidFormat(String),
}

impl std::fmt::Display for MessageIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageIdError::InvalidFormat(s) => {
                write!(f, "Invalid MessageId format (expected UUID): '{}'", s)
            }
        }
    }
}

impl std::error::Error for MessageIdError {}

/// Unique identifier for a message (UUID v4)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(String);

impl MessageId {
    /// Create a new random message ID (UUID v4)
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Parse and validate a message ID from a string
    ///
    /// Returns an error if the string is not a valid UUID format.
    ///
    /// # Examples
    ///
    /// ```
    /// use skreaver_mesh::MessageId;
    ///
    /// // Valid UUID
    /// let id = MessageId::parse("550e8400-e29b-41d4-a716-446655440000").unwrap();
    ///
    /// // Invalid format
    /// assert!(MessageId::parse("not-a-uuid").is_err());
    /// assert!(MessageId::parse("").is_err());
    /// ```
    pub fn parse(id: impl AsRef<str>) -> Result<Self, MessageIdError> {
        let s = id.as_ref();

        // Validate UUID format
        Uuid::parse_str(s).map_err(|_| MessageIdError::InvalidFormat(s.to_string()))?;

        Ok(Self(s.to_string()))
    }

    /// Create a message ID from a string without validation
    ///
    /// # Panics
    /// Panics if the string is not a valid UUID format.
    /// For non-panicking construction, use `MessageId::parse()` instead.
    pub fn from_string(id: String) -> Self {
        Self::parse(&id).unwrap_or_else(|e| panic!("Invalid MessageId: {}", e))
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

// ============================================================================
// MessagePayload Type
// ============================================================================

/// Message payload - can be any serializable type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum MessagePayload {
    /// String payload
    #[serde(rename = "text")]
    Text(String),
    /// JSON payload
    #[serde(rename = "json")]
    Json(serde_json::Value),
    /// Binary payload (base64 encoded in JSON)
    #[serde(rename = "binary")]
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

// ============================================================================
// MessageMetadata Type
// ============================================================================

/// Message metadata
pub type MessageMetadata = HashMap<String, String>;
