//! Core type definitions for mesh communication

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

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

/// Unique identifier for an agent in the mesh
///
/// ## Validation
/// Use `AgentId::parse()` to create validated IDs. The `from()` constructor
/// is deprecated as it doesn't validate input.
///
/// Valid IDs:
/// - Non-empty
/// - No leading/trailing whitespace
/// - Only alphanumeric, hyphens, underscores, dots
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(String);

impl AgentId {
    /// Parse and validate an agent ID from a string
    ///
    /// Returns an error if the string is invalid (empty, whitespace-only,
    /// contains invalid characters, etc.)
    ///
    /// # Examples
    ///
    /// ```
    /// use skreaver_mesh::AgentId;
    ///
    /// // Valid IDs
    /// assert!(AgentId::parse("agent-1").is_ok());
    /// assert!(AgentId::parse("my_agent").is_ok());
    /// assert!(AgentId::parse("agent.123").is_ok());
    ///
    /// // Invalid IDs
    /// assert!(AgentId::parse("").is_err());           // Empty
    /// assert!(AgentId::parse("   ").is_err());        // Whitespace only
    /// assert!(AgentId::parse(" agent").is_err());     // Leading whitespace
    /// assert!(AgentId::parse("agent/path").is_err()); // Invalid char
    /// ```
    pub fn parse(id: impl AsRef<str>) -> Result<Self, IdValidationError> {
        let s = id.as_ref();

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
        if !s
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Err(IdValidationError::InvalidCharacters);
        }

        Ok(Self(s.to_string()))
    }

    /// Get the agent ID as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for AgentId {
    type Err = IdValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl From<String> for AgentId {
    /// Creates an AgentId from a String.
    ///
    /// # Panics
    /// Panics if the string fails validation (empty, whitespace-only, invalid characters).
    /// For non-panicking construction, use `AgentId::parse()` instead.
    fn from(s: String) -> Self {
        Self::parse(&s).unwrap_or_else(|e| panic!("Invalid AgentId '{}': {}", s, e))
    }
}

