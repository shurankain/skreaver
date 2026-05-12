//! Human-in-the-loop approval hooks.
//!
//! Provides an `ApprovalHook` trait that users implement to define
//! their approval mechanism (Slack, CLI prompt, API endpoint, etc.).
//! `ApprovalRule` wraps a hook as an `AsyncRule` in the guardrail pipeline.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use skreaver_agent::types::{ContentPart, UnifiedMessage};

use crate::rule::{AsyncRule, RuleContext, RuleResult};

/// Request sent to an approval hook.
#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    /// Agent requesting approval.
    pub agent_id: String,
    /// Preview of the message content.
    pub message_preview: String,
    /// Why approval is required.
    pub reason: String,
    /// Tool calls that triggered the approval.
    pub tool_calls: Vec<String>,
}

/// Decision returned by an approval hook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    /// Approved — proceed with execution.
    Approved,
    /// Denied — block execution.
    Denied { reason: String },
    /// Hook did not respond in time.
    Timeout,
}

/// Trait for implementing approval mechanisms.
///
/// Users provide their own implementation (Slack webhook, CLI prompt,
/// REST API, etc.). The guardrail pipeline calls `request_approval`
/// and blocks until a decision is made or the timeout expires.
#[async_trait]
pub trait ApprovalHook: Send + Sync {
    async fn request_approval(&self, request: &ApprovalRequest) -> ApprovalDecision;
}

/// When to require approval.
#[derive(Debug, Clone)]
pub enum ApprovalTrigger {
    /// Require approval for every message.
    Always,
    /// Require approval only when specific tools are called.
    OnToolCall(HashSet<String>),
}

/// Async rule that gates execution on human approval.
pub struct ApprovalRule {
    hook: Arc<dyn ApprovalHook>,
    trigger: ApprovalTrigger,
    timeout: Duration,
}

impl ApprovalRule {
    /// Create an approval rule that triggers on every message.
    pub fn always(hook: Arc<dyn ApprovalHook>, timeout: Duration) -> Self {
        Self {
            hook,
            trigger: ApprovalTrigger::Always,
            timeout,
        }
    }

    /// Create an approval rule that triggers only for specific tool calls.
    pub fn on_tools(
        hook: Arc<dyn ApprovalHook>,
        tools: impl IntoIterator<Item = impl Into<String>>,
        timeout: Duration,
    ) -> Self {
        Self {
            hook,
            trigger: ApprovalTrigger::OnToolCall(tools.into_iter().map(Into::into).collect()),
            timeout,
        }
    }

    fn should_trigger(&self, message: &UnifiedMessage) -> Option<Vec<String>> {
        match &self.trigger {
            ApprovalTrigger::Always => Some(vec![]),
            ApprovalTrigger::OnToolCall(tools) => {
                let matched: Vec<String> = message
                    .content
                    .iter()
                    .filter_map(|part| {
                        if let ContentPart::ToolCall { name, .. } = part
                            && tools.contains(name)
                        {
                            return Some(name.clone());
                        }
                        None
                    })
                    .collect();
                if matched.is_empty() {
                    None
                } else {
                    Some(matched)
                }
            }
        }
    }
}

#[async_trait]
impl AsyncRule for ApprovalRule {
    fn name(&self) -> &str {
        "approval"
    }

