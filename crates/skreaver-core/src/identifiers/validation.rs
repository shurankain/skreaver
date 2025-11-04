//! Identifier validation rules and utilities

use std::fmt;

/// Maximum length for all identifier types
pub const MAX_ID_LENGTH: usize = 128;

/// Error type for identifier validation failures
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdValidationError {
    /// The identifier string is empty
    Empty,
    /// The identifier contains only whitespace
    WhitespaceOnly,
    /// The identifier has leading or trailing whitespace
    LeadingTrailingWhitespace,
    /// The identifier contains invalid characters
    InvalidCharacters,
    /// The identifier exceeds the maximum length
    TooLong { length: usize, max: usize },
    /// The identifier contains path traversal sequences
    PathTraversal,
}

impl fmt::Display for IdValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "Identifier cannot be empty"),
            Self::WhitespaceOnly => write!(f, "Identifier cannot be whitespace-only"),
            Self::LeadingTrailingWhitespace => {
                write!(f, "Identifier cannot have leading or trailing whitespace")
            }
            Self::InvalidCharacters => write!(
                f,
                "Identifier can only contain alphanumeric characters, hyphens, underscores, and dots"
            ),
            Self::TooLong { length, max } => {
                write!(f, "Identifier too long ({} chars, max {})", length, max)
            }
            Self::PathTraversal => {
                write!(
                    f,
                    "Identifier cannot contain path traversal sequences (../)"
                )
            }
        }
    }
}

impl std::error::Error for IdValidationError {}

/// Validator for identifier strings
pub struct IdValidator;

impl IdValidator {
    /// Validate an identifier string according to Skreaver rules
    ///
    /// # Validation Rules
    ///
    /// - Non-empty (minimum 1 character)
    /// - Maximum 128 characters
    /// - No leading or trailing whitespace
    /// - Only alphanumeric characters, hyphens (`-`), underscores (`_`), and dots (`.`)
    /// - No path traversal sequences (`../`, `./`)
    ///
    /// # Security
    ///
    /// This validation prevents:
    /// - Path traversal attacks
    /// - Shell injection attacks
    /// - Unicode normalization issues
    ///
    /// # Examples
    ///
    /// ```rust
    /// use skreaver_core::identifiers::IdValidator;
    ///
    /// // Valid identifiers
    /// assert!(IdValidator::validate("agent-1").is_ok());
    /// assert!(IdValidator::validate("my_agent").is_ok());
    /// assert!(IdValidator::validate("agent.123").is_ok());
    ///
    /// // Invalid identifiers
    /// assert!(IdValidator::validate("").is_err());
    /// assert!(IdValidator::validate("  ").is_err());
    /// assert!(IdValidator::validate("../etc").is_err());
    /// assert!(IdValidator::validate("agent/path").is_err());
    /// ```
    pub fn validate(id: &str) -> Result<&str, IdValidationError> {
        // Check for empty string
        if id.is_empty() {
            return Err(IdValidationError::Empty);
        }

        // Check for whitespace-only
        if id.trim().is_empty() {
            return Err(IdValidationError::WhitespaceOnly);
        }

        // Check for leading/trailing whitespace
        if id != id.trim() {
            return Err(IdValidationError::LeadingTrailingWhitespace);
        }

        // Check length
        if id.len() > MAX_ID_LENGTH {
            return Err(IdValidationError::TooLong {
                length: id.len(),
                max: MAX_ID_LENGTH,
            });
        }

        // Check for path traversal sequences
        if id.contains("../") || id.contains("./") {
            return Err(IdValidationError::PathTraversal);
        }

        // Validate characters (alphanumeric, hyphen, underscore, dot)
        // Note: Dots are allowed but path traversal sequences are rejected above
        if !id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Err(IdValidationError::InvalidCharacters);
        }

