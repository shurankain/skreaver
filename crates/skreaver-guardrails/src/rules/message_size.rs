//! Message size rule — limits total content size across all parts.

use skreaver_agent::types::{ContentPart, UnifiedMessage};

use crate::rule::{Rule, RuleContext, RuleResult};

/// Rejects messages whose total content exceeds a byte limit.
pub struct MessageSizeRule {
    max_bytes: usize,
}

impl MessageSizeRule {
    pub fn new(max_bytes: usize) -> Self {
        Self { max_bytes }
    }
}

impl Rule for MessageSizeRule {
    fn name(&self) -> &str {
        "message_size"
    }

    fn check_pre(&self, _ctx: &RuleContext<'_>, message: &UnifiedMessage) -> RuleResult {
        let total: usize = message.content.iter().map(content_part_size).sum();
        if total > self.max_bytes {
            RuleResult::Deny(format!(
                "Message size {} bytes exceeds limit of {} bytes",
                total, self.max_bytes
            ))
        } else {
            RuleResult::Allow
        }
    }
}

fn content_part_size(part: &ContentPart) -> usize {
    match part {
        ContentPart::Text { text } => text.len(),
        ContentPart::Data { data, .. } => data.len(),
        ContentPart::File { uri, .. } => uri.len(),
        ContentPart::ToolCall { arguments, .. } => arguments.to_string().len(),
        ContentPart::ToolResult { result, .. } => result.to_string().len(),
    }
}
