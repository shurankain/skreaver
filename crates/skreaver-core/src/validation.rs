//! Shared validation utilities for identifiers across the codebase
//!
//! This module provides consistent validation logic for string-based identifiers
//! like tool names, memory keys, etc.

/// Validation rules for string identifiers
#[derive(Debug, Clone, Copy)]
pub struct IdentifierRules {
    /// Maximum allowed length in characters
    pub max_length: usize,
    /// Whether to allow dots (.) in the identifier
    pub allow_dots: bool,
    /// Whether to allow colons (:) in the identifier
    pub allow_colons: bool,
    /// Whether to allow forward slashes (/) in the identifier
    pub allow_slashes: bool,
}

impl IdentifierRules {
    /// Standard rules for tool names
    ///
    /// - Max length: 64 characters
    /// - Allows: alphanumeric, `_`, `-`
    /// - Disallows: `.`, `:`, `/`, spaces, and other special characters
    pub const TOOL_NAME: Self = Self {
        max_length: 64,
        allow_dots: false,
        allow_colons: false,
        allow_slashes: false,
    };

    /// Standard rules for memory keys
    ///
    /// - Max length: 128 characters
    /// - Allows: alphanumeric, `_`, `-`, `.`, `:`
    /// - Disallows: `/`, spaces, and other special characters
    ///
    /// The additional characters (`.` and `:`) enable namespacing patterns like:
    /// - `user.settings`
    /// - `cache:session:123`
    pub const MEMORY_KEY: Self = Self {
        max_length: 128,
        allow_dots: true,
        allow_colons: true,
        allow_slashes: false,
    };

    /// Validate a string against these rules
    ///
    /// # Parameters
    ///
    /// * `input` - The string to validate
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The trimmed, validated string
    /// * `Err(ValidationError)` - Description of validation failure
    pub fn validate(&self, input: &str) -> Result<String, ValidationError> {
        let trimmed = input.trim();

        if trimmed.is_empty() {
            return Err(ValidationError::Empty);
        }

        if trimmed.len() > self.max_length {
            return Err(ValidationError::TooLong {
                length: trimmed.len(),
                max: self.max_length,
            });
        }

        // Check for invalid characters
        for ch in trimmed.chars() {
            let is_valid = ch.is_alphanumeric()
                || ch == '_'
                || ch == '-'
                || (ch == '.' && self.allow_dots)
                || (ch == ':' && self.allow_colons)
                || (ch == '/' && self.allow_slashes);

            if !is_valid {
                return Err(ValidationError::InvalidChar {
                    char: ch,
                    input: trimmed.to_string(),
                });
            }
        }

        Ok(trimmed.to_string())
    }
}

/// Errors that can occur during identifier validation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// Identifier is empty or contains only whitespace
    Empty,
    /// Identifier exceeds maximum allowed length
    TooLong {
        /// Actual length
        length: usize,
        /// Maximum allowed length
        max: usize,
    },
    /// Identifier contains an invalid character
    InvalidChar {
        /// The invalid character
        char: char,
        /// The full input string
        input: String,
    },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::Empty => write!(f, "Identifier cannot be empty"),
            ValidationError::TooLong { length, max } => {
                write!(
                    f,
                    "Identifier too long: {} characters (max {})",
                    length, max
                )
            }
            ValidationError::InvalidChar { char, input } => {
                write!(
                    f,
                    "Identifier '{}' contains invalid character '{}'",
                    input, char
                )
            }
        }
    }
}

impl std::error::Error for ValidationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_name_rules() {
        let rules = IdentifierRules::TOOL_NAME;

        // Valid tool names
        assert!(rules.validate("calculator").is_ok());
        assert!(rules.validate("tool_name").is_ok());
        assert!(rules.validate("tool-name").is_ok());
        assert!(rules.validate("Tool123").is_ok());

        // Invalid tool names
        assert!(matches!(rules.validate(""), Err(ValidationError::Empty)));
        assert!(matches!(rules.validate("   "), Err(ValidationError::Empty)));
        assert!(matches!(
            rules.validate("tool with spaces"),
            Err(ValidationError::InvalidChar { .. })
        ));
        assert!(matches!(
            rules.validate("tool.name"),
            Err(ValidationError::InvalidChar { char: '.', .. })
        ));
        assert!(matches!(
            rules.validate("tool:name"),
            Err(ValidationError::InvalidChar { char: ':', .. })
        ));

        // Too long
        let long_name = "a".repeat(65);
        assert!(matches!(
            rules.validate(&long_name),
            Err(ValidationError::TooLong { .. })
        ));
    }

    #[test]
    fn test_memory_key_rules() {
        let rules = IdentifierRules::MEMORY_KEY;

        // Valid memory keys
        assert!(rules.validate("user_context").is_ok());
        assert!(rules.validate("cache.session").is_ok());
        assert!(rules.validate("user:12345").is_ok());
        assert!(rules.validate("nested.key.path").is_ok());
        assert!(rules.validate("cache:session:user").is_ok());

        // Invalid memory keys
        assert!(matches!(rules.validate(""), Err(ValidationError::Empty)));
        assert!(matches!(
            rules.validate("key with spaces"),
            Err(ValidationError::InvalidChar { .. })
        ));
        assert!(matches!(
            rules.validate("key/path"),
            Err(ValidationError::InvalidChar { char: '/', .. })
        ));

        // Too long
        let long_key = "a".repeat(129);
        assert!(matches!(
            rules.validate(&long_key),
            Err(ValidationError::TooLong { .. })
        ));
    }

    #[test]
    fn test_trimming() {
        let rules = IdentifierRules::TOOL_NAME;

        let result = rules.validate("  tool_name  ").unwrap();
        assert_eq!(result, "tool_name");
    }
}
