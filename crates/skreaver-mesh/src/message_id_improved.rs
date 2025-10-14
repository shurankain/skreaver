//! Improved MessageId with validation

use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

/// Unique identifier for a message
///
/// Guarantees:
/// - Always contains a valid UUID
/// - Cannot be constructed with invalid data
/// - Parsing failures are explicit
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct MessageId(Uuid);

impl MessageId {
    /// Create a new random message ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Parse a message ID from a string
    ///
    /// Returns an error if the string is not a valid UUID.
    pub fn parse(id: impl AsRef<str>) -> Result<Self, MessageIdError> {
        let uuid = Uuid::parse_str(id.as_ref())
            .map_err(|_| MessageIdError::InvalidFormat)?;
        Ok(Self(uuid))
    }

    /// Get the message ID as a string
    pub fn as_str(&self) -> String {
        self.0.to_string()
    }

    /// Get the underlying UUID
    pub fn as_uuid(&self) -> &Uuid {
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

impl FromStr for MessageId {
    type Err = MessageIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl From<MessageId> for String {
    fn from(id: MessageId) -> String {
        id.0.to_string()
    }
}

impl TryFrom<String> for MessageId {
    type Error = MessageIdError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

/// Error type for MessageId parsing
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageIdError {
    /// The provided string is not a valid UUID format
    InvalidFormat,
}

impl std::fmt::Display for MessageIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFormat => write!(f, "Invalid message ID format (expected UUID)"),
        }
    }
}

impl std::error::Error for MessageIdError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_id_new() {
        let id1 = MessageId::new();
        let id2 = MessageId::new();
        assert_ne!(id1, id2); // UUIDs should be unique
    }

    #[test]
    fn test_message_id_parse_valid() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let id = MessageId::parse(uuid_str).unwrap();
        assert_eq!(id.as_str(), uuid_str);
    }

    #[test]
    fn test_message_id_parse_invalid() {
        let invalid = "not-a-uuid";
        let result = MessageId::parse(invalid);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), MessageIdError::InvalidFormat);
    }

    #[test]
    fn test_message_id_parse_empty() {
        let result = MessageId::parse("");
        assert!(result.is_err());
    }

    #[test]
    fn test_message_id_roundtrip() {
        let id = MessageId::new();
        let id_str = id.to_string();
        let parsed = MessageId::parse(id_str).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_message_id_serde() {
        let id = MessageId::new();
        let json = serde_json::to_string(&id).unwrap();
        let deserialized: MessageId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn test_message_id_serde_invalid() {
        let json = r#""not-a-uuid""#;
        let result: Result<MessageId, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}
