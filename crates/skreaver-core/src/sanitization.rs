//! Unified sanitization utilities for the Skreaver framework
//!
//! This module provides consistent sanitization functions to prevent information
//! disclosure and ensure data safety across the codebase.
//!
//! # Categories
//!
//! 1. **Identifier Sanitization** - Make strings safe for use as identifiers
//! 2. **Error Sanitization** - Remove sensitive information from error messages
//! 3. **Secret Redaction** - Detect and redact secrets from output
//! 4. **Control Character Removal** - Remove potentially dangerous characters
//!
//! # Examples
//!
//! ```rust
//! use skreaver_core::sanitization::{SanitizeIdentifier, SanitizeError};
//!
//! // Sanitize identifier
//! let safe_id = "User Name!".sanitize_identifier();
//! assert_eq!(safe_id, "User_Name_");
//!
//! // Sanitize error (removes sensitive info)
//! let error = "Connection failed: password=secret123";
//! let safe_error = error.sanitize_error(&["password"]);
//! assert!(safe_error.contains("***"));
//! ```

/// Maximum length for sanitized identifiers
pub const MAX_IDENTIFIER_LENGTH: usize = 128;

/// Maximum length for sanitized error messages
pub const MAX_ERROR_MESSAGE_LENGTH: usize = 200;

/// Trait for sanitizing strings to make them safe identifiers
pub trait SanitizeIdentifier {
    /// Sanitize a string to make it a valid identifier
    ///
    /// This replaces invalid characters with underscores and truncates to max length.
    /// Useful for generating identifiers from user input.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use skreaver_core::sanitization::SanitizeIdentifier;
    ///
    /// assert_eq!("Hello World!".sanitize_identifier(), "Hello_World_");
    /// assert_eq!("agent/path".sanitize_identifier(), "agent_path");
    /// assert_eq!("  spaces  ".sanitize_identifier(), "spaces");
    /// ```
    fn sanitize_identifier(&self) -> String;
}

