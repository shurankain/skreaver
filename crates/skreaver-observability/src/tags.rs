//! Cardinal Tags System
//!
//! Defines the core tagging system for Skreaver observability with strict
//! cardinality controls to prevent metrics explosion.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Cardinal tags for Skreaver telemetry as specified in DEVELOPMENT_PLAN.md
/// These tags are used consistently across all metrics and traces
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CardinalTags {
    pub agent_id: Option<AgentId>,
    pub tool_name: Option<ToolName>,
    pub session_id: Option<SessionId>,
    pub error_kind: Option<ErrorKind>,
}

impl CardinalTags {
    /// Create new empty cardinal tags
    pub fn new() -> Self {
        Self {
            agent_id: None,
            tool_name: None,
            session_id: None,
            error_kind: None,
        }
    }

    /// Create tags for agent session
    pub fn for_agent_session(agent_id: AgentId, session_id: SessionId) -> Self {
        Self {
            agent_id: Some(agent_id),
            tool_name: None,
            session_id: Some(session_id),
            error_kind: None,
        }
    }

    /// Create tags for tool execution
    pub fn for_tool_execution(
        agent_id: AgentId,
        session_id: SessionId,
        tool_name: ToolName,
    ) -> Self {
        Self {
            agent_id: Some(agent_id),
            tool_name: Some(tool_name),
            session_id: Some(session_id),
            error_kind: None,
        }
    }

    /// Create tags for error tracking
    pub fn for_error(error_kind: ErrorKind) -> Self {
        Self {
            agent_id: None,
            tool_name: None,
            session_id: None,
            error_kind: Some(error_kind),
        }
    }

    /// Set agent ID
    pub fn with_agent_id(mut self, agent_id: AgentId) -> Self {
        self.agent_id = Some(agent_id);
        self
    }

    /// Set tool name
    pub fn with_tool_name(mut self, tool_name: ToolName) -> Self {
        self.tool_name = Some(tool_name);
        self
    }

    /// Set session ID
    pub fn with_session_id(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Set error kind
    pub fn with_error_kind(mut self, error_kind: ErrorKind) -> Self {
        self.error_kind = Some(error_kind);
        self
    }
}

impl Default for CardinalTags {
    fn default() -> Self {
        Self::new()
    }
}

/// Agent identifier with validation
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(String);

impl AgentId {
    /// Create new agent ID with validation
    pub fn new(id: impl Into<String>) -> Result<Self, TagValidationError> {
        let id = id.into();
        if id.is_empty() || id.len() > 64 {
            return Err(TagValidationError::InvalidAgentId(id));
        }
        // Only allow alphanumeric, hyphens, and underscores
        if !id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(TagValidationError::InvalidAgentId(id));
        }
        Ok(Self(id))
    }

    /// Get the raw string value
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Tool name with cardinality enforcement (≤20 tools per DEVELOPMENT_PLAN.md)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolName(String);

impl ToolName {
    /// Create new tool name with validation
    pub fn new(name: impl Into<String>) -> Result<Self, TagValidationError> {
        let name = name.into();
        if name.is_empty() || name.len() > 32 {
            return Err(TagValidationError::InvalidToolName(name));
        }
        // Only allow alphanumeric and underscores (snake_case)
        if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(TagValidationError::InvalidToolName(name));
        }
        Ok(Self(name))
    }

    /// Get the raw string value
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ToolName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Session identifier for request correlation
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(uuid::Uuid);

impl SessionId {
    /// Generate new session ID
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    /// Create from existing UUID
    pub fn from_uuid(uuid: uuid::Uuid) -> Self {
        Self(uuid)
    }

    /// Get the UUID value
    pub fn as_uuid(&self) -> uuid::Uuid {
        self.0
    }

    /// Get string representation
    pub fn as_str(&self) -> String {
        self.0.to_string()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Error kinds with controlled cardinality (≤10 per DEVELOPMENT_PLAN.md)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorKind {
    /// Parsing errors (malformed input, JSON, etc.)
    Parse,
    /// Timeout errors (tool execution, network, etc.)
    Timeout,
    /// Authentication/authorization errors
    Auth,
    /// Tool execution errors
    Tool,
    /// Memory backend errors
    Memory,
    /// Network/HTTP errors
    Network,
    /// Configuration errors
    Config,
    /// Resource exhaustion errors
    Resource,
    /// Internal system errors
    Internal,
    /// Unknown/unclassified errors
    Unknown,
}

impl ErrorKind {
    /// Get string representation for metrics
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorKind::Parse => "parse",
            ErrorKind::Timeout => "timeout",
            ErrorKind::Auth => "auth",
            ErrorKind::Tool => "tool",
            ErrorKind::Memory => "memory",
            ErrorKind::Network => "network",
            ErrorKind::Config => "config",
            ErrorKind::Resource => "resource",
            ErrorKind::Internal => "internal",
            ErrorKind::Unknown => "unknown",
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Memory operation types for metrics (cardinality: 4)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryOp {
    Read,
    Write,
    Backup,
    Restore,
}

impl MemoryOp {
    /// Get string representation for metrics
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryOp::Read => "read",
            MemoryOp::Write => "write",
            MemoryOp::Backup => "backup",
            MemoryOp::Restore => "restore",
        }
    }
}

impl fmt::Display for MemoryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Tag validation errors
#[derive(thiserror::Error, Debug)]
pub enum TagValidationError {
    #[error("Invalid agent ID: {0} (must be 1-64 chars, alphanumeric/hyphen/underscore only)")]
    InvalidAgentId(String),

    #[error("Invalid tool name: {0} (must be 1-32 chars, alphanumeric/underscore only)")]
    InvalidToolName(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_id_validation() {
        assert!(AgentId::new("valid-agent_123").is_ok());
        assert!(AgentId::new("").is_err());
        assert!(AgentId::new("a".repeat(65)).is_err());
        assert!(AgentId::new("invalid@agent").is_err());
    }

    #[test]
    fn test_tool_name_validation() {
        assert!(ToolName::new("valid_tool").is_ok());
        assert!(ToolName::new("http_client").is_ok());
        assert!(ToolName::new("").is_err());
        assert!(ToolName::new("a".repeat(33)).is_err());
        assert!(ToolName::new("invalid-tool").is_err());
    }

    #[test]
    fn test_cardinal_tags_builder() {
        let agent_id = AgentId::new("test-agent").unwrap();
        let session_id = SessionId::generate();
        let tool_name = ToolName::new("test_tool").unwrap();

        let tags = CardinalTags::new()
            .with_agent_id(agent_id.clone())
            .with_session_id(session_id.clone())
            .with_tool_name(tool_name.clone());

        assert_eq!(tags.agent_id, Some(agent_id));
        assert_eq!(tags.session_id, Some(session_id));
        assert_eq!(tags.tool_name, Some(tool_name));
    }

    #[test]
    fn test_error_kind_strings() {
        assert_eq!(ErrorKind::Parse.as_str(), "parse");
        assert_eq!(ErrorKind::Timeout.as_str(), "timeout");
        assert_eq!(ErrorKind::Tool.as_str(), "tool");
    }

    #[test]
    fn test_memory_op_strings() {
        assert_eq!(MemoryOp::Read.as_str(), "read");
        assert_eq!(MemoryOp::Write.as_str(), "write");
        assert_eq!(MemoryOp::Backup.as_str(), "backup");
        assert_eq!(MemoryOp::Restore.as_str(), "restore");
    }
}
