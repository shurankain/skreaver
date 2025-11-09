//! Unified identifier types for the Skreaver framework
//!
//! This module provides validated, type-safe identifiers used throughout all
//! Skreaver crates. All identifiers enforce consistent validation rules and
//! provide compile-time type safety to prevent mixing different identifier types.
//!
//! # Design Principles
//!
//! 1. **Parse-Don't-Validate**: All identifiers use `parse()` constructors that
//!    return `Result` instead of panicking on invalid input
//! 2. **Newtype Pattern**: Each identifier type is a distinct newtype preventing
//!    accidental mixing (can't pass `ToolId` where `AgentId` is expected)
//! 3. **Zero-Cost Abstractions**: Identifiers compile down to their underlying
//!    `String` representation with no runtime overhead
//! 4. **Consistent Validation**: All identifiers share the same validation rules
//!    for predictable behavior across the framework
//!
//! # Validation Rules
//!
//! All identifier types enforce these rules:
//! - Non-empty (minimum 1 character)
//! - Maximum 128 characters
//! - No leading or trailing whitespace
//! - Only alphanumeric characters, hyphens (`-`), underscores (`_`), and dots (`.`)
//! - No path traversal sequences (`../`, `./`)
//!
//! # Security Considerations
//!
//! The validation rules prevent several security issues:
//! - **Path Traversal**: Dots are allowed but `../` sequences are rejected
//! - **Injection Attacks**: Only safe characters allowed, no shell metacharacters
//! - **Normalization Issues**: No unicode combining characters or RTL markers
//!
//! # Examples
//!
//! ```rust
//! use skreaver_core::identifiers::{AgentId, ToolId, SessionId};
//!
//! // Valid identifiers
//! let agent = AgentId::parse("agent-1").unwrap();
//! let tool = ToolId::parse("calculator").unwrap();
//! let session = SessionId::parse("session_abc123").unwrap();
//!
//! // Invalid identifiers
//! assert!(AgentId::parse("").is_err());              // Empty
//! assert!(AgentId::parse("  agent  ").is_err());     // Whitespace
//! assert!(AgentId::parse("agent/path").is_err());    // Invalid char
//! assert!(AgentId::parse("../../../etc").is_err());  // Path traversal
//!
//! // Type safety - won't compile!
//! // fn use_agent(id: AgentId) { }
//! // use_agent(tool);  // Compile error: expected AgentId, found ToolId
//! ```

mod validation;

pub use validation::{IdValidationError, IdValidator};

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Unique identifier for an agent instance
///
/// Agents are the fundamental execution units in Skreaver. Each agent has a
/// unique identifier used for routing messages, tracking state, and authorization.
///
/// # Examples
///
/// ```rust
/// use skreaver_core::identifiers::AgentId;
///
/// // Create from validated string
/// let id = AgentId::parse("my-agent-123").unwrap();
/// assert_eq!(id.as_str(), "my-agent-123");
///
/// // FromStr trait support
/// let id: AgentId = "another-agent".parse().unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AgentId(String);

impl AgentId {
    /// Parse and validate an agent ID from a string
    ///
    /// Returns an error if the string violates validation rules (empty,
    /// too long, contains invalid characters, etc.)
    pub fn parse(id: impl AsRef<str>) -> Result<Self, crate::validation::ValidationError> {
        IdValidator::validate(id.as_ref()).map(|s| Self(s.to_string()))
    }

