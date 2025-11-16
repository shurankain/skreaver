//! # Specific Error Types for Agent Operations
//!
//! This module provides detailed, type-safe error handling for agent operations.
//! Instead of generic `Result<T, String>`, we use specific error types that:
//!
//! - Provide structured error information
//! - Enable programmatic error handling
//! - Improve error messages and debugging
//! - Make error cases explicit in the type system
//!
//! # Problem: Generic String Errors
//!
//! Generic error types lose information and make handling difficult:
//!
//! ```ignore
//! fn create_agent(config: Config) -> Result<Agent, String> {
//!     // Error could be anything - caller can't distinguish!
//!     if config.is_empty() {
//!         return Err("Config is empty".to_string());
//!     }
//!     if !config.contains_key("mode") {
//!         return Err("Missing mode".to_string());
//!     }
//!     // ... more errors
//! }
//!
//! // Caller must parse strings to understand what went wrong
//! match create_agent(config) {
//!     Err(e) if e.contains("empty") => handle_empty(),
//!     Err(e) if e.contains("mode") => handle_missing_mode(),
//!     Err(e) => handle_unknown(e), // Fragile string matching!
//! }
//! ```
//!
//! # Solution: Specific Error Types
//!
//! Use enums to represent all possible error cases:
//!
//! ```ignore
//! fn create_agent(config: Config) -> Result<Agent, AgentBuildError> {
//!     if config.is_empty() {
//!         return Err(AgentBuildError::EmptyConfig);
//!     }
//!     if !config.contains_key("mode") {
//!         return Err(AgentBuildError::MissingField {
//!             field: "mode".to_string(),
//!         });
//!     }
//!     // ... more errors
//! }
//!
//! // Caller can match on specific error types
//! match create_agent(config) {
//!     Err(AgentBuildError::EmptyConfig) => handle_empty(),
//!     Err(AgentBuildError::MissingField { field }) => handle_missing(field),
//!     Err(e) => handle_other(e), // Type-safe!
//! }
//! ```

use serde_json::Value;
use std::collections::HashMap;

/// Errors that can occur when building or configuring an agent
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentBuildError {
    /// Agent configuration is empty
    EmptyConfig,

    /// Required configuration field is missing
    MissingField {
        /// Name of the missing field
        field: String,
    },

    /// Configuration field has invalid type
    InvalidFieldType {
        /// Name of the field
        field: String,
        /// Expected type
        expected: String,
        /// Actual type
        actual: String,
    },

    /// Configuration field has invalid value
    InvalidFieldValue {
        /// Name of the field
        field: String,
        /// The invalid value
        value: String,
        /// Reason why it's invalid
        reason: String,
    },

    /// Processing mode is invalid
    InvalidProcessingMode {
        /// The invalid mode
        mode: String,
        /// Valid modes
        valid_modes: Vec<String>,
    },

    /// Agent type is not supported
    UnsupportedAgentType {
        /// The unsupported type
        agent_type: String,
    },

    /// Memory initialization failed
    MemoryInitializationFailed {
        /// Error message from memory system
        error: String,
    },

    /// Tool registry setup failed
    ToolRegistryFailed {
        /// Error message from tool system
        error: String,
    },

    /// Custom validation error
    ValidationFailed {
        /// What failed validation
        what: String,
        /// Why it failed
        reason: String,
    },
}

impl AgentBuildError {
    /// Create an error for a missing required field
    pub fn missing_field(field: impl Into<String>) -> Self {
        Self::MissingField {
            field: field.into(),
        }
    }

    /// Create an error for an invalid field type
    pub fn invalid_type(
        field: impl Into<String>,
        expected: impl Into<String>,
        value: &Value,
    ) -> Self {
        Self::InvalidFieldType {
            field: field.into(),
            expected: expected.into(),
            actual: match value {
                Value::Null => "null",
                Value::Bool(_) => "boolean",
                Value::Number(_) => "number",
                Value::String(_) => "string",
                Value::Array(_) => "array",
                Value::Object(_) => "object",
            }
            .to_string(),
        }
    }

    /// Create an error for an invalid field value
    pub fn invalid_value(
        field: impl Into<String>,
        value: impl ToString,
        reason: impl Into<String>,
    ) -> Self {
        Self::InvalidFieldValue {
            field: field.into(),
            value: value.to_string(),
            reason: reason.into(),
        }
    }

    /// Create an error for an invalid processing mode
    pub fn invalid_mode(mode: impl Into<String>, valid_modes: Vec<String>) -> Self {
        Self::InvalidProcessingMode {
            mode: mode.into(),
            valid_modes,
        }
    }
}

