//! Shared validation utilities for identifiers across the codebase
//!
//! This module provides consistent validation logic for string-based identifiers
//! like tool names, memory keys, etc.

/// Character policy for identifiers - defines which special characters are allowed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterPolicy {
    /// Only alphanumeric, underscores, and hyphens
    Strict,
    /// Also allow dots (e.g., "domain.name")
    WithDots,
    /// Also allow colons (e.g., "namespace:resource")
    WithColons,
    /// Allow both dots and colons (e.g., "user.settings:v1")
    WithDotsAndColons,
}

impl CharacterPolicy {
    /// Check if a character is allowed under this policy
    fn is_allowed(&self, ch: char) -> bool {
        if ch.is_alphanumeric() || ch == '_' || ch == '-' {
            return true;
        }

        match self {
            CharacterPolicy::Strict => false,
            CharacterPolicy::WithDots => ch == '.',
            CharacterPolicy::WithColons => ch == ':',
            CharacterPolicy::WithDotsAndColons => ch == '.' || ch == ':',
        }
    }
}

/// Path safety policy - defines how path-like characters are handled
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathSafetyPolicy {
    /// Reject all path characters (/, \) and check for path traversal sequences
    NoPathCharacters,
    /// Allow slashes but check for path traversal sequences (../, ./)
    AllowPathsWithTraversalChecks,
}

impl PathSafetyPolicy {
    /// Check if slashes are allowed
    fn allows_slashes(&self) -> bool {
        matches!(self, PathSafetyPolicy::AllowPathsWithTraversalChecks)
    }

    /// Check if path traversal checking should be performed
    /// Both policies check for traversal - one rejects slashes entirely,
    /// the other allows them but still checks for dangerous patterns
    fn should_check_traversal(&self) -> bool {
        true // Always check for path traversal
    }
}

/// Whitespace handling policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhitespaceHandling {
    /// Preserve whitespace as-is, reject if leading/trailing
    Preserve,
    /// Trim leading and trailing whitespace before validation
    TrimEdges,
}

/// Validation rules for string identifiers
///
/// This struct uses enum-based policies instead of booleans to make
/// invalid state combinations impossible at compile time.
#[derive(Debug, Clone, Copy)]
pub struct IdentifierRules {
    /// Maximum allowed length in characters
    pub max_length: usize,
    /// Character policy - which special characters are allowed
    pub character_policy: CharacterPolicy,
    /// Path safety policy - how to handle path-like characters
    pub path_safety: PathSafetyPolicy,
    /// Whitespace handling policy
    pub whitespace_handling: WhitespaceHandling,
}

impl IdentifierRules {
    /// Standard rules for tool names
    ///
    /// - Max length: 64 characters
    /// - Allows: alphanumeric, `_`, `-`
    /// - Disallows: `.`, `:`, `/`, spaces, and other special characters
    /// - Trims whitespace before validation
    pub const TOOL_NAME: Self = Self {
        max_length: 64,
        character_policy: CharacterPolicy::Strict,
        path_safety: PathSafetyPolicy::NoPathCharacters,
        whitespace_handling: WhitespaceHandling::TrimEdges,
    };

    /// Standard rules for memory keys
    ///
    /// - Max length: 128 characters
    /// - Allows: alphanumeric, `_`, `-`, `.`, `:`
    /// - Disallows: `/`, spaces, and other special characters
    /// - Trims whitespace before validation
    ///
    /// The additional characters (`.` and `:`) enable namespacing patterns like:
    /// - `user.settings`
    /// - `cache:session:123`
    pub const MEMORY_KEY: Self = Self {
        max_length: 128,
        character_policy: CharacterPolicy::WithDotsAndColons,
        path_safety: PathSafetyPolicy::NoPathCharacters,
        whitespace_handling: WhitespaceHandling::TrimEdges,
    };

    /// Standard rules for general identifiers (AgentId, ToolId, SessionId)
    ///
    /// - Max length: 128 characters
    /// - Allows: alphanumeric, `_`, `-`, `.`
    /// - Disallows: `:`, `/`, spaces, and other special characters
    /// - Preserves whitespace (rejects if leading/trailing)
    pub const IDENTIFIER: Self = Self {
        max_length: 128,
        character_policy: CharacterPolicy::WithDots,
        path_safety: PathSafetyPolicy::NoPathCharacters,
        whitespace_handling: WhitespaceHandling::Preserve,
    };

