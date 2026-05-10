//! Input validation rule — scans text for secrets and injection patterns.
//!
//! Wraps `skreaver_core::security::InputValidator`.

use skreaver_agent::types::{ContentPart, UnifiedMessage};
use skreaver_core::security::InputValidator;
use skreaver_core::security::policy::SecurityPolicy;

use crate::rule::{Rule, RuleContext, RuleResult};

/// Scans message text for secrets (API keys, tokens) and injection
/// patterns (SQL, command, XSS) using skreaver-core's InputValidator.
pub struct InputValidationRule {
    validator: InputValidator,
}

impl InputValidationRule {
    pub fn new() -> Self {
        Self {
            validator: InputValidator::new(&SecurityPolicy::default()),
        }
    }
}

impl Default for InputValidationRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for InputValidationRule {
    fn name(&self) -> &str {
        "input_validation"
    }

    fn check_pre(&self, _ctx: &RuleContext<'_>, message: &UnifiedMessage) -> RuleResult {
        for part in &message.content {
            if let ContentPart::Text { text } = part
                && let Err(e) = self.validator.validate(text)
            {
                return RuleResult::Deny(format!("Input validation failed: {}", e));
            }
        }
        RuleResult::Allow
    }
}