    /// Get the agent ID as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Create an agent ID without validation (for testing only)
    ///
    /// # Safety
    ///
    /// This bypasses all validation checks. Only use this in tests or when
    /// the input is guaranteed to be valid. For all user input, use `parse()`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use skreaver_core::identifiers::AgentId;
    /// // In tests only:
    /// let id = AgentId::new_unchecked("test-agent");
    /// ```
    #[doc(hidden)]
    pub fn new_unchecked(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for AgentId {
    type Err = crate::validation::ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl From<AgentId> for String {
    fn from(id: AgentId) -> Self {
        id.0
    }
}

impl TryFrom<String> for AgentId {
    type Error = crate::validation::ValidationError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

/// Unique identifier for a tool
///
/// Tools are the capabilities available to agents. Each tool has a unique
/// identifier used for discovery, authorization, and execution tracking.
///
/// # Examples
///
/// ```rust
/// use skreaver_core::identifiers::ToolId;
///
/// let calculator = ToolId::parse("calculator").unwrap();
/// let http_client = ToolId::parse("http-client").unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ToolId(String);

impl ToolId {
    /// Parse and validate a tool ID from a string
    pub fn parse(id: impl AsRef<str>) -> Result<Self, crate::validation::ValidationError> {
        IdValidator::validate(id.as_ref()).map(|s| Self(s.to_string()))
    }

    /// Get the tool ID as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Create a tool ID without validation (for testing only)
    #[doc(hidden)]
    pub fn new_unchecked(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl fmt::Display for ToolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for ToolId {
    type Err = crate::validation::ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl From<ToolId> for String {
    fn from(id: ToolId) -> Self {
        id.0
    }
}

impl TryFrom<String> for ToolId {
    type Error = crate::validation::ValidationError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

/// Unique identifier for a session
///
/// Sessions represent a conversation or interaction context between a user
/// and one or more agents. Session IDs are used for message correlation,
/// state management, and audit logging.
///
/// # Examples
///
/// ```rust
/// use skreaver_core::identifiers::SessionId;
///
/// let session = SessionId::parse("session-abc123").unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SessionId(String);

impl SessionId {
    /// Parse and validate a session ID from a string
    pub fn parse(id: impl AsRef<str>) -> Result<Self, crate::validation::ValidationError> {
        IdValidator::validate(id.as_ref()).map(|s| Self(s.to_string()))
    }

    /// Get the session ID as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Create a session ID without validation (for testing only)
    #[doc(hidden)]
    pub fn new_unchecked(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generate a new random session ID using UUID v4
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for SessionId {
    type Err = crate::validation::ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl From<SessionId> for String {
    fn from(id: SessionId) -> Self {
        id.0
    }
}

impl TryFrom<String> for SessionId {
    type Error = crate::validation::ValidationError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

/// Unique identifier for a request
///
/// Request IDs are used to track individual API requests, correlate logs,
/// and implement distributed tracing across the system.
///
/// # Examples
///
/// ```rust
/// use skreaver_core::identifiers::RequestId;
/// use uuid::Uuid;
///
/// // From UUID
/// let uuid = Uuid::new_v4();
/// let request = RequestId::parse(uuid.to_string()).unwrap();
///
/// // From string
/// let request = RequestId::parse("req-12345").unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RequestId(String);

impl RequestId {
    /// Parse and validate a request ID from a string
    pub fn parse(id: impl AsRef<str>) -> Result<Self, crate::validation::ValidationError> {
        IdValidator::validate(id.as_ref()).map(|s| Self(s.to_string()))
    }

    /// Get the request ID as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Create a request ID without validation (for testing only)
    #[doc(hidden)]
    pub fn new_unchecked(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generate a new random request ID using UUID v4
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for RequestId {
    type Err = crate::validation::ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl From<RequestId> for String {
    fn from(id: RequestId) -> Self {
        id.0
    }
}

impl TryFrom<String> for RequestId {
    type Error = crate::validation::ValidationError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

/// Validated principal identifier for authentication and authorization
///
/// Represents a user or service identity with security-first validation to prevent
/// injection attacks and audit log corruption.
///
/// # Security
///
/// `PrincipalId` enforces strict validation rules:
/// - Non-empty, maximum 256 characters (reasonable for emails/usernames)
/// - Only alphanumeric characters plus `@`, `.`, `-`, `_` (common in identities)
/// - No path traversal sequences (`../`, `./`)
/// - No SQL injection patterns (`;`, `--`, `/*`)
/// - No shell metacharacters
///
/// # Examples
///
/// ```rust
/// use skreaver_core::PrincipalId;
///
/// // Valid principals
/// let user = PrincipalId::parse("user@example.com").unwrap();
/// let service = PrincipalId::parse("service-account-123").unwrap();
/// let system = PrincipalId::parse("system.admin").unwrap();
///
/// // Invalid principals (injection attempts blocked)
/// assert!(PrincipalId::parse("admin'; DROP TABLE users--").is_err());
/// assert!(PrincipalId::parse("../etc/passwd").is_err());
/// assert!(PrincipalId::parse("user; rm -rf /").is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PrincipalId(String);

impl PrincipalId {
    /// Maximum allowed length for principal identifiers (reasonable for emails/usernames)
    pub const MAX_LENGTH: usize = 256;

    /// Parse and validate a principal identifier
    ///
    /// # Validation Rules
    ///
    /// - Non-empty (minimum 1 character)
    /// - Maximum 256 characters
    /// - Only alphanumeric characters, `@`, `.`, `-`, `_`
    /// - No path traversal sequences (`../`, `./`)
    /// - No leading or trailing whitespace
    ///
    /// # Security
    ///
    /// This validation prevents:
    /// - SQL injection: `"admin'; DROP TABLE users--"`
    /// - Path traversal: `"../../../etc/passwd"`
    /// - Shell injection: `"user; rm -rf /"`
    /// - LDAP injection: `"*)(uid=*))(|(uid=*"`
    ///
    /// # Examples
    ///
    /// ```rust
    /// use skreaver_core::PrincipalId;
    ///
    /// let principal = PrincipalId::parse("alice@example.com")?;
    /// assert_eq!(principal.as_str(), "alice@example.com");
    /// # Ok::<(), skreaver_core::ValidationError>(())
    /// ```
    pub fn parse(id: impl AsRef<str>) -> Result<Self, crate::validation::ValidationError> {
        let id = id.as_ref();

        // Check for empty
        if id.is_empty() {
            return Err(crate::validation::ValidationError::Empty);
        }

        // Check for whitespace only
        if id.trim().is_empty() {
            return Err(crate::validation::ValidationError::WhitespaceOnly);
        }

        // Check for leading/trailing whitespace
        if id != id.trim() {
            return Err(crate::validation::ValidationError::LeadingTrailingWhitespace);
        }

        // Check length
        if id.len() > Self::MAX_LENGTH {
            return Err(crate::validation::ValidationError::TooLong {
                length: id.len(),
                max: Self::MAX_LENGTH,
            });
        }

        // Check for path traversal
        if id.contains("../") || id.contains("./") || id.contains("..\\") || id.contains(".\\") {
            return Err(crate::validation::ValidationError::PathTraversal);
        }

        // Check for SQL injection patterns
        if id.contains(';') || id.contains("--") || id.contains("/*") || id.contains("*/") {
            return Err(crate::validation::ValidationError::InvalidChar {
                char: ';',
                input: id.to_string(),
            });
        }

        // Check for shell metacharacters
        if let Some(c) = id
            .chars()
            .find(|c| matches!(c, '|' | '&' | '`' | '$' | '>' | '<' | '\\' | '\n' | '\r'))
        {
            return Err(crate::validation::ValidationError::InvalidChar {
                char: c,
                input: id.to_string(),
            });
        }

        // Check valid characters: alphanumeric + @ . - _
        if let Some(c) = id
            .chars()
            .find(|c| !c.is_alphanumeric() && !matches!(c, '@' | '.' | '-' | '_'))
        {
            return Err(crate::validation::ValidationError::InvalidChar {
                char: c,
                input: id.to_string(),
            });
        }

        Ok(Self(id.to_string()))
    }

    /// Get the principal ID as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Create a principal ID without validation (for testing only)
    ///
    /// # Safety
    ///
    /// This bypasses all security validation. Only use for:
    /// - Testing with known-good values
    /// - Migration from legacy systems (validate externally)
    /// - System-generated principals (ensure generation is secure)
    #[doc(hidden)]
    pub fn new_unchecked(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl fmt::Display for PrincipalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for PrincipalId {
    type Err = crate::validation::ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl From<PrincipalId> for String {
    fn from(id: PrincipalId) -> Self {
        id.0
    }
}

impl TryFrom<String> for PrincipalId {
    type Error = crate::validation::ValidationError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_id_valid() {
        assert!(AgentId::parse("agent-1").is_ok());
        assert!(AgentId::parse("my_agent").is_ok());
        assert!(AgentId::parse("agent.123").is_ok());
        assert!(AgentId::parse("a").is_ok());
    }

    #[test]
    fn test_agent_id_invalid() {
        assert!(AgentId::parse("").is_err());
        assert!(AgentId::parse("   ").is_err());
        assert!(AgentId::parse(" agent").is_err());
        assert!(AgentId::parse("agent ").is_err());
        assert!(AgentId::parse("agent/path").is_err());
        assert!(AgentId::parse("../etc").is_err());
        assert!(AgentId::parse("a".repeat(129)).is_err());
    }

    #[test]
    fn test_tool_id_valid() {
        assert!(ToolId::parse("calculator").is_ok());
        assert!(ToolId::parse("http-client").is_ok());
        assert!(ToolId::parse("tool_123").is_ok());
    }

    #[test]
    fn test_session_id_valid() {
        assert!(SessionId::parse("session-abc123").is_ok());
        assert!(SessionId::parse("sess_456").is_ok());
    }

    #[test]
    fn test_request_id_generate() {
        let id1 = RequestId::generate();
        let id2 = RequestId::generate();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_type_safety() {
        let agent = AgentId::parse("agent-1").unwrap();
        let tool = ToolId::parse("tool-1").unwrap();

        // These should be different types (won't compile if uncommented)
        // let _: AgentId = tool;  // Compile error

        assert_eq!(agent.as_str(), "agent-1");
        assert_eq!(tool.as_str(), "tool-1");
    }

    #[test]
    fn test_display_trait() {
        let agent = AgentId::parse("my-agent").unwrap();
        assert_eq!(format!("{}", agent), "my-agent");
    }

    // PrincipalId tests
    #[test]
    fn test_principal_id_valid() {
        // Valid email-like principals
        assert!(PrincipalId::parse("user@example.com").is_ok());
        assert!(PrincipalId::parse("alice.bob@company.org").is_ok());

        // Valid username-like principals
        assert!(PrincipalId::parse("alice").is_ok());
        assert!(PrincipalId::parse("user-123").is_ok());
        assert!(PrincipalId::parse("service_account").is_ok());

        // Valid system/service accounts
        assert!(PrincipalId::parse("system.admin").is_ok());
        assert!(PrincipalId::parse("service-worker-1").is_ok());
        assert!(PrincipalId::parse("bot_assistant").is_ok());
    }

    #[test]
    fn test_principal_id_sql_injection_blocked() {
        // SQL injection attempts should be blocked
        assert!(PrincipalId::parse("admin'; DROP TABLE users--").is_err());
        assert!(PrincipalId::parse("user'; DELETE FROM *--").is_err());
        assert!(PrincipalId::parse("alice;").is_err());
        assert!(PrincipalId::parse("user--comment").is_err());
        assert!(PrincipalId::parse("admin/*comment*/").is_err());
        assert!(PrincipalId::parse("user*/malicious/*").is_err());
    }

    #[test]
    fn test_principal_id_path_traversal_blocked() {
        // Path traversal attempts should be blocked
        assert!(PrincipalId::parse("../etc/passwd").is_err());
        assert!(PrincipalId::parse("../../root").is_err());
        assert!(PrincipalId::parse("./config").is_err());
        assert!(PrincipalId::parse("user/../admin").is_err());
        assert!(PrincipalId::parse("..\\windows\\system32").is_err());
    }

    #[test]
    fn test_principal_id_shell_injection_blocked() {
        // Shell metacharacters should be blocked
        assert!(PrincipalId::parse("user; rm -rf /").is_err());
        assert!(PrincipalId::parse("alice | cat /etc/passwd").is_err());
        assert!(PrincipalId::parse("user && whoami").is_err());
        assert!(PrincipalId::parse("user`whoami`").is_err());
        assert!(PrincipalId::parse("user$var").is_err());
        assert!(PrincipalId::parse("user > file").is_err());
        assert!(PrincipalId::parse("user < file").is_err());
        assert!(PrincipalId::parse("user\\escape").is_err());
    }

    #[test]
    fn test_principal_id_empty_invalid() {
        assert!(PrincipalId::parse("").is_err());
        assert!(PrincipalId::parse("   ").is_err());
        assert!(PrincipalId::parse("\t").is_err());
        assert!(PrincipalId::parse("\n").is_err());
    }

    #[test]
    fn test_principal_id_whitespace_invalid() {
        // Leading/trailing whitespace not allowed
        assert!(PrincipalId::parse(" alice").is_err());
        assert!(PrincipalId::parse("alice ").is_err());
        assert!(PrincipalId::parse(" alice ").is_err());

        // Embedded newlines/carriage returns not allowed
        assert!(PrincipalId::parse("user\nname").is_err());
        assert!(PrincipalId::parse("user\rname").is_err());
    }

    #[test]
    fn test_principal_id_too_long() {
        let long_principal = "a".repeat(257);
        assert!(PrincipalId::parse(long_principal).is_err());

        // 256 should be ok
        let max_principal = "a".repeat(256);
        assert!(PrincipalId::parse(max_principal).is_ok());
    }

    #[test]
    fn test_principal_id_invalid_characters() {
        // Characters outside allowed set
        assert!(PrincipalId::parse("user#tag").is_err());
        assert!(PrincipalId::parse("user%percent").is_err());
        assert!(PrincipalId::parse("user*wildcard").is_err());
        assert!(PrincipalId::parse("user(paren").is_err());
        assert!(PrincipalId::parse("user)paren").is_err());
        assert!(PrincipalId::parse("user=equals").is_err());
        assert!(PrincipalId::parse("user+plus").is_err());
        assert!(PrincipalId::parse("user[bracket").is_err());
        assert!(PrincipalId::parse("user{brace").is_err());
    }

    #[test]
    fn test_principal_id_display() {
        let principal = PrincipalId::parse("alice@example.com").unwrap();
        assert_eq!(format!("{}", principal), "alice@example.com");
        assert_eq!(principal.as_str(), "alice@example.com");
    }

    #[test]
    fn test_principal_id_serde() {
        use serde_json;

        let principal = PrincipalId::parse("user@example.com").unwrap();
        let json = serde_json::to_string(&principal).unwrap();
        assert_eq!(json, "\"user@example.com\"");

        let deserialized: PrincipalId = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, principal);
    }

    #[test]
    fn test_principal_id_ldap_injection_blocked() {
        // LDAP injection attempts
        assert!(PrincipalId::parse("*").is_err());
        assert!(PrincipalId::parse("user*").is_err());
        assert!(PrincipalId::parse("*)(uid=*))(|(uid=*").is_err());
    }

    #[test]
    fn test_from_str_trait() {
        let agent: AgentId = "test-agent".parse().unwrap();
        assert_eq!(agent.as_str(), "test-agent");

        let invalid: Result<AgentId, _> = "".parse();
        assert!(invalid.is_err());
    }

    #[test]
    fn test_serde_roundtrip() {
        let agent = AgentId::parse("serde-test").unwrap();
        let json = serde_json::to_string(&agent).unwrap();
        let deserialized: AgentId = serde_json::from_str(&json).unwrap();
        assert_eq!(agent, deserialized);
    }
}