    /// Validate a string against these rules
    ///
    /// # Parameters
    ///
    /// * `input` - The string to validate
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The validated string (trimmed if whitespace_handling is TrimEdges)
    /// * `Err(ValidationError)` - Description of validation failure
    pub fn validate(&self, input: &str) -> Result<String, ValidationError> {
        // Apply whitespace handling
        let processed = match self.whitespace_handling {
            WhitespaceHandling::TrimEdges => input.trim(),
            WhitespaceHandling::Preserve => input,
        };

        // Check for empty
        if processed.is_empty() {
            return Err(ValidationError::Empty);
        }

        // Check for whitespace-only (if preserving and original had whitespace)
        if matches!(self.whitespace_handling, WhitespaceHandling::Preserve)
            && input.trim().is_empty()
        {
            return Err(ValidationError::WhitespaceOnly);
        }

        // Check for leading/trailing whitespace when preserving
        if matches!(self.whitespace_handling, WhitespaceHandling::Preserve) && input != input.trim()
        {
            return Err(ValidationError::LeadingTrailingWhitespace);
        }

        // Check length
        if processed.len() > self.max_length {
            return Err(ValidationError::TooLong {
                length: processed.len(),
                max: self.max_length,
            });
        }

        // Check for path traversal sequences if policy requires it
        if self.path_safety.should_check_traversal()
            && (processed.contains("../") || processed.contains("./"))
        {
            return Err(ValidationError::PathTraversal);
        }

        // Check for invalid characters
        for ch in processed.chars() {
            // Check against character policy
            let char_allowed = self.character_policy.is_allowed(ch);

            // Check against path safety policy for slashes
            let slash_allowed = (ch == '/' || ch == '\\') && self.path_safety.allows_slashes();

            if !char_allowed && !slash_allowed {
                return Err(ValidationError::InvalidChar {
                    char: ch,
                    input: processed.to_string(),
                });
            }
        }

        Ok(processed.to_string())
    }
}

/// Errors that can occur during identifier validation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// Identifier is empty
    Empty,
    /// Identifier contains only whitespace
    WhitespaceOnly,
    /// Identifier has leading or trailing whitespace
    LeadingTrailingWhitespace,
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
    /// Identifier contains path traversal sequences
    PathTraversal,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::Empty => write!(f, "Identifier cannot be empty"),
            ValidationError::WhitespaceOnly => write!(f, "Identifier cannot be whitespace-only"),
            ValidationError::LeadingTrailingWhitespace => {
                write!(f, "Identifier cannot have leading or trailing whitespace")
            }
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
            ValidationError::PathTraversal => {
                write!(
                    f,
                    "Identifier cannot contain path traversal sequences (../)"
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

    #[test]
    fn test_path_traversal_detection() {
        let rules = IdentifierRules::MEMORY_KEY;

        assert!(matches!(
            rules.validate("../etc"),
            Err(ValidationError::PathTraversal)
        ));
        assert!(matches!(
            rules.validate("./file"),
            Err(ValidationError::PathTraversal)
        ));
        assert!(matches!(
            rules.validate("path/../other"),
            Err(ValidationError::PathTraversal)
        ));
    }

    #[test]
    fn test_identifier_rules() {
        let rules = IdentifierRules::IDENTIFIER;

        // Valid identifiers
        assert!(rules.validate("agent-1").is_ok());
        assert!(rules.validate("my_agent").is_ok());
        assert!(rules.validate("agent.123").is_ok());

        // Invalid - leading/trailing whitespace (no trimming for IDENTIFIER)
        assert!(matches!(
            rules.validate(" agent"),
            Err(ValidationError::LeadingTrailingWhitespace)
        ));
        assert!(matches!(
            rules.validate("agent "),
            Err(ValidationError::LeadingTrailingWhitespace)
        ));

        // Invalid - whitespace only
        assert!(matches!(
            rules.validate("   "),
            Err(ValidationError::WhitespaceOnly)
        ));

        // Invalid - colons not allowed
        assert!(matches!(
            rules.validate("agent:123"),
            Err(ValidationError::InvalidChar { char: ':', .. })
        ));

        // Invalid - path traversal
        assert!(matches!(
            rules.validate("../agent"),
            Err(ValidationError::PathTraversal)
        ));
    }
}
