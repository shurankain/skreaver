//! GuardedAgent — a `UnifiedAgent` wrapper that enforces guardrail policies.
//!
//! This is the async-agent analogue of `SecureTool<T>` from skreaver-core.

use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use std::sync::Arc;

use skreaver_agent::error::AgentResult;
use skreaver_agent::traits::UnifiedAgent;
use skreaver_agent::types::{AgentInfo, ContentPart, StreamEvent, UnifiedMessage, UnifiedTask};
use skreaver_core::security::SecurityManager;

use crate::anomaly::{AnomalyDetector, AnomalyEvent, AnomalyEventType};
use crate::dynamic::DynamicPolicy;
use crate::error::GuardrailError;
use crate::policy::{GuardrailPolicy, ToolFilter};
use crate::rule::{RuleContext, RuleSet};

/// An agent wrapper that enforces guardrail policies before delegating
/// to the inner agent.
///
/// Supports three modes (additive):
/// - **Policy mode** (Phase A): hardcoded tool filter + size checks
/// - **Rules mode** (Phase B): composable `RuleSet` with pre/post hooks
/// - **Dynamic mode** (Phase C): threat-level-based policy switching + anomaly detection
pub struct GuardedAgent<A: UnifiedAgent> {
    inner: A,
    policy: GuardrailPolicy,
    rules: Option<RuleSet>,
    dynamic_policy: Option<Arc<DynamicPolicy>>,
    anomaly_detector: Option<Arc<dyn AnomalyDetector>>,
    security_manager: Option<Arc<SecurityManager>>,
}

impl<A: UnifiedAgent> GuardedAgent<A> {
    /// Create a new guarded agent with the given policy (no rules).
    pub fn new(agent: A, policy: GuardrailPolicy) -> Self {
        Self {
            inner: agent,
            policy,
            rules: None,
            dynamic_policy: None,
            anomaly_detector: None,
            security_manager: None,
        }
    }

    /// Create a guarded agent with both a policy and a rule set.
    pub fn with_rules(agent: A, policy: GuardrailPolicy, rules: RuleSet) -> Self {
        Self {
            inner: agent,
            policy,
            rules: Some(rules),
            dynamic_policy: None,
            anomaly_detector: None,
            security_manager: None,
        }
    }

    /// Attach an optional `SecurityManager` for deeper input validation.
    pub fn with_security_manager(mut self, manager: Arc<SecurityManager>) -> Self {
        self.security_manager = Some(manager);
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

    /// Access the inner agent.
    pub fn inner(&self) -> &A {
        &self.inner
    }

    /// Get the effective policy (dynamic overrides static if set).
    pub fn effective_policy(&self) -> GuardrailPolicy {
        self.dynamic_policy
            .as_ref()
            .map(|dp| dp.policy())
            .unwrap_or_else(|| self.policy.clone())
    }

    /// Pre-execution check: async (rules with async support) or sync fallback.
    async fn run_pre_checks(&self, message: &UnifiedMessage) -> Result<(), GuardrailError> {
        let policy = self.effective_policy();

        if let Some(rules) = &self.rules {
            let ctx = RuleContext {
                agent_info: self.inner.info(),
                policy: &policy,
            };
            if rules.has_async() {
                rules.check_pre_async(&ctx, message).await?;
            } else {
                rules.check_pre(&ctx, message)?;
            }
        } else {
            self.check_message_size(message, &policy)?;
            self.check_tool_calls(message, &policy)?;
        }
        Ok(())
    }

    /// Post-execution check via rules (no-op if no rules).
    async fn run_post_checks(&self, task: &UnifiedTask) -> Result<(), GuardrailError> {
        let policy = self.effective_policy();

        if let Some(rules) = &self.rules {
            let ctx = RuleContext {
                agent_info: self.inner.info(),
                policy: &policy,
            };
            if rules.has_async() {
                rules.check_post_async(&ctx, task).await?;
            } else {
                rules.check_post(&ctx, task)?;
            }
        }
        Ok(())
    }

    /// Feed a denial event to the anomaly detector and escalate if needed.
    fn on_denial(&self, reason: &str) {
        if let Some(detector) = &self.anomaly_detector {
            let event = AnomalyEvent {
                agent_id: self.inner.info().id.clone(),
                event_type: AnomalyEventType::RuleDenied {
                    rule_name: reason.to_string(),
                },
                timestamp: std::time::Instant::now(),
            };
            let score = detector.analyze(&event);
            if let Some(dp) = &self.dynamic_policy {
                dp.set_level(score.level);
            }
        }
    }

    fn check_message_size(
        &self,
        message: &UnifiedMessage,
        policy: &GuardrailPolicy,
    ) -> Result<(), GuardrailError> {
        if let Some(max_size) = policy.max_message_size {
            let total_size: usize = message.content.iter().map(content_part_size).sum();
            if total_size > max_size {
                return Err(GuardrailError::MessageRejected {
                    reason: format!(
                        "Message size {} bytes exceeds limit of {} bytes",
                        total_size, max_size
                    ),
                });
            }
        }
        Ok(())
    }

    fn check_tool_calls(
        &self,
        message: &UnifiedMessage,
        policy: &GuardrailPolicy,
    ) -> Result<(), GuardrailError> {
        for part in &message.content {
            if let ContentPart::ToolCall { name, .. } = part
                && !policy.tool_filter.is_allowed(name)
            {
                if policy.reject_on_violation {
                    return Err(to_tool_error(&policy.tool_filter, name));
                }
                tracing::warn!(tool = %name, "Tool call blocked by guardrail (non-rejecting mode)");
            }
        }
        Ok(())
    }
}

/// Estimate the byte size of a content part.
fn content_part_size(part: &ContentPart) -> usize {
    match part {
        ContentPart::Text { text } => text.len(),
        ContentPart::Data { data, .. } => data.len(),
        ContentPart::File { uri, .. } => uri.len(),
        ContentPart::ToolCall { arguments, .. } => arguments.to_string().len(),
        ContentPart::ToolResult { result, .. } => result.to_string().len(),
    }
}

fn to_tool_error(filter: &ToolFilter, tool_name: &str) -> GuardrailError {
    match filter {
        ToolFilter::DenyList { .. } | ToolFilter::AllowDeny { .. } => GuardrailError::ToolDenied {
            tool_name: tool_name.to_string(),
        },
        _ => GuardrailError::ToolNotAllowed {
            tool_name: tool_name.to_string(),
        },
    }
}

#[async_trait]
impl<A: UnifiedAgent> UnifiedAgent for GuardedAgent<A> {
    fn info(&self) -> &AgentInfo {
        self.inner.info()
    }

