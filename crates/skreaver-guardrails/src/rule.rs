//! Rule trait and RuleSet for composable guardrail checks.

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

/// An ordered collection of rules evaluated sequentially.
///
/// Pre-checks short-circuit on the first `Deny`. Warnings accumulate
/// and are logged but do not block execution.
pub struct RuleSet {
    rules: Vec<Box<dyn Rule>>,
}

impl RuleSet {
    /// Create an empty rule set.
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Add a rule (builder pattern).
    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, rule: impl Rule + 'static) -> Self {
        self.rules.push(Box::new(rule));
        self
    }

    /// Number of rules in the set.
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
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
