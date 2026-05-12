//! Fluent builder for constructing `GuardedAgent` instances.

use std::sync::Arc;

use skreaver_agent::traits::UnifiedAgent;
use skreaver_core::security::SecurityManager;

use crate::agent::GuardedAgent;
use crate::anomaly::AnomalyDetector;
use crate::dynamic::DynamicPolicy;
use crate::policy::{GuardrailPolicy, ToolFilter};
use crate::preset::Preset;
use crate::rule::{AsyncRule, Rule, RuleSet};

/// Builder for constructing a `GuardedAgent` with a fluent API.
pub struct GuardedAgentBuilder<A: UnifiedAgent> {
    agent: A,
    tool_filter: ToolFilter,
    max_message_size: Option<usize>,
    reject_on_violation: bool,
    rules: Option<RuleSet>,
    dynamic_policy: Option<Arc<DynamicPolicy>>,
    anomaly_detector: Option<Arc<dyn AnomalyDetector>>,
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
            rules: None,
            dynamic_policy: None,
            anomaly_detector: None,
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

    /// Add a single rule to the rule set.
    pub fn with_rule(mut self, rule: impl Rule + 'static) -> Self {
        let rules = self.rules.take().unwrap_or_default();
        self.rules = Some(rules.add(rule));
        self
    }

    /// Apply a preset, which sets policy defaults and adds preset rules.
    ///
    /// For `Preset::Strict`, pass an allowlist via `.allow_tools()` first
    /// or the strict preset will block all tools.
    pub fn with_preset(mut self, preset: Preset) -> Self {
        let policy = preset.policy();
        self.max_message_size = policy.max_message_size;
        self.reject_on_violation = policy.reject_on_violation;

        // Collect allowed tools from the current filter for strict preset
        let allowed = match &self.tool_filter {
            ToolFilter::AllowList { tools } => Some(tools.iter().cloned().collect()),
            _ => None,
        };
        self.rules = Some(preset.rules(allowed));
        self
    }

    /// Add an async rule (e.g., `ApprovalRule`).
    pub fn with_async_rule(mut self, rule: impl AsyncRule + 'static) -> Self {
        let rules = self.rules.take().unwrap_or_default();
        self.rules = Some(rules.add_async(rule));
        self
    }

    /// Attach a dynamic policy for threat-level-based switching.
    pub fn with_dynamic_policy(mut self, dp: Arc<DynamicPolicy>) -> Self {
        self.dynamic_policy = Some(dp);
        self
    }

    /// Attach an anomaly detector for automatic threat escalation.
    pub fn with_anomaly_detector(mut self, detector: Arc<dyn AnomalyDetector>) -> Self {
        self.anomaly_detector = Some(detector);
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
        let mut agent = match self.rules {
            Some(rules) => GuardedAgent::with_rules(self.agent, policy, rules),
            None => GuardedAgent::new(self.agent, policy),
        };
        if let Some(dp) = self.dynamic_policy {
            agent = agent.with_dynamic_policy(dp);
        }
        if let Some(ad) = self.anomaly_detector {
            agent = agent.with_anomaly_detector(ad);
        }
        if let Some(sm) = self.security_manager {
            agent = agent.with_security_manager(sm);
        }
        agent
    }
}