    async fn send_message(&self, message: UnifiedMessage) -> AgentResult<UnifiedTask> {
        if let Err(e) = self.run_pre_checks(&message).await {
            self.on_denial(&e.to_string());
            return Err(e.into_agent_error());
        }
        let task = self.inner.send_message(message).await?;
        if let Err(e) = self.run_post_checks(&task).await {
            self.on_denial(&e.to_string());
            return Err(e.into_agent_error());
        }
        Ok(task)
    }

    async fn send_message_to_task(
        &self,
        task_id: &str,
        message: UnifiedMessage,
    ) -> AgentResult<UnifiedTask> {
        if let Err(e) = self.run_pre_checks(&message).await {
            self.on_denial(&e.to_string());
            return Err(e.into_agent_error());
        }
        let task = self.inner.send_message_to_task(task_id, message).await?;
        if let Err(e) = self.run_post_checks(&task).await {
            self.on_denial(&e.to_string());
            return Err(e.into_agent_error());
        }
        Ok(task)
    }

    async fn send_message_streaming(
        &self,
        message: UnifiedMessage,
    ) -> AgentResult<Pin<Box<dyn Stream<Item = AgentResult<StreamEvent>> + Send>>> {
        if let Err(e) = self.run_pre_checks(&message).await {
            self.on_denial(&e.to_string());
            return Err(e.into_agent_error());
        }
        self.inner.send_message_streaming(message).await
    }

    async fn get_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
        self.inner.get_task(task_id).await
    }

