//! Core type definitions for mesh communication

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

// Re-export unified types from skreaver-core
pub use skreaver_core::{AgentId, ValidationError};

// Keep deprecated alias for backward compatibility during transition period
#[deprecated(
    since = "0.6.0",
    note = "Use ValidationError instead. See skreaver_core::ValidationError for migration guide."
)]
#[allow(deprecated)]
pub use skreaver_core::IdValidationError;

/// Legacy AgentId type alias for backward compatibility
///
/// **DEPRECATED**: Use `skreaver_core::AgentId` directly instead.
/// This type alias will be removed in version 0.7.0.
#[deprecated(
    since = "0.6.0",
    note = "Use skreaver_core::AgentId instead. This alias will be removed in 0.7.0"
)]
pub type LegacyAgentId = AgentId;

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
    pub fn parse(topic: impl AsRef<str>) -> Result<Self, ValidationError> {
        let s = topic.as_ref();

        // Check for empty string
        if s.is_empty() {
            return Err(ValidationError::Empty);
        }

        // Check for whitespace-only
        if s.trim().is_empty() {
            return Err(ValidationError::WhitespaceOnly);
        }

        // Check for leading/trailing whitespace
        if s != s.trim() {
            return Err(ValidationError::LeadingTrailingWhitespace);
        }

        // Check for path traversal
        if s.contains("../") || s.contains("/..") {
            return Err(ValidationError::PathTraversal);
        }

        // Validate characters (alphanumeric, hyphen, underscore, dot)
        for ch in s.chars() {
            if !ch.is_alphanumeric() && ch != '-' && ch != '_' && ch != '.' {
                return Err(ValidationError::InvalidChar {
                    char: ch,
                    input: s.to_string(),
                });
            }
        }

        Ok(Self(s.to_string()))
    }

    /// Get the topic as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Topic {
    type Err = ValidationError;

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
        let id = AgentId::parse("agent-2").unwrap();
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
    fn test_agent_id_parse_fails_on_empty() {
        assert!(AgentId::parse("").is_err());
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
        assert!(matches!(result.unwrap_err(), ValidationError::Empty));
    }

    #[test]
    fn test_agent_id_parse_whitespace_only() {
        assert!(matches!(
            AgentId::parse("   "),
            Err(ValidationError::WhitespaceOnly)
        ));
        assert!(matches!(
            AgentId::parse("\t\n"),
            Err(ValidationError::WhitespaceOnly)
        ));
    }

    #[test]
    fn test_agent_id_parse_leading_trailing_whitespace() {
        assert!(matches!(
            AgentId::parse(" agent"),
            Err(ValidationError::LeadingTrailingWhitespace)
        ));
        assert!(matches!(
            AgentId::parse("agent "),
            Err(ValidationError::LeadingTrailingWhitespace)
        ));
        assert!(matches!(
            AgentId::parse(" agent "),
            Err(ValidationError::LeadingTrailingWhitespace)
        ));
    }

    #[test]
    fn test_agent_id_parse_invalid_characters() {
        // Path traversal attempts - unified AgentId detects these specifically
        assert!(matches!(
            AgentId::parse("../agent"),
            Err(ValidationError::PathTraversal)
        ));
        assert!(matches!(
            AgentId::parse("agent/../../etc/passwd"),
            Err(ValidationError::PathTraversal)
        ));

        // Other invalid characters
        assert!(matches!(
            AgentId::parse("agent@host"),
            Err(ValidationError::InvalidChar { char: '@', .. })
        ));
        assert!(matches!(
            AgentId::parse("agent:port"),
            Err(ValidationError::InvalidChar { char: ':', .. })
        ));
        assert!(matches!(
            AgentId::parse("agent$var"),
            Err(ValidationError::InvalidChar { char: '$', .. })
        ));
        assert!(matches!(
            AgentId::parse("agent space"),
            Err(ValidationError::InvalidChar { char: ' ', .. })
        ));
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
        assert!(matches!(result.unwrap_err(), ValidationError::Empty));
    }

    #[test]
    fn test_topic_parse_whitespace_only() {
        assert!(matches!(
            Topic::parse("   "),
            Err(ValidationError::WhitespaceOnly)
        ));
    }

    #[test]
    fn test_topic_parse_leading_trailing_whitespace() {
        assert!(matches!(
            Topic::parse(" topic"),
            Err(ValidationError::LeadingTrailingWhitespace)
        ));
        assert!(matches!(
            Topic::parse("topic "),
            Err(ValidationError::LeadingTrailingWhitespace)
        ));
    }

    #[test]
    fn test_topic_parse_invalid_characters() {
        // Path traversal attempts
        assert!(matches!(
            Topic::parse("../topic"),
            Err(ValidationError::PathTraversal)
        ));
        assert!(matches!(
            Topic::parse("topic/sub"),
            Err(ValidationError::InvalidChar { char: '/', .. })
        ));

        // Other invalid characters
        assert!(matches!(
            Topic::parse("topic@host"),
            Err(ValidationError::InvalidChar { char: '@', .. })
        ));
        assert!(matches!(
            Topic::parse("topic space"),
            Err(ValidationError::InvalidChar { char: ' ', .. })
        ));
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
    fn test_validation_error_display() {
        // Updated to match unified error messages from skreaver-core
        assert_eq!(
            ValidationError::Empty.to_string(),
            "Identifier cannot be empty"
        );
        assert_eq!(
            ValidationError::WhitespaceOnly.to_string(),
            "Identifier cannot be whitespace-only"
        );
        assert_eq!(
            ValidationError::LeadingTrailingWhitespace.to_string(),
            "Identifier cannot have leading or trailing whitespace"
        );
        assert_eq!(
            ValidationError::PathTraversal.to_string(),
            "Identifier cannot contain path traversal sequences (../)"
        );

        // Test InvalidChar variant
        let err = ValidationError::InvalidChar {
            char: '@',
            input: "test@value".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Identifier 'test@value' contains invalid character '@'"
        );
    }
}