impl From<&str> for AgentId {
    /// Creates an AgentId from a string slice.
    ///
    /// # Panics
    /// Panics if the string fails validation (empty, whitespace-only, invalid characters).
    /// For non-panicking construction, use `AgentId::parse()` instead.
    fn from(s: &str) -> Self {
        Self::parse(s).unwrap_or_else(|e| panic!("Invalid AgentId '{}': {}", s, e))
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Topic identifier for pub/sub messaging
///
/// ## Validation
/// Use `Topic::parse()` to create validated topics. The `from()` constructor
/// is deprecated as it doesn't validate input.
///
/// Valid topics:
/// - Non-empty
/// - No leading/trailing whitespace
/// - Only alphanumeric, hyphens, underscores, dots
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Topic(String);

impl Topic {
    /// Parse and validate a topic from a string
    ///
    /// Returns an error if the string is invalid (empty, whitespace-only,
    /// contains invalid characters, etc.)
    ///
    /// # Examples
    ///
    /// ```
    /// use skreaver_mesh::Topic;
    ///
    /// // Valid topics
    /// assert!(Topic::parse("notifications").is_ok());
    /// assert!(Topic::parse("user-events").is_ok());
    /// assert!(Topic::parse("system.logs").is_ok());
    ///
    /// // Invalid topics
    /// assert!(Topic::parse("").is_err());           // Empty
    /// assert!(Topic::parse("   ").is_err());        // Whitespace only
    /// assert!(Topic::parse(" topic").is_err());     // Leading whitespace
    /// assert!(Topic::parse("topic/sub").is_err());  // Invalid char
    /// ```
    pub fn parse(topic: impl AsRef<str>) -> Result<Self, IdValidationError> {
        let s = topic.as_ref();

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
        if !s
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Err(IdValidationError::InvalidCharacters);
        }

        Ok(Self(s.to_string()))
    }

    /// Get the topic as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Topic {
    type Err = IdValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl From<String> for Topic {
    /// Creates a Topic from a String.
    ///
    /// # Panics
    /// Panics if the string fails validation (empty, whitespace-only, invalid characters).
    /// For non-panicking construction, use `Topic::parse()` instead.
    fn from(s: String) -> Self {
        Self::parse(&s).unwrap_or_else(|e| panic!("Invalid Topic '{}': {}", s, e))
    }
}

impl From<&str> for Topic {
    /// Creates a Topic from a string slice.
    ///
    /// # Panics
    /// Panics if the string fails validation (empty, whitespace-only, invalid characters).
    /// For non-panicking construction, use `Topic::parse()` instead.
    fn from(s: &str) -> Self {
        Self::parse(s).unwrap_or_else(|e| panic!("Invalid Topic '{}': {}", s, e))
    }
}

impl fmt::Display for Topic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_id_creation() {
        let id = AgentId::parse("agent-1").unwrap();
        assert_eq!(id.as_str(), "agent-1");
        assert_eq!(id.to_string(), "agent-1");
    }

    #[test]
    fn test_agent_id_from_string() {
        let id: AgentId = "agent-2".into();
        assert_eq!(id.as_str(), "agent-2");
    }

    #[test]
    fn test_topic_creation() {
        let topic = Topic::parse("notifications").unwrap();
        assert_eq!(topic.as_str(), "notifications");
        assert_eq!(topic.to_string(), "notifications");
    }

    #[test]
    fn test_topic_from_str() {
        let topic: Topic = "events".into();
        assert_eq!(topic.as_str(), "events");
    }

    #[test]
    #[should_panic(expected = "Invalid AgentId")]
    fn test_agent_id_from_panics_on_empty() {
        let _: AgentId = "".into();
    }

    #[test]
    #[should_panic(expected = "Invalid Topic")]
    fn test_topic_from_panics_on_invalid() {
        let _: Topic = "../path".into();
    }

    // Validation tests for AgentId
    #[test]
    fn test_agent_id_parse_valid() {
        assert!(AgentId::parse("agent-1").is_ok());
        assert!(AgentId::parse("my_agent").is_ok());
        assert!(AgentId::parse("agent.123").is_ok());
        assert!(AgentId::parse("AGENT-2").is_ok());
        assert!(AgentId::parse("a").is_ok());
        assert!(AgentId::parse("123").is_ok());
    }

    #[test]
    fn test_agent_id_parse_empty() {
        let result = AgentId::parse("");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), IdValidationError::Empty);
    }

    #[test]
    fn test_agent_id_parse_whitespace_only() {
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
    fn test_agent_id_parse_leading_trailing_whitespace() {
        assert_eq!(
            AgentId::parse(" agent"),
            Err(IdValidationError::LeadingTrailingWhitespace)
        );
        assert_eq!(
            AgentId::parse("agent "),
            Err(IdValidationError::LeadingTrailingWhitespace)
        );
        assert_eq!(
            AgentId::parse(" agent "),
            Err(IdValidationError::LeadingTrailingWhitespace)
        );
    }

    #[test]
    fn test_agent_id_parse_invalid_characters() {
        // Path traversal attempts
        assert_eq!(
            AgentId::parse("../agent"),
            Err(IdValidationError::InvalidCharacters)
        );
        assert_eq!(
            AgentId::parse("agent/../../etc/passwd"),
            Err(IdValidationError::InvalidCharacters)
        );

        // Other invalid characters
        assert_eq!(
            AgentId::parse("agent@host"),
            Err(IdValidationError::InvalidCharacters)
        );
        assert_eq!(
            AgentId::parse("agent:port"),
            Err(IdValidationError::InvalidCharacters)
        );
        assert_eq!(
            AgentId::parse("agent$var"),
            Err(IdValidationError::InvalidCharacters)
        );
        assert_eq!(
            AgentId::parse("agent space"),
            Err(IdValidationError::InvalidCharacters)
        );
    }

    #[test]
    fn test_agent_id_from_str_trait() {
        use std::str::FromStr;
        let result = AgentId::from_str("agent-1");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), "agent-1");

        let result = AgentId::from_str("");
        assert!(result.is_err());
    }

    // Validation tests for Topic
    #[test]
    fn test_topic_parse_valid() {
        assert!(Topic::parse("notifications").is_ok());
        assert!(Topic::parse("user-events").is_ok());
        assert!(Topic::parse("system.logs").is_ok());
        assert!(Topic::parse("EVENTS").is_ok());
        assert!(Topic::parse("t").is_ok());
    }

    #[test]
    fn test_topic_parse_empty() {
        let result = Topic::parse("");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), IdValidationError::Empty);
    }

    #[test]
    fn test_topic_parse_whitespace_only() {
        assert_eq!(Topic::parse("   "), Err(IdValidationError::WhitespaceOnly));
    }

    #[test]
    fn test_topic_parse_leading_trailing_whitespace() {
        assert_eq!(
            Topic::parse(" topic"),
            Err(IdValidationError::LeadingTrailingWhitespace)
        );
        assert_eq!(
            Topic::parse("topic "),
            Err(IdValidationError::LeadingTrailingWhitespace)
        );
    }

    #[test]
    fn test_topic_parse_invalid_characters() {
        // Path traversal attempts
        assert_eq!(
            Topic::parse("../topic"),
            Err(IdValidationError::InvalidCharacters)
        );
        assert_eq!(
            Topic::parse("topic/sub"),
            Err(IdValidationError::InvalidCharacters)
        );

        // Other invalid characters
        assert_eq!(
            Topic::parse("topic@host"),
            Err(IdValidationError::InvalidCharacters)
        );
        assert_eq!(
            Topic::parse("topic space"),
            Err(IdValidationError::InvalidCharacters)
        );
    }

    #[test]
    fn test_topic_from_str_trait() {
        use std::str::FromStr;
        let result = Topic::from_str("events");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), "events");

        let result = Topic::from_str("");
        assert!(result.is_err());
    }

    #[test]
    fn test_id_validation_error_display() {
        assert_eq!(IdValidationError::Empty.to_string(), "ID cannot be empty");
        assert_eq!(
            IdValidationError::WhitespaceOnly.to_string(),
            "ID cannot be whitespace-only"
        );
        assert_eq!(
            IdValidationError::LeadingTrailingWhitespace.to_string(),
            "ID cannot have leading or trailing whitespace"
        );
        assert_eq!(
            IdValidationError::InvalidCharacters.to_string(),
            "ID can only contain alphanumeric characters, hyphens, underscores, and dots"
        );
    }
}
