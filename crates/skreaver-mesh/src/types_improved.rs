//! Improved type definitions with validation

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// A non-empty, validated string identifier
///
/// Guarantees:
/// - Never empty
/// - No leading/trailing whitespace
/// - Only contains valid characters (alphanumeric, hyphen, underscore, dot)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ValidatedId(String);

impl ValidatedId {
    /// Create a validated ID, returning an error if invalid
    fn parse(s: impl AsRef<str>) -> Result<Self, IdValidationError> {
        let s = s.as_ref();

        // Check for empty string
        if s.is_empty() {
            return Err(IdValidationError::Empty);
        }

        // Check for whitespace-only
        if s.trim().is_empty() {
            return Err(IdValidationError::WhitespaceOnly);
        }

        // Check for leading/trailing whitespace
        if s != s.trim() {
            return Err(IdValidationError::LeadingTrailingWhitespace);
        }

        // Validate characters (alphanumeric, hyphen, underscore, dot)
        if !s.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.') {
            return Err(IdValidationError::InvalidCharacters);
        }

        Ok(Self(s.to_string()))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

/// Unique identifier for an agent in the mesh
///
/// Guarantees:
/// - Non-empty
/// - No leading/trailing whitespace
/// - Contains only valid characters
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AgentId(ValidatedId);

impl AgentId {
    /// Parse an agent ID from a string
    ///
    /// Returns an error if the string is invalid (empty, whitespace-only,
    /// contains invalid characters, etc.)
    pub fn parse(id: impl AsRef<str>) -> Result<Self, IdValidationError> {
        ValidatedId::parse(id).map(Self)
    }

    /// Get the agent ID as a string slice
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Create an agent ID without validation (unsafe)
    ///
    /// This should only be used for testing or when the input is known to be valid.
    /// Prefer `parse()` for all user-provided input.
    #[doc(hidden)]
    pub fn new_unchecked(id: impl Into<String>) -> Self {
        Self(ValidatedId(id.into()))
    }
}

impl FromStr for AgentId {
    type Err = IdValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl TryFrom<String> for AgentId {
    type Error = IdValidationError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

impl TryFrom<&str> for AgentId {
    type Error = IdValidationError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

impl From<AgentId> for String {
    fn from(id: AgentId) -> String {
        id.0 .0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.as_str())
    }
}

/// Topic identifier for pub/sub messaging
///
/// Guarantees:
/// - Non-empty
/// - No leading/trailing whitespace
/// - Contains only valid characters
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Topic(ValidatedId);

impl Topic {
    /// Parse a topic from a string
    ///
    /// Returns an error if the string is invalid (empty, whitespace-only,
    /// contains invalid characters, etc.)
    pub fn parse(topic: impl AsRef<str>) -> Result<Self, IdValidationError> {
        ValidatedId::parse(topic).map(Self)
    }

    /// Get the topic as a string slice
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Create a topic without validation (unsafe)
    ///
    /// This should only be used for testing or when the input is known to be valid.
    /// Prefer `parse()` for all user-provided input.
    #[doc(hidden)]
    pub fn new_unchecked(topic: impl Into<String>) -> Self {
        Self(ValidatedId(topic.into()))
    }
}

impl FromStr for Topic {
    type Err = IdValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl TryFrom<String> for Topic {
    type Error = IdValidationError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

impl TryFrom<&str> for Topic {
    type Error = IdValidationError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

impl From<Topic> for String {
    fn from(topic: Topic) -> String {
        topic.0 .0
    }
}

impl fmt::Display for Topic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.as_str())
    }
}

/// Error type for ID validation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdValidationError {
    /// The ID string is empty
    Empty,
    /// The ID contains only whitespace
    WhitespaceOnly,
    /// The ID has leading or trailing whitespace
    LeadingTrailingWhitespace,
    /// The ID contains invalid characters
    InvalidCharacters,
}

impl fmt::Display for IdValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "ID cannot be empty"),
            Self::WhitespaceOnly => write!(f, "ID cannot be whitespace-only"),
            Self::LeadingTrailingWhitespace => {
                write!(f, "ID cannot have leading or trailing whitespace")
            }
            Self::InvalidCharacters => write!(
                f,
                "ID can only contain alphanumeric characters, hyphens, underscores, and dots"
            ),
        }
    }
}

impl std::error::Error for IdValidationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_id_valid() {
        assert!(AgentId::parse("agent-1").is_ok());
        assert!(AgentId::parse("my_agent").is_ok());
        assert!(AgentId::parse("agent.123").is_ok());
        assert!(AgentId::parse("AGENT-2").is_ok());
    }

    #[test]
    fn test_agent_id_empty() {
        assert_eq!(AgentId::parse(""), Err(IdValidationError::Empty));
    }

    #[test]
    fn test_agent_id_whitespace_only() {
        assert_eq!(
            AgentId::parse("   "),
            Err(IdValidationError::WhitespaceOnly)
        );
        assert_eq!(
            AgentId::parse("\t\n"),
            Err(IdValidationError::WhitespaceOnly)
        );
    }

    #[test]
    fn test_agent_id_leading_trailing_whitespace() {
        assert_eq!(
            AgentId::parse(" agent-1"),
            Err(IdValidationError::LeadingTrailingWhitespace)
        );
        assert_eq!(
            AgentId::parse("agent-1 "),
            Err(IdValidationError::LeadingTrailingWhitespace)
        );
    }

    #[test]
    fn test_agent_id_invalid_characters() {
        assert_eq!(
            AgentId::parse("agent/1"),
            Err(IdValidationError::InvalidCharacters)
        );
        assert_eq!(
            AgentId::parse("agent@host"),
            Err(IdValidationError::InvalidCharacters)
        );
        assert_eq!(
            AgentId::parse("agent:1"),
            Err(IdValidationError::InvalidCharacters)
        );
    }

    #[test]
    fn test_agent_id_display() {
        let id = AgentId::parse("agent-1").unwrap();
        assert_eq!(id.to_string(), "agent-1");
        assert_eq!(id.as_str(), "agent-1");
    }

    #[test]
    fn test_agent_id_serde() {
        let id = AgentId::parse("agent-1").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        let deserialized: AgentId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn test_agent_id_serde_invalid() {
        let json = r#""""#; // Empty string
        let result: Result<AgentId, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_topic_valid() {
        assert!(Topic::parse("notifications").is_ok());
        assert!(Topic::parse("system.events").is_ok());
        assert!(Topic::parse("user_updates").is_ok());
    }

    #[test]
    fn test_topic_invalid() {
        assert!(Topic::parse("").is_err());
        assert!(Topic::parse("   ").is_err());
        assert!(Topic::parse("topic/subtopic").is_err());
    }
}
