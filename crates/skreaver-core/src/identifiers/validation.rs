//! Identifier validation rules and utilities
//!
//! This module provides identifier validation that builds on the shared
//! validation infrastructure in `crate::validation`.
//!
//! # Consolidation Status
//!
//! This module has been consolidated with `crate::validation` to reduce duplication:
//! - `IdValidator` now uses `IdentifierRules::IDENTIFIER` internally
//! - `IdValidationError` is compatible with `ValidationError` via `From` traits
//! - New code should prefer `IdentifierRules` directly for more flexibility
//!
//! # Migration Path
//!
//! For new code, prefer using `IdentifierRules` directly:
//!
//! ```rust
//! use skreaver_core::validation::IdentifierRules;
//!
//! // Instead of IdValidator::validate(id)
//! let validated = IdentifierRules::IDENTIFIER.validate(id)?;
//! ```
//!
//! Existing code using `IdValidator` will continue to work without changes.

use crate::validation::{IdentifierRules, ValidationError};

/// Maximum length for all identifier types
/// Note: Uses the same value as `crate::sanitization::MAX_IDENTIFIER_LENGTH`
#[cfg(test)]
const MAX_ID_LENGTH: usize = crate::sanitization::MAX_IDENTIFIER_LENGTH;

/// Error type for identifier validation failures
///
/// This is a wrapper around `ValidationError` with conversion support
/// for backward compatibility.
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

impl std::fmt::Display for IdValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

/// Convert ValidationError to IdValidationError for backward compatibility
impl From<ValidationError> for IdValidationError {
    fn from(err: ValidationError) -> Self {
        match err {
            ValidationError::Empty => IdValidationError::Empty,
            ValidationError::WhitespaceOnly => IdValidationError::WhitespaceOnly,
            ValidationError::LeadingTrailingWhitespace => {
                IdValidationError::LeadingTrailingWhitespace
            }
            ValidationError::TooLong { length, max } => IdValidationError::TooLong { length, max },
            ValidationError::InvalidChar { .. } => IdValidationError::InvalidCharacters,
            ValidationError::PathTraversal => IdValidationError::PathTraversal,
        }
    }
}

/// Convert IdValidationError to ValidationError
impl From<IdValidationError> for ValidationError {
    fn from(err: IdValidationError) -> Self {
        match err {
            IdValidationError::Empty => ValidationError::Empty,
            IdValidationError::WhitespaceOnly => ValidationError::WhitespaceOnly,
            IdValidationError::LeadingTrailingWhitespace => {
                ValidationError::LeadingTrailingWhitespace
            }
            IdValidationError::TooLong { length, max } => ValidationError::TooLong { length, max },
            IdValidationError::InvalidCharacters => ValidationError::InvalidChar {
                char: ' ',
                input: String::new(),
            },
            IdValidationError::PathTraversal => ValidationError::PathTraversal,
        }
    }
}

/// Validator for identifier strings
///
/// This validator now uses the shared `IdentifierRules` infrastructure
/// for consistency across the codebase.
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
        // Use shared validation infrastructure
        IdentifierRules::IDENTIFIER
            .validate(id)
            .map(|_| id) // Return original &str instead of String
            .map_err(|e| e.into()) // Convert ValidationError to IdValidationError
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
        use crate::sanitization::SanitizeIdentifier;
        input.sanitize_identifier()
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
