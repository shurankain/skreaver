//! Guardrail configuration with per-agent policy overrides.

use crate::policy::GuardrailPolicy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for guardrails, supporting per-agent overrides.
///
/// Follows the same hierarchical override pattern as
/// `skreaver_core::security::SecurityConfig::tool_policy()`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GuardrailConfig {
    /// Default policy applied to all agents unless overridden.
    #[serde(default)]
    pub default_policy: GuardrailPolicy,

    /// Per-agent policy overrides, keyed by agent ID.
    #[serde(default)]
    pub agent_overrides: HashMap<String, GuardrailPolicy>,
}

impl GuardrailConfig {
    /// Resolve the effective policy for a given agent.
    ///
    /// Returns the agent-specific override if one exists, otherwise
    /// returns the default policy.
    pub fn policy_for(&self, agent_id: &str) -> GuardrailPolicy {
        self.agent_overrides
            .get(agent_id)
            .cloned()
            .unwrap_or_else(|| self.default_policy.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::ToolFilter;

    #[test]
    fn test_default_config() {
        let config = GuardrailConfig::default();
        let policy = config.policy_for("any-agent");
        assert!(policy.tool_filter.is_allowed("any_tool"));
    }

    #[test]
    fn test_agent_override() {
        let config = GuardrailConfig {
            default_policy: GuardrailPolicy::default(),
            agent_overrides: HashMap::from([(
                "restricted-agent".to_string(),
                GuardrailPolicy {
                    tool_filter: ToolFilter::allow_only(["http_get"]),
                    max_message_size: Some(512),
                    reject_on_violation: true,
                },
            )]),
        };

        // Default agent gets permissive policy
        let default = config.policy_for("normal-agent");
        assert!(default.tool_filter.is_allowed("shell_exec"));

        // Restricted agent gets override
        let restricted = config.policy_for("restricted-agent");
        assert!(restricted.tool_filter.is_allowed("http_get"));
        assert!(!restricted.tool_filter.is_allowed("shell_exec"));
        assert_eq!(restricted.max_message_size, Some(512));
    }

    #[test]
    fn test_serde_round_trip() {
        let config = GuardrailConfig {
            default_policy: GuardrailPolicy {
                tool_filter: ToolFilter::deny(["dangerous"]),
                max_message_size: Some(4096),
                reject_on_violation: true,
            },
            agent_overrides: HashMap::new(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: GuardrailConfig = serde_json::from_str(&json).unwrap();
        assert!(
            !deserialized
                .default_policy
                .tool_filter
                .is_allowed("dangerous")
        );
        assert!(
            deserialized
                .default_policy
                .tool_filter
                .is_allowed("safe_tool")
        );
    }
}
