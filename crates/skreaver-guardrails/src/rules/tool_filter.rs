//! Tool filter rule — checks tool calls against the policy's ToolFilter.

use skreaver_agent::types::{ContentPart, UnifiedMessage};

use crate::policy::ToolFilter;
use crate::rule::{Rule, RuleContext, RuleResult};

/// Checks that all tool calls in a message are permitted by the tool filter.
pub struct ToolFilterRule {
    filter: ToolFilter,
}

impl ToolFilterRule {
    pub fn new(filter: ToolFilter) -> Self {
        Self { filter }
    }
}

impl Rule for ToolFilterRule {
    fn name(&self) -> &str {
        "tool_filter"
    }

    fn check_pre(&self, _ctx: &RuleContext<'_>, message: &UnifiedMessage) -> RuleResult {
        for part in &message.content {
            if let ContentPart::ToolCall { name, .. } = part
                && !self.filter.is_allowed(name)
            {
                return RuleResult::Deny(format!("Tool '{}' not permitted", name));
            }
        }
        RuleResult::Allow
    }
}
