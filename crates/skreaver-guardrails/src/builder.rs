//! Fluent builder for constructing `GuardedAgent` instances.

use std::sync::Arc;

use skreaver_agent::traits::UnifiedAgent;
use skreaver_core::security::SecurityManager;

use crate::agent::GuardedAgent;
use crate::policy::{GuardrailPolicy, ToolFilter};

/// Builder for constructing a `GuardedAgent` with a fluent API.
pub struct GuardedAgentBuilder<A: UnifiedAgent> {
    agent: A,
    tool_filter: ToolFilter,
    max_message_size: Option<usize>,
    reject_on_violation: bool,
    security_manager: Option<Arc<SecurityManager>>,
}

impl<A: UnifiedAgent> GuardedAgentBuilder<A> {
    /// Start building a guarded agent wrapping the given inner agent.
    pub fn new(agent: A) -> Self {
        Self {
            agent,
            tool_filter: ToolFilter::default(),
            max_message_size: None,
            reject_on_violation: true,
            security_manager: None,
        }
    }

    /// Set an allowlist of permitted tools (replaces current filter).
    pub fn allow_tools(mut self, tools: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tool_filter = ToolFilter::allow_only(tools);
        self
    }

    /// Set a denylist of blocked tools (replaces current filter).
    pub fn deny_tools(mut self, tools: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tool_filter = ToolFilter::deny(tools);
        self
    }

    /// Set the maximum allowed message size in bytes.
    pub fn max_message_size(mut self, bytes: usize) -> Self {
        self.max_message_size = Some(bytes);
        self
    }

    /// Set whether violations cause rejection (true) or just warnings (false).
    pub fn reject_on_violation(mut self, reject: bool) -> Self {
        self.reject_on_violation = reject;
        self
    }

    /// Attach a `SecurityManager` for deeper input validation.
    pub fn security_manager(mut self, manager: Arc<SecurityManager>) -> Self {
        self.security_manager = Some(manager);
        self
    }

    /// Build the `GuardedAgent`.
    pub fn build(self) -> GuardedAgent<A> {
        let policy = GuardrailPolicy {
            tool_filter: self.tool_filter,
            max_message_size: self.max_message_size,
            reject_on_violation: self.reject_on_violation,
        };
        let mut agent = GuardedAgent::new(self.agent, policy);
        if let Some(sm) = self.security_manager {
            agent = agent.with_security_manager(sm);
        }
        agent
    }
}
