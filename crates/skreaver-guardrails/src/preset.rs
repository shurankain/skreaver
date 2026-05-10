//! Policy presets for common guardrail configurations.

use crate::policy::{GuardrailPolicy, ToolFilter};
use crate::rule::RuleSet;
use crate::rules::{
    InputValidationRule, MessageSizeRule, OutputScanRule, RateLimitRule, ToolFilterRule,
};

/// Pre-configured guardrail presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    /// No restrictions. Tool filter allows all, no size limit, no scanning.
    Permissive,
    /// Balanced defaults. All tools allowed, 1 MB message limit, input validation.
    Standard,
    /// Maximum safety. Must specify allowlist, 100 KB limit, input+output scanning,
    /// rate limit of 60 requests per minute.
    Strict,
}

impl Preset {
    /// Build a `RuleSet` for this preset.
    ///
    /// For `Strict`, pass a tool allowlist. For other presets it's ignored.
    pub fn rules(&self, allowed_tools: Option<Vec<String>>) -> RuleSet {
        match self {
            Preset::Permissive => RuleSet::new(),
            Preset::Standard => RuleSet::new()
                .add(MessageSizeRule::new(1024 * 1024))
                .add(InputValidationRule::new()),
            Preset::Strict => {
                let filter = match allowed_tools {
                    Some(tools) => ToolFilter::allow_only(tools),
                    None => ToolFilter::allow_only(Vec::<String>::new()),
                };
                RuleSet::new()
                    .add(ToolFilterRule::new(filter))
                    .add(MessageSizeRule::new(100 * 1024))
                    .add(InputValidationRule::new())
                    .add(RateLimitRule::per_minute(60))
                    .add(OutputScanRule::new())
            }
        }
    }

    /// Build a default `GuardrailPolicy` for this preset.
    pub fn policy(&self) -> GuardrailPolicy {
        match self {
            Preset::Permissive => GuardrailPolicy {
                tool_filter: ToolFilter::AllowAll,
                max_message_size: None,
                reject_on_violation: false,
            },
            Preset::Standard => GuardrailPolicy {
                tool_filter: ToolFilter::AllowAll,
                max_message_size: Some(1024 * 1024),
                reject_on_violation: true,
            },
            Preset::Strict => GuardrailPolicy {
                tool_filter: ToolFilter::AllowAll, // actual filter is in the RuleSet
                max_message_size: Some(100 * 1024),
                reject_on_violation: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permissive_has_no_rules() {
        let rs = Preset::Permissive.rules(None);
        assert!(rs.is_empty());
    }

    #[test]
    fn test_standard_has_two_rules() {
        let rs = Preset::Standard.rules(None);
        assert_eq!(rs.len(), 2);
    }

    #[test]
    fn test_strict_has_five_rules() {
        let rs = Preset::Strict.rules(Some(vec!["http_get".into()]));
        assert_eq!(rs.len(), 5);
    }

    #[test]
    fn test_preset_policies() {
        assert!(!Preset::Permissive.policy().reject_on_violation);
        assert!(Preset::Standard.policy().reject_on_violation);
        assert_eq!(Preset::Strict.policy().max_message_size, Some(100 * 1024));
    }
}