impl SanitizeIdentifier for str {
    fn sanitize_identifier(&self) -> String {
        let trimmed = self.trim();
        if trimmed.is_empty() {
            return "unnamed".to_string();
        }

        let sanitized: String = trimmed
            .chars()
            .map(|c| {
                // Valid characters for identifiers: alphanumeric, -, _, .
                if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .collect();

        // Truncate to max length
        if sanitized.len() > MAX_IDENTIFIER_LENGTH {
            sanitized[..MAX_IDENTIFIER_LENGTH].to_string()
        } else {
            sanitized
        }
    }
}

impl SanitizeIdentifier for String {
    fn sanitize_identifier(&self) -> String {
        self.as_str().sanitize_identifier()
    }
}

/// Trait for sanitizing error messages to remove sensitive information
pub trait SanitizeError {
    /// Sanitize error message by redacting sensitive patterns
    ///
    /// # Parameters
    ///
    /// * `patterns` - Patterns to redact (e.g., ["password", "token", "key"])
    ///
    /// # Examples
    ///
    /// ```rust
    /// use skreaver_core::sanitization::SanitizeError;
    ///
    /// let error = "Connection failed: password=secret123";
    /// let safe = error.sanitize_error(&["password"]);
    /// assert!(safe.contains("***"));
    /// ```
    fn sanitize_error(&self, patterns: &[&str]) -> String;

    /// Sanitize database error with common patterns
    fn sanitize_database_error(&self) -> String {
        self.sanitize_error(&["password", "auth", "token", "key", "secret", "credential"])
    }
}

impl SanitizeError for str {
    fn sanitize_error(&self, patterns: &[&str]) -> String {
        let mut sanitized = self.to_string();

        // Redact sensitive patterns
        for pattern in patterns {
            // Case-insensitive pattern matching
            let pattern_lower = pattern.to_lowercase();
            if sanitized.to_lowercase().contains(&pattern_lower) {
                // Replace the sensitive content
                sanitized = format!("*** (sensitive information redacted: {}) ***", pattern);
                break; // Stop after first match to avoid exposing multiple patterns
            }
        }

        // Truncate to max length
        if sanitized.len() > MAX_ERROR_MESSAGE_LENGTH {
            format!("{}...", &sanitized[..MAX_ERROR_MESSAGE_LENGTH - 3])
        } else {
            sanitized
        }
    }
}

impl SanitizeError for String {
    fn sanitize_error(&self, patterns: &[&str]) -> String {
        self.as_str().sanitize_error(patterns)
    }
}

/// Helper for sanitizing database-specific errors
pub struct DatabaseErrorSanitizer;

impl DatabaseErrorSanitizer {
    /// Sanitize a database error with generic safe messages
    ///
    /// This function categorizes errors into safe categories without revealing
    /// sensitive database schema or connection details.
    pub fn sanitize<E: std::fmt::Display>(error: &E) -> String {
        let error_str = error.to_string();
        let error_lower = error_str.to_lowercase();

        // Check for sensitive patterns
        if error_lower.contains("password")
            || error_lower.contains("auth")
            || error_lower.contains("credential")
        {
            return "Authentication failed".to_string();
        }

        if error_lower.contains("connection") || error_lower.contains("connect") {
            return "Connection failed".to_string();
        }

        if error_lower.contains("timeout") || error_lower.contains("timed out") {
            return "Operation timed out".to_string();
        }

        if error_lower.contains("permission") || error_lower.contains("access denied") {
            return "Permission denied".to_string();
        }

        // Generic safe message with limited detail
        let sanitized = error_str.chars().take(100).collect::<String>();
        if sanitized.len() < error_str.len() {
            "Database operation failed".to_string()
        } else {
            format!("Database error: {}", sanitized)
        }
    }

    /// Categorize error type for logging (safe for logs)
    pub fn categorize<E: std::fmt::Display>(error: &E) -> &'static str {
        let error_str = error.to_string().to_lowercase();

        if error_str.contains("connection") || error_str.contains("connect") {
            "connection"
        } else if error_str.contains("timeout") {
            "timeout"
        } else if error_str.contains("auth") || error_str.contains("permission") {
            "authorization"
        } else if error_str.contains("not found") || error_str.contains("no rows") {
            "not_found"
        } else if error_str.contains("constraint") || error_str.contains("unique") {
            "constraint_violation"
        } else if error_str.contains("syntax") || error_str.contains("invalid") {
            "invalid_input"
        } else {
            "unknown"
        }
    }
}

/// Helper for sanitizing control characters and dangerous content
pub struct ContentSanitizer;

impl ContentSanitizer {
    /// Remove control characters except whitespace
    ///
    /// # Examples
    ///
    /// ```rust
    /// use skreaver_core::sanitization::ContentSanitizer;
    ///
    /// let input = "Hello\x00World\x1b";
    /// let clean = ContentSanitizer::remove_control_chars(input);
    /// assert_eq!(clean, "HelloWorld");
    /// ```
    pub fn remove_control_chars(input: &str) -> String {
        input
            .chars()
            .filter(|c| !c.is_control() || c.is_whitespace())
            .collect()
    }

    /// Remove ANSI escape sequences
    pub fn remove_ansi_escapes(input: &str) -> String {
        // Simple ANSI escape sequence removal (ESC + [ + commands)
        let mut result = String::with_capacity(input.len());
        let mut chars = input.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '\x1b' {
                // ESC character
                if chars.peek() == Some(&'[') {
                    // Skip ANSI escape sequence
                    chars.next(); // Skip '['
                    while let Some(&ch) = chars.peek() {
                        chars.next();
                        if ch.is_ascii_alphabetic() {
                            break; // End of escape sequence
                        }
                    }
                } else {
                    result.push(c);
                }
            } else {
                result.push(c);
            }
        }

        result
    }

    /// Sanitize output for safe display (removes ANSI and control chars)
    pub fn sanitize_output(input: &str) -> String {
        // Remove ANSI first since it contains control chars
        let without_ansi = Self::remove_ansi_escapes(input);
        Self::remove_control_chars(&without_ansi)
    }
}

/// Helper for detecting and redacting secrets
pub struct SecretRedactor;

