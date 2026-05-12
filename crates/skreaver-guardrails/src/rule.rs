//! Rule trait and RuleSet for composable guardrail checks.

use async_trait::async_trait;
use skreaver_agent::types::{AgentInfo, UnifiedMessage, UnifiedTask};

use crate::error::GuardrailError;
use crate::policy::GuardrailPolicy;

/// Outcome of a rule evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleResult {
    /// Message is allowed to proceed.
    Allow,
    /// Message is denied — stop processing.
    Deny(String),
    /// Warning logged but message proceeds.
    Warn(String),
}

/// Context available to rules during evaluation.
pub struct RuleContext<'a> {
    pub agent_info: &'a AgentInfo,
    pub policy: &'a GuardrailPolicy,
}

/// A composable guardrail check.
///
/// Rules inspect messages before execution (pre) and tasks after
/// execution (post). Both methods default to `Allow` so rules only
/// need to implement the hooks they care about.
pub trait Rule: Send + Sync {
    /// Human-readable rule name for logging.
    fn name(&self) -> &str;

    /// Check a message before it reaches the inner agent.
    fn check_pre(&self, _ctx: &RuleContext<'_>, _message: &UnifiedMessage) -> RuleResult {
        RuleResult::Allow
    }

    /// Check a task after the inner agent returns it.
    fn check_post(&self, _ctx: &RuleContext<'_>, _task: &UnifiedTask) -> RuleResult {
        RuleResult::Allow
    }
}

/// An async guardrail check for operations that require I/O
/// (e.g., approval hooks, external validators).
///
/// Separate from `Rule` so sync rules don't pay async overhead.
#[async_trait]
pub trait AsyncRule: Send + Sync {
    fn name(&self) -> &str;

    async fn check_pre(&self, _ctx: &RuleContext<'_>, _message: &UnifiedMessage) -> RuleResult {
        RuleResult::Allow
    }

    async fn check_post(&self, _ctx: &RuleContext<'_>, _task: &UnifiedTask) -> RuleResult {
        RuleResult::Allow
    }
}

/// An ordered collection of rules evaluated sequentially.
///
/// Sync rules run first, then async rules. Both short-circuit
/// on the first `Deny`. Warnings accumulate.
pub struct RuleSet {
    rules: Vec<Box<dyn Rule>>,
    async_rules: Vec<Box<dyn AsyncRule>>,
}

impl RuleSet {
    /// Create an empty rule set.
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            async_rules: Vec::new(),
        }
    }

    /// Add a sync rule (builder pattern).
    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, rule: impl Rule + 'static) -> Self {
        self.rules.push(Box::new(rule));
        self
    }

    /// Add an async rule (builder pattern).
    pub fn add_async(mut self, rule: impl AsyncRule + 'static) -> Self {
        self.async_rules.push(Box::new(rule));
        self
    }

    /// Total number of rules (sync + async).
    pub fn len(&self) -> usize {
        self.rules.len() + self.async_rules.len()
    }

    /// Whether the set has no rules.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty() && self.async_rules.is_empty()
    }

    /// Whether the set contains async rules.
    pub fn has_async(&self) -> bool {
        !self.async_rules.is_empty()
    }

    /// Run all pre-execution rules. Returns accumulated warnings on success.
    ///
    /// Short-circuits on the first `Deny`, returning a `GuardrailError`.
    pub fn check_pre(
        &self,
        ctx: &RuleContext<'_>,
        message: &UnifiedMessage,
    ) -> Result<Vec<String>, GuardrailError> {
        let mut warnings = Vec::new();

        for rule in &self.rules {
            match rule.check_pre(ctx, message) {
                RuleResult::Allow => {}
                RuleResult::Warn(msg) => {
                    tracing::warn!(rule = rule.name(), warning = %msg, "Guardrail warning");
                    warnings.push(msg);
                }
                RuleResult::Deny(reason) => {
                    tracing::warn!(rule = rule.name(), reason = %reason, "Guardrail denied");
                    return Err(GuardrailError::MessageRejected {
                        reason: format!("[{}] {}", rule.name(), reason),
                    });
                }
            }
        }

        Ok(warnings)
    }

    /// Run all post-execution rules. Returns accumulated warnings on success.
    pub fn check_post(
        &self,
        ctx: &RuleContext<'_>,
        task: &UnifiedTask,
    ) -> Result<Vec<String>, GuardrailError> {
        let mut warnings = Vec::new();

        for rule in &self.rules {
            match rule.check_post(ctx, task) {
                RuleResult::Allow => {}
                RuleResult::Warn(msg) => {
                    tracing::warn!(rule = rule.name(), warning = %msg, "Post-execution warning");
                    warnings.push(msg);
                }
                RuleResult::Deny(reason) => {
                    tracing::warn!(rule = rule.name(), reason = %reason, "Post-execution denied");
                    return Err(GuardrailError::MessageRejected {
                        reason: format!("[{}] {}", rule.name(), reason),
                    });
                }
            }
        }

        Ok(warnings)
    }

    /// Run all rules (sync then async) for pre-execution. Returns warnings on success.
    pub async fn check_pre_async(
        &self,
        ctx: &RuleContext<'_>,
        message: &UnifiedMessage,
    ) -> Result<Vec<String>, GuardrailError> {
        // Sync rules first
        let mut warnings = self.check_pre(ctx, message)?;

        // Then async rules
        for rule in &self.async_rules {
            match rule.check_pre(ctx, message).await {
                RuleResult::Allow => {}
                RuleResult::Warn(msg) => {
                    tracing::warn!(rule = rule.name(), warning = %msg, "Async guardrail warning");
                    warnings.push(msg);
                }
                RuleResult::Deny(reason) => {
                    tracing::warn!(rule = rule.name(), reason = %reason, "Async guardrail denied");
                    return Err(GuardrailError::MessageRejected {
                        reason: format!("[{}] {}", rule.name(), reason),
                    });
                }
            }
        }

        Ok(warnings)
    }

    /// Run all rules (sync then async) for post-execution.
    pub async fn check_post_async(
        &self,
        ctx: &RuleContext<'_>,
        task: &UnifiedTask,
    ) -> Result<Vec<String>, GuardrailError> {
        let mut warnings = self.check_post(ctx, task)?;

        for rule in &self.async_rules {
            match rule.check_post(ctx, task).await {
                RuleResult::Allow => {}
                RuleResult::Warn(msg) => {
                    tracing::warn!(rule = rule.name(), warning = %msg, "Async post-execution warning");
                    warnings.push(msg);
                }
                RuleResult::Deny(reason) => {
                    tracing::warn!(rule = rule.name(), reason = %reason, "Async post-execution denied");
                    return Err(GuardrailError::MessageRejected {
                        reason: format!("[{}] {}", rule.name(), reason),
                    });
                }
            }
        }

        Ok(warnings)
    }
}