        Ok(id)
    }

    /// Check if a character is valid in an identifier
    pub fn is_valid_char(c: char) -> bool {
        c.is_alphanumeric() || c == '-' || c == '_' || c == '.'
    }

    /// Sanitize a string to make it a valid identifier
    ///
    /// This replaces invalid characters with underscores and truncates to max length.
    /// Useful for generating identifiers from user input.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use skreaver_core::identifiers::IdValidator;
    ///
    /// assert_eq!(IdValidator::sanitize("Hello World!"), "Hello_World_");
    /// assert_eq!(IdValidator::sanitize("agent/path"), "agent_path");
    /// assert_eq!(IdValidator::sanitize("  spaces  "), "spaces");
    /// ```
    pub fn sanitize(input: &str) -> String {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return "unnamed".to_string();
        }

        let sanitized: String = trimmed
            .chars()
            .map(|c| if Self::is_valid_char(c) { c } else { '_' })
            .collect();

        // Truncate to max length
        if sanitized.len() > MAX_ID_LENGTH {
            sanitized[..MAX_ID_LENGTH].to_string()
        } else {
            sanitized
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_ids() {
        assert!(IdValidator::validate("agent-1").is_ok());
        assert!(IdValidator::validate("my_agent").is_ok());
        assert!(IdValidator::validate("agent.123").is_ok());
        assert!(IdValidator::validate("a").is_ok());
        assert!(IdValidator::validate("ABC-def_123").is_ok());
    }

    #[test]
    fn test_validate_empty() {
        assert_eq!(IdValidator::validate(""), Err(IdValidationError::Empty));
    }

    #[test]
    fn test_validate_whitespace_only() {
        assert_eq!(
            IdValidator::validate("   "),
            Err(IdValidationError::WhitespaceOnly)
        );
        assert_eq!(
            IdValidator::validate("\t\n"),
            Err(IdValidationError::WhitespaceOnly)
        );
    }

    #[test]
    fn test_validate_leading_trailing_whitespace() {
        assert_eq!(
            IdValidator::validate(" agent"),
            Err(IdValidationError::LeadingTrailingWhitespace)
        );
        assert_eq!(
            IdValidator::validate("agent "),
            Err(IdValidationError::LeadingTrailingWhitespace)
        );
        assert_eq!(
            IdValidator::validate(" agent "),
            Err(IdValidationError::LeadingTrailingWhitespace)
        );
    }

    #[test]
    fn test_validate_invalid_characters() {
        assert_eq!(
            IdValidator::validate("agent/path"),
            Err(IdValidationError::InvalidCharacters)
        );
        assert_eq!(
            IdValidator::validate("agent@host"),
            Err(IdValidationError::InvalidCharacters)
        );
        assert_eq!(
            IdValidator::validate("agent:port"),
            Err(IdValidationError::InvalidCharacters)
        );
    }

    #[test]
    fn test_validate_path_traversal() {
        assert_eq!(
            IdValidator::validate("../etc"),
            Err(IdValidationError::PathTraversal)
        );
        assert_eq!(
            IdValidator::validate("./file"),
            Err(IdValidationError::PathTraversal)
        );
        assert_eq!(
            IdValidator::validate("path/../other"),
            Err(IdValidationError::PathTraversal)
        );
    }

    #[test]
    fn test_validate_too_long() {
        let long_id = "a".repeat(129);
        match IdValidator::validate(&long_id) {
            Err(IdValidationError::TooLong { length, max }) => {
                assert_eq!(length, 129);
                assert_eq!(max, MAX_ID_LENGTH);
            }
            _ => panic!("Expected TooLong error"),
        }
    }

    #[test]
    fn test_validate_max_length_ok() {
        let max_id = "a".repeat(128);
        assert!(IdValidator::validate(&max_id).is_ok());
    }

    #[test]
    fn test_sanitize() {
        assert_eq!(IdValidator::sanitize("Hello World!"), "Hello_World_");
        assert_eq!(IdValidator::sanitize("agent/path"), "agent_path");
        assert_eq!(IdValidator::sanitize("  spaces  "), "spaces");
        assert_eq!(IdValidator::sanitize("valid-id_123"), "valid-id_123");
    }

    #[test]
    fn test_sanitize_empty() {
        assert_eq!(IdValidator::sanitize(""), "unnamed");
        assert_eq!(IdValidator::sanitize("   "), "unnamed");
    }

    #[test]
    fn test_sanitize_truncates() {
        let long_input = "a".repeat(200);
        let sanitized = IdValidator::sanitize(&long_input);
        assert_eq!(sanitized.len(), MAX_ID_LENGTH);
    }

    #[test]
    fn test_is_valid_char() {
        assert!(IdValidator::is_valid_char('a'));
        assert!(IdValidator::is_valid_char('Z'));
        assert!(IdValidator::is_valid_char('0'));
        assert!(IdValidator::is_valid_char('-'));
        assert!(IdValidator::is_valid_char('_'));
        assert!(IdValidator::is_valid_char('.'));

        assert!(!IdValidator::is_valid_char('/'));
        assert!(!IdValidator::is_valid_char(' '));
        assert!(!IdValidator::is_valid_char('@'));
    }

    #[test]
    fn test_error_display() {
        assert_eq!(
            IdValidationError::Empty.to_string(),
            "Identifier cannot be empty"
        );
        assert_eq!(
            IdValidationError::PathTraversal.to_string(),
            "Identifier cannot contain path traversal sequences (../)"
        );
        assert_eq!(
            IdValidationError::TooLong {
                length: 150,
                max: 128
            }
            .to_string(),
            "Identifier too long (150 chars, max 128)"
        );
    }
}
