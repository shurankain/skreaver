//! Core type definitions for mesh communication

use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for an agent in the mesh
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(String);

impl AgentId {
    /// Create a new agent ID
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Get the agent ID as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for AgentId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for AgentId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Topic identifier for pub/sub messaging
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Topic(String);

impl Topic {
    /// Create a new topic
    pub fn new(topic: impl Into<String>) -> Self {
        Self(topic.into())
    }

    /// Get the topic as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for Topic {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for Topic {
    fn from(s: &str) -> Self {
        Self(s.to_string())
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
        let id = AgentId::new("agent-1");
        assert_eq!(id.as_str(), "agent-1");
        assert_eq!(id.to_string(), "agent-1");
    }

    #[test]
    fn test_agent_id_from_string() {
        let id: AgentId = "agent-2".into();
        assert_eq!(id.as_str(), "agent-2");
    }

    #[test]
    fn test_topic_creation() {
        let topic = Topic::new("notifications");
        assert_eq!(topic.as_str(), "notifications");
        assert_eq!(topic.to_string(), "notifications");
    }

    #[test]
    fn test_topic_from_str() {
        let topic: Topic = "events".into();
        assert_eq!(topic.as_str(), "events");
    }
}