impl SecretRedactor {
    /// Common secret patterns (for detection)
    pub const SECRET_PATTERNS: &'static [&'static str] = &[
        "password",
        "passwd",
        "pwd",
        "secret",
        "token",
        "api_key",
        "apikey",
        "auth",
        "credential",
        "private_key",
        "access_token",
        "refresh_token",
    ];

    /// Check if a string might contain secrets
    pub fn might_contain_secrets(input: &str) -> bool {
        let input_lower = input.to_lowercase();
        Self::SECRET_PATTERNS
            .iter()
            .any(|pattern| input_lower.contains(pattern))
    }

    /// Redact potential secrets from a string
    ///
    /// # Examples
    ///
    /// ```rust
    /// use skreaver_core::sanitization::SecretRedactor;
    ///
    /// let input = "password=secret123&user=admin";
    /// let redacted = SecretRedactor::redact_secrets(input);
    /// assert!(redacted.contains("***"));
    /// ```
    pub fn redact_secrets(input: &str) -> String {
        if !Self::might_contain_secrets(input) {
            return input.to_string();
        }

        let mut result = input.to_string();

        // Redact key=value patterns for known secrets
        for pattern in Self::SECRET_PATTERNS {
            // Match pattern=value or pattern:value or pattern="value"
            let patterns_to_match = [
                format!("{}=", pattern),
                format!("{}:", pattern),
                format!("{}=\"", pattern),
                format!("{}: ", pattern),
            ];

            for match_pattern in &patterns_to_match {
                if let Some(idx) = result.to_lowercase().find(&match_pattern.to_lowercase()) {
                    // Find the end of the value (space, &, or end of string)
                    let value_start = idx + match_pattern.len();
                    let value_end = result[value_start..]
                        .find(|c: char| c.is_whitespace() || c == '&' || c == ';' || c == '"')
                        .map(|i| value_start + i)
                        .unwrap_or(result.len());

                    // Replace value with ***
                    result.replace_range(value_start..value_end, "***");
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_identifier() {
        assert_eq!("Hello World!".sanitize_identifier(), "Hello_World_");
        assert_eq!("agent/path".sanitize_identifier(), "agent_path");
        assert_eq!("  spaces  ".sanitize_identifier(), "spaces");
        assert_eq!("".sanitize_identifier(), "unnamed");
        assert_eq!("valid-name_123".sanitize_identifier(), "valid-name_123");
    }

    #[test]
    fn test_sanitize_error() {
        let error = "Connection failed: password=secret123";
        let safe = error.sanitize_error(&["password"]);
        assert!(safe.contains("***"));
        assert!(!safe.contains("secret123"));
    }

    #[test]
    fn test_database_error_sanitizer() {
        assert_eq!(
            DatabaseErrorSanitizer::sanitize(&"password authentication failed"),
            "Authentication failed"
        );
        assert_eq!(
            DatabaseErrorSanitizer::sanitize(&"connection refused"),
            "Connection failed"
        );
        assert_eq!(
            DatabaseErrorSanitizer::sanitize(&"operation timed out"),
            "Operation timed out"
        );
    }

    #[test]
    fn test_categorize_error() {
        assert_eq!(
            DatabaseErrorSanitizer::categorize(&"connection refused"),
            "connection"
        );
        assert_eq!(
            DatabaseErrorSanitizer::categorize(&"timeout occurred"),
            "timeout"
        );
        assert_eq!(
            DatabaseErrorSanitizer::categorize(&"no rows returned"),
            "not_found"
        );
    }

    #[test]
    fn test_remove_control_chars() {
        let input = "Hello\x00World\x1b";
        let clean = ContentSanitizer::remove_control_chars(input);
        assert_eq!(clean, "HelloWorld");

        // Whitespace should be preserved
        let input_with_space = "Hello World\n";
        let clean = ContentSanitizer::remove_control_chars(input_with_space);
        assert_eq!(clean, "Hello World\n");
    }

    #[test]
    fn test_remove_ansi_escapes() {
        let input = "\x1b[31mRed Text\x1b[0m Normal";
        let clean = ContentSanitizer::remove_ansi_escapes(input);
        assert_eq!(clean, "Red Text Normal");
    }

    #[test]
    fn test_secret_detection() {
        assert!(SecretRedactor::might_contain_secrets("password=123"));
        assert!(SecretRedactor::might_contain_secrets("api_key=abc"));
        assert!(!SecretRedactor::might_contain_secrets("username=admin"));
    }

    #[test]
    fn test_redact_secrets() {
        let input = "password=secret123&user=admin";
        let redacted = SecretRedactor::redact_secrets(input);
        assert!(redacted.contains("***"));
        assert!(!redacted.contains("secret123"));
        assert!(redacted.contains("user=admin")); // Non-secret preserved
    }

    #[test]
    fn test_sanitize_output() {
        let input = "Hello\x00World\x1b[31mRed\x1b[0m";
        let clean = ContentSanitizer::sanitize_output(input);
        assert_eq!(clean, "HelloWorldRed");
    }
}
