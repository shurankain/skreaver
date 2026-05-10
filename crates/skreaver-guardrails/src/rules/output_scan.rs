//! Output scan rule — checks task results for leaked secrets.
//!
//! Wraps `skreaver_core::sanitization::SecretRedactor`.

use skreaver_agent::types::{ContentPart, UnifiedTask};
use skreaver_core::sanitization::SecretRedactor;

use crate::rule::{Rule, RuleContext, RuleResult};

/// Post-execution rule that scans agent output for leaked secrets.
///
/// If secrets are detected in the task's messages, the rule denies
/// the result to prevent sensitive data from reaching the caller.
pub struct OutputScanRule;

impl OutputScanRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OutputScanRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for OutputScanRule {
    fn name(&self) -> &str {
        "output_scan"
    }

    fn check_post(&self, _ctx: &RuleContext<'_>, task: &UnifiedTask) -> RuleResult {
        for message in &task.messages {
            for part in &message.content {
                if let ContentPart::Text { text } = part {
                    let redacted = SecretRedactor::redact_secrets(text);
                    if redacted != *text {
                        return RuleResult::Deny("Output contains potential secrets".to_string());
                    }
                }
            }
        }
        RuleResult::Allow
    }
}
