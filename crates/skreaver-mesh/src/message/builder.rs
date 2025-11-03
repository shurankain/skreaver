//! Fluent message builder API.
//!
//! This module provides `MessageBuilder` for constructing messages with a
//! convenient fluent API. This is a simpler alternative to the typestate-based
//! `TypedMessage` when compile-time routing guarantees are not required.

use super::core::Message;
use super::types::MessagePayload;
use crate::types::AgentId;

/// Builder for creating messages with a fluent API
pub struct MessageBuilder {
    message: Message,
}

impl MessageBuilder {
    /// Create a new anonymous message builder
    pub fn new(payload: impl Into<MessagePayload>) -> Self {
        Self {
            message: Message::new(payload),
        }
    }

    /// Create a unicast message builder
    pub fn unicast(
        from: impl Into<AgentId>,
        to: impl Into<AgentId>,
        payload: impl Into<MessagePayload>,
    ) -> Self {
        Self {
            message: Message::unicast(from, to, payload),
        }
    }

    /// Create a broadcast message builder
    pub fn broadcast(from: impl Into<AgentId>, payload: impl Into<MessagePayload>) -> Self {
        Self {
            message: Message::broadcast(from, payload),
        }
    }

    /// Create a system message builder
    pub fn system(to: impl Into<AgentId>, payload: impl Into<MessagePayload>) -> Self {
        Self {
            message: Message::system(to, payload),
        }
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.message = self.message.with_metadata(key, value);
        self
    }

    /// Set correlation ID
    pub fn with_correlation_id(mut self, correlation_id: impl Into<String>) -> Self {
        self.message = self.message.with_correlation_id(correlation_id);
        self
    }

    /// Build the message
    pub fn build(self) -> Message {
        self.message
    }
}