impl std::fmt::Display for AgentBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyConfig => {
                write!(f, "Agent configuration is empty")
            }
            Self::MissingField { field } => {
                write!(f, "Missing required configuration field: '{}'", field)
            }
            Self::InvalidFieldType {
                field,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Invalid type for field '{}': expected {}, got {}",
                    field, expected, actual
                )
            }
            Self::InvalidFieldValue {
                field,
                value,
                reason,
            } => {
                write!(
                    f,
                    "Invalid value for field '{}': '{}' - {}",
                    field, value, reason
                )
            }
            Self::InvalidProcessingMode { mode, valid_modes } => {
                write!(
                    f,
                    "Invalid processing mode '{}'. Valid modes: {}",
                    mode,
                    valid_modes.join(", ")
                )
            }
            Self::UnsupportedAgentType { agent_type } => {
                write!(f, "Unsupported agent type: '{}'", agent_type)
            }
            Self::MemoryInitializationFailed { error } => {
                write!(f, "Memory initialization failed: {}", error)
            }
            Self::ToolRegistryFailed { error } => {
                write!(f, "Tool registry setup failed: {}", error)
            }
            Self::ValidationFailed { what, reason } => {
                write!(f, "Validation failed for {}: {}", what, reason)
            }
        }
    }
}

impl std::error::Error for AgentBuildError {}

/// Helper functions for extracting typed values from config
pub trait ConfigExt {
    /// Get a required string field
    fn get_string(&self, field: &str) -> Result<String, AgentBuildError>;

    /// Get an optional string field with default
    fn get_string_or(&self, field: &str, default: &str) -> String;

    /// Get a required boolean field
    fn get_bool(&self, field: &str) -> Result<bool, AgentBuildError>;

    /// Get an optional boolean field with default
    fn get_bool_or(&self, field: &str, default: bool) -> bool;

    /// Get a required integer field
    fn get_i64(&self, field: &str) -> Result<i64, AgentBuildError>;

    /// Get an optional integer field with default
    fn get_i64_or(&self, field: &str, default: i64) -> i64;
}

impl ConfigExt for HashMap<String, Value> {
    fn get_string(&self, field: &str) -> Result<String, AgentBuildError> {
        self.get(field)
            .ok_or_else(|| AgentBuildError::missing_field(field))?
            .as_str()
            .ok_or_else(|| AgentBuildError::invalid_type(field, "string", self.get(field).unwrap()))
            .map(|s| s.to_string())
    }

    fn get_string_or(&self, field: &str, default: &str) -> String {
        self.get(field)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| default.to_string())
    }

    fn get_bool(&self, field: &str) -> Result<bool, AgentBuildError> {
        self.get(field)
            .ok_or_else(|| AgentBuildError::missing_field(field))?
            .as_bool()
            .ok_or_else(|| {
                AgentBuildError::invalid_type(field, "boolean", self.get(field).unwrap())
            })
    }

    fn get_bool_or(&self, field: &str, default: bool) -> bool {
        self.get(field).and_then(|v| v.as_bool()).unwrap_or(default)
    }

    fn get_i64(&self, field: &str) -> Result<i64, AgentBuildError> {
        self.get(field)
            .ok_or_else(|| AgentBuildError::missing_field(field))?
            .as_i64()
            .ok_or_else(|| {
                AgentBuildError::invalid_type(field, "integer", self.get(field).unwrap())
            })
    }

    fn get_i64_or(&self, field: &str, default: i64) -> i64 {
        self.get(field).and_then(|v| v.as_i64()).unwrap_or(default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = AgentBuildError::missing_field("mode");
        assert_eq!(
            err.to_string(),
            "Missing required configuration field: 'mode'"
        );

        let err = AgentBuildError::invalid_type("count", "integer", &Value::String("10".into()));
        assert_eq!(
            err.to_string(),
            "Invalid type for field 'count': expected integer, got string"
        );

        let err =
            AgentBuildError::invalid_mode("unknown", vec!["simple".into(), "advanced".into()]);
        assert_eq!(
            err.to_string(),
            "Invalid processing mode 'unknown'. Valid modes: simple, advanced"
        );
    }

    #[test]
    fn test_config_ext_get_string() {
        let mut config = HashMap::new();
        config.insert("name".to_string(), Value::String("test".into()));

        assert_eq!(config.get_string("name").unwrap(), "test");
        assert!(config.get_string("missing").is_err());

        config.insert("not_string".to_string(), Value::Number(42.into()));
        assert!(config.get_string("not_string").is_err());
    }

    #[test]
    fn test_config_ext_get_bool() {
        let mut config = HashMap::new();
        config.insert("enabled".to_string(), Value::Bool(true));

        assert_eq!(config.get_bool("enabled").unwrap(), true);
        assert_eq!(config.get_bool_or("enabled", false), true);
        assert_eq!(config.get_bool_or("missing", false), false);
    }

    #[test]
    fn test_config_ext_get_i64() {
        let mut config = HashMap::new();
        config.insert("count".to_string(), Value::Number(42.into()));

        assert_eq!(config.get_i64("count").unwrap(), 42);
        assert_eq!(config.get_i64_or("count", 0), 42);
        assert_eq!(config.get_i64_or("missing", 10), 10);
    }
}