impl Default for RuleSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skreaver_agent::types::{AgentInfo, Protocol, UnifiedMessage};

    struct AlwaysAllow;
    impl Rule for AlwaysAllow {
        fn name(&self) -> &str {
            "always_allow"
        }
    }

    struct AlwaysWarn(String);
    impl Rule for AlwaysWarn {
        fn name(&self) -> &str {
            "always_warn"
        }
        fn check_pre(&self, _ctx: &RuleContext<'_>, _msg: &UnifiedMessage) -> RuleResult {
            RuleResult::Warn(self.0.clone())
        }
    }

    struct AlwaysDeny(String);
    impl Rule for AlwaysDeny {
        fn name(&self) -> &str {
            "always_deny"
        }
        fn check_pre(&self, _ctx: &RuleContext<'_>, _msg: &UnifiedMessage) -> RuleResult {
            RuleResult::Deny(self.0.clone())
        }
    }

    fn test_ctx() -> (AgentInfo, GuardrailPolicy) {
        (
            AgentInfo::new("test", "Test Agent").with_protocol(Protocol::A2a),
            GuardrailPolicy::default(),
        )
    }

    #[test]
    fn test_empty_ruleset_allows() {
        let (info, policy) = test_ctx();
        let ctx = RuleContext {
            agent_info: &info,
            policy: &policy,
        };
        let rs = RuleSet::new();
        let msg = UnifiedMessage::user("hello");
        assert!(rs.check_pre(&ctx, &msg).is_ok());
    }

    #[test]
    fn test_warnings_accumulate() {
        let (info, policy) = test_ctx();
        let ctx = RuleContext {
            agent_info: &info,
            policy: &policy,
        };
        let rs = RuleSet::new()
            .add(AlwaysWarn("w1".into()))
            .add(AlwaysWarn("w2".into()));
        let msg = UnifiedMessage::user("hello");
        let warnings = rs.check_pre(&ctx, &msg).unwrap();
        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn test_deny_short_circuits() {
        let (info, policy) = test_ctx();
        let ctx = RuleContext {
            agent_info: &info,
            policy: &policy,
        };
        let rs = RuleSet::new()
            .add(AlwaysWarn("w1".into()))
            .add(AlwaysDeny("blocked".into()))
            .add(AlwaysWarn("w2".into())); // should not run
        let msg = UnifiedMessage::user("hello");
        let err = rs.check_pre(&ctx, &msg).unwrap_err();
        assert!(err.to_string().contains("blocked"));
    }

    #[test]
    fn test_allow_passes() {
        let (info, policy) = test_ctx();
        let ctx = RuleContext {
            agent_info: &info,
            policy: &policy,
        };
        let rs = RuleSet::new().add(AlwaysAllow).add(AlwaysAllow);
        let msg = UnifiedMessage::user("hello");
        let warnings = rs.check_pre(&ctx, &msg).unwrap();
        assert!(warnings.is_empty());
    }
}