    async fn check_pre(&self, ctx: &RuleContext<'_>, message: &UnifiedMessage) -> RuleResult {
        let tool_calls = match self.should_trigger(message) {
            Some(calls) => calls,
            None => return RuleResult::Allow,
        };

        let preview = message
            .content
            .iter()
            .find_map(|p| {
                if let ContentPart::Text { text } = p {
                    Some(text.chars().take(200).collect::<String>())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        let request = ApprovalRequest {
            agent_id: ctx.agent_info.id.clone(),
            message_preview: preview,
            reason: if tool_calls.is_empty() {
                "Approval required for all messages".to_string()
            } else {
                format!("Tool calls require approval: {}", tool_calls.join(", "))
            },
            tool_calls,
        };

        let decision = tokio::time::timeout(self.timeout, self.hook.request_approval(&request))
            .await
            .unwrap_or(ApprovalDecision::Timeout);

        match decision {
            ApprovalDecision::Approved => RuleResult::Allow,
            ApprovalDecision::Denied { reason } => {
                RuleResult::Deny(format!("Approval denied: {}", reason))
            }
            ApprovalDecision::Timeout => RuleResult::Deny("Approval timed out".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skreaver_agent::types::{AgentInfo, ContentPart, Protocol, UnifiedMessage};

    struct AutoApprove;
    #[async_trait]
    impl ApprovalHook for AutoApprove {
        async fn request_approval(&self, _req: &ApprovalRequest) -> ApprovalDecision {
            ApprovalDecision::Approved
        }
    }

    struct AutoDeny;
    #[async_trait]
    impl ApprovalHook for AutoDeny {
        async fn request_approval(&self, _req: &ApprovalRequest) -> ApprovalDecision {
            ApprovalDecision::Denied {
                reason: "nope".to_string(),
            }
        }
    }

    fn test_ctx() -> (AgentInfo, crate::policy::GuardrailPolicy) {
        (
            AgentInfo::new("test", "Test").with_protocol(Protocol::A2a),
            crate::policy::GuardrailPolicy::default(),
        )
    }

    #[tokio::test]
    async fn test_always_approve() {
        let rule = ApprovalRule::always(Arc::new(AutoApprove), Duration::from_secs(5));
        let (info, policy) = test_ctx();
        let ctx = RuleContext {
            agent_info: &info,
            policy: &policy,
        };
        let msg = UnifiedMessage::user("hello");
        assert_eq!(rule.check_pre(&ctx, &msg).await, RuleResult::Allow);
    }

    #[tokio::test]
    async fn test_always_deny() {
        let rule = ApprovalRule::always(Arc::new(AutoDeny), Duration::from_secs(5));
        let (info, policy) = test_ctx();
        let ctx = RuleContext {
            agent_info: &info,
            policy: &policy,
        };
        let msg = UnifiedMessage::user("hello");
        match rule.check_pre(&ctx, &msg).await {
            RuleResult::Deny(reason) => assert!(reason.contains("nope")),
            other => panic!("Expected Deny, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_tool_trigger_skips_non_matching() {
        let rule = ApprovalRule::on_tools(
            Arc::new(AutoDeny),
            ["dangerous_tool"],
            Duration::from_secs(5),
        );
        let (info, policy) = test_ctx();
        let ctx = RuleContext {
            agent_info: &info,
            policy: &policy,
        };
        // No tool calls — should skip approval
        let msg = UnifiedMessage::user("safe message");
        assert_eq!(rule.check_pre(&ctx, &msg).await, RuleResult::Allow);
    }

    #[tokio::test]
    async fn test_tool_trigger_matches() {
        let rule = ApprovalRule::on_tools(
            Arc::new(AutoDeny),
            ["dangerous_tool"],
            Duration::from_secs(5),
        );
        let (info, policy) = test_ctx();
        let ctx = RuleContext {
            agent_info: &info,
            policy: &policy,
        };
        let msg = UnifiedMessage::user("run it").with_part(ContentPart::tool_call(
            "c1",
            "dangerous_tool",
            serde_json::json!({}),
        ));
        match rule.check_pre(&ctx, &msg).await {
            RuleResult::Deny(reason) => assert!(reason.contains("nope")),
            other => panic!("Expected Deny, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_timeout() {
        struct SlowHook;
        #[async_trait]
        impl ApprovalHook for SlowHook {
            async fn request_approval(&self, _req: &ApprovalRequest) -> ApprovalDecision {
                tokio::time::sleep(Duration::from_secs(10)).await;
                ApprovalDecision::Approved
            }
        }

        let rule = ApprovalRule::always(Arc::new(SlowHook), Duration::from_millis(50));
        let (info, policy) = test_ctx();
        let ctx = RuleContext {
            agent_info: &info,
            policy: &policy,
        };
        let msg = UnifiedMessage::user("hello");
        match rule.check_pre(&ctx, &msg).await {
            RuleResult::Deny(reason) => assert!(reason.contains("timed out")),
            other => panic!("Expected Deny (timeout), got {:?}", other),
        }
    }
}
