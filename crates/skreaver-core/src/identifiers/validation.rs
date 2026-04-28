//! Identifier validation rules and utilities.
//!
//! Uses the shared `IdentifierRules` infrastructure from `crate::validation`.

use crate::validation::{IdentifierRules, ValidationError};

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
    pub fn validate(id: &str) -> Result<&str, ValidationError> {
        // Use shared validation infrastructure
        IdentifierRules::IDENTIFIER.validate(id).map(|_| id) // Return original &str instead of String
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
        assert_eq!(IdValidator::validate(""), Err(ValidationError::Empty));
    }

    #[test]
    fn test_validate_whitespace_only() {
        assert_eq!(
            IdValidator::validate("   "),
            Err(ValidationError::WhitespaceOnly)
        );
        assert_eq!(
            IdValidator::validate("\t\n"),
            Err(ValidationError::WhitespaceOnly)
        );
    }

    #[test]
    fn test_validate_leading_trailing_whitespace() {
        assert_eq!(
            IdValidator::validate(" agent"),
            Err(ValidationError::LeadingTrailingWhitespace)
        );
        assert_eq!(
            IdValidator::validate("agent "),
            Err(ValidationError::LeadingTrailingWhitespace)
        );
        assert_eq!(
            IdValidator::validate(" agent "),
            Err(ValidationError::LeadingTrailingWhitespace)
        );
    }

    #[test]
    fn test_validate_invalid_characters() {
        assert!(matches!(
            IdValidator::validate("agent/path"),
            Err(ValidationError::InvalidChar { .. })
        ));
        assert!(matches!(
            IdValidator::validate("agent@host"),
            Err(ValidationError::InvalidChar { .. })
        ));
        assert!(matches!(
            IdValidator::validate("agent:port"),
            Err(ValidationError::InvalidChar { .. })
        ));
    }

    #[test]
    fn test_validate_path_traversal() {
        assert_eq!(
            IdValidator::validate("../etc"),
            Err(ValidationError::PathTraversal)
        );
        assert_eq!(
            IdValidator::validate("./file"),
            Err(ValidationError::PathTraversal)
        );
        assert_eq!(
            IdValidator::validate("path/../other"),
            Err(ValidationError::PathTraversal)
        );
    }

    #[test]
    fn test_validate_too_long() {
        let long_id = "a".repeat(129);
        match IdValidator::validate(&long_id) {
            Err(ValidationError::TooLong { length, max }) => {
                assert_eq!(length, 129);
                assert_eq!(max, 128);
            }
            _ => panic!("Expected TooLong error"),
        }
    }

    #[test]
    fn test_validate_max_length_ok() {
        let max_id = "a".repeat(128);
        assert!(IdValidator::validate(&max_id).is_ok());
    }

}