    async fn cancel_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
        self.inner.cancel_task(task_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::ToolFilter;
    use skreaver_agent::error::AgentError;
    use skreaver_agent::types::{
        AgentInfo, ContentPart, Protocol, StreamEvent, TaskStatus, UnifiedMessage, UnifiedTask,
    };

    /// Minimal mock agent for testing guardrails.
    struct MockAgent {
        info: AgentInfo,
    }

    impl MockAgent {
        fn new() -> Self {
            Self {
                info: AgentInfo::new("mock-agent", "Mock Agent").with_protocol(Protocol::A2a),
            }
        }
    }

    #[async_trait]
    impl UnifiedAgent for MockAgent {
        fn info(&self) -> &AgentInfo {
            &self.info
        }

        async fn send_message(&self, _message: UnifiedMessage) -> AgentResult<UnifiedTask> {
            Ok(UnifiedTask::new("mock-agent"))
        }

        async fn send_message_to_task(
            &self,
            _task_id: &str,
            _message: UnifiedMessage,
        ) -> AgentResult<UnifiedTask> {
            Ok(UnifiedTask::new("mock-agent"))
        }

        async fn send_message_streaming(
            &self,
            _message: UnifiedMessage,
        ) -> AgentResult<Pin<Box<dyn Stream<Item = AgentResult<StreamEvent>> + Send>>> {
            Err(AgentError::Internal("Streaming not supported".to_string()))
        }

        async fn get_task(&self, _task_id: &str) -> AgentResult<UnifiedTask> {
            let mut task = UnifiedTask::new("mock-agent");
            task.set_status(TaskStatus::Completed);
            Ok(task)
        }

        async fn cancel_task(&self, _task_id: &str) -> AgentResult<UnifiedTask> {
            let mut task = UnifiedTask::new("mock-agent");
            task.set_status(TaskStatus::Cancelled);
            Ok(task)
        }
    }

    #[tokio::test]
    async fn test_allow_all_passes_through() {
        let agent = GuardedAgent::new(MockAgent::new(), GuardrailPolicy::default());
        let msg = UnifiedMessage::user("Hello");
        let result = agent.send_message(msg).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_allowed_tool_call_passes() {
        let policy = GuardrailPolicy {
            tool_filter: ToolFilter::allow_only(["http_get"]),
            ..Default::default()
        };
        let agent = GuardedAgent::new(MockAgent::new(), policy);

        let msg = UnifiedMessage::user("test").with_part(ContentPart::tool_call(
            "call-1",
            "http_get",
            serde_json::json!({"url": "https://example.com"}),
        ));
        assert!(agent.send_message(msg).await.is_ok());
    }

    #[tokio::test]
    async fn test_denied_tool_call_rejected() {
        let policy = GuardrailPolicy {
            tool_filter: ToolFilter::allow_only(["http_get"]),
            ..Default::default()
        };
        let agent = GuardedAgent::new(MockAgent::new(), policy);

        let msg = UnifiedMessage::user("test").with_part(ContentPart::tool_call(
            "call-1",
            "shell_exec",
            serde_json::json!({"cmd": "rm -rf /"}),
        ));
        let result = agent.send_message(msg).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("shell_exec"),
            "Error should mention tool name: {err}"
        );
    }

    #[tokio::test]
    async fn test_deny_list_blocks_tool() {
        let policy = GuardrailPolicy {
            tool_filter: ToolFilter::deny(["shell_exec"]),
            ..Default::default()
        };
        let agent = GuardedAgent::new(MockAgent::new(), policy);

        let msg = UnifiedMessage::user("test").with_part(ContentPart::tool_call(
            "call-1",
            "shell_exec",
            serde_json::json!({}),
        ));
        assert!(agent.send_message(msg).await.is_err());

        // Other tools pass
        let msg2 = UnifiedMessage::user("test").with_part(ContentPart::tool_call(
            "call-2",
            "http_get",
            serde_json::json!({}),
        ));
        assert!(agent.send_message(msg2).await.is_ok());
    }

    #[tokio::test]
    async fn test_message_size_limit() {
        let policy = GuardrailPolicy {
            max_message_size: Some(10),
            ..Default::default()
        };
        let agent = GuardedAgent::new(MockAgent::new(), policy);

        // Small message passes
        let small = UnifiedMessage::user("hi");
        assert!(agent.send_message(small).await.is_ok());

        // Large message rejected
        let large = UnifiedMessage::user("a]".repeat(100));
        let result = agent.send_message(large).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds limit"));
    }

    #[tokio::test]
    async fn test_get_task_delegates_without_checks() {
        let policy = GuardrailPolicy {
            tool_filter: ToolFilter::allow_only(["nothing"]),
            max_message_size: Some(1),
            ..Default::default()
        };
        let agent = GuardedAgent::new(MockAgent::new(), policy);

        // get_task and cancel_task should always pass regardless of policy
        assert!(agent.get_task("task-1").await.is_ok());
        assert!(agent.cancel_task("task-1").await.is_ok());
    }

    #[tokio::test]
    async fn test_composability() {
        // Layer 1: only allow http_get and file_read
        let policy1 = GuardrailPolicy {
            tool_filter: ToolFilter::allow_only(["http_get", "file_read"]),
            ..Default::default()
        };
        // Layer 2: deny file_read specifically
        let policy2 = GuardrailPolicy {
            tool_filter: ToolFilter::deny(["file_read"]),
            ..Default::default()
        };

        let inner = GuardedAgent::new(MockAgent::new(), policy1);
        let outer = GuardedAgent::new(inner, policy2);

        // http_get: allowed by layer 1, not denied by layer 2 → passes
        let msg1 = UnifiedMessage::user("test").with_part(ContentPart::tool_call(
            "c1",
            "http_get",
            serde_json::json!({}),
        ));
        assert!(outer.send_message(msg1).await.is_ok());

        // file_read: allowed by layer 1, denied by layer 2 → blocked
        let msg2 = UnifiedMessage::user("test").with_part(ContentPart::tool_call(
            "c2",
            "file_read",
            serde_json::json!({}),
        ));
        assert!(outer.send_message(msg2).await.is_err());

        // shell_exec: blocked by layer 1 (never reaches layer 2)
        let msg3 = UnifiedMessage::user("test").with_part(ContentPart::tool_call(
            "c3",
            "shell_exec",
            serde_json::json!({}),
        ));
        assert!(outer.send_message(msg3).await.is_err());
    }

    #[tokio::test]
    async fn test_text_message_no_tool_calls_always_passes() {
        let policy = GuardrailPolicy {
            tool_filter: ToolFilter::allow_only(["nothing"]),
            ..Default::default()
        };
        let agent = GuardedAgent::new(MockAgent::new(), policy);

        // Pure text message with no tool calls should pass tool filter
        let msg = UnifiedMessage::user("Just a question");
        assert!(agent.send_message(msg).await.is_ok());
    }
}
