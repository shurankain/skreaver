//! Guardrail policy definitions.
//!
//! Defines tool filtering rules and message constraints that
//! `GuardedAgent` enforces before delegating to the inner agent.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Determines which tools an agent is permitted to invoke.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ToolFilter {
    /// All tools are allowed (no restriction).
    AllowAll,
    /// Only listed tools are allowed; everything else is denied.
    AllowList { tools: HashSet<String> },
    /// All tools are allowed except those listed.
    DenyList { tools: HashSet<String> },
    /// Allowlist with explicit denylist override (deny wins on conflict).
    AllowDeny {
        allow: HashSet<String>,
        deny: HashSet<String>,
    },
}

impl ToolFilter {
    /// Check whether a tool name is permitted by this filter.
    pub fn is_allowed(&self, tool_name: &str) -> bool {
        match self {
            ToolFilter::AllowAll => true,
            ToolFilter::AllowList { tools } => tools.contains(tool_name),
            ToolFilter::DenyList { tools } => !tools.contains(tool_name),
            ToolFilter::AllowDeny { allow, deny } => {
                !deny.contains(tool_name) && allow.contains(tool_name)
            }
        }
    }

    /// Create a filter that allows all tools.
    pub fn allow_all() -> Self {
        Self::AllowAll
    }

    /// Create a filter that only allows the specified tools.
    pub fn allow_only(tools: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::AllowList {
            tools: tools.into_iter().map(Into::into).collect(),
        }
    }

    /// Create a filter that denies the specified tools.
    pub fn deny(tools: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::DenyList {
            tools: tools.into_iter().map(Into::into).collect(),
        }
    }
}

impl Default for ToolFilter {
    fn default() -> Self {
        Self::AllowAll
    }
}

/// Top-level guardrail policy for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardrailPolicy {
    /// Which tools the agent may invoke.
    #[serde(default)]
    pub tool_filter: ToolFilter,

    /// Maximum total content length (bytes) across all parts of a message.
    /// `None` means no guardrail limit (SecurityManager may still enforce its own).
    #[serde(default)]
    pub max_message_size: Option<usize>,

    /// If true, reject messages that violate policy.
    /// If false, log a warning but allow the message through.
    #[serde(default = "default_true")]
    pub reject_on_violation: bool,
}

fn default_true() -> bool {
    true
}

impl Default for GuardrailPolicy {
    fn default() -> Self {
        Self {
            tool_filter: ToolFilter::default(),
            max_message_size: None,
            reject_on_violation: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allow_all() {
        let filter = ToolFilter::allow_all();
        assert!(filter.is_allowed("any_tool"));
        assert!(filter.is_allowed("another"));
    }

    #[test]
    fn test_allow_list() {
        let filter = ToolFilter::allow_only(["http_get", "file_read"]);
        assert!(filter.is_allowed("http_get"));
        assert!(filter.is_allowed("file_read"));
        assert!(!filter.is_allowed("shell_exec"));
    }

    #[test]
    fn test_deny_list() {
        let filter = ToolFilter::deny(["shell_exec", "file_delete"]);
        assert!(filter.is_allowed("http_get"));
        assert!(!filter.is_allowed("shell_exec"));
        assert!(!filter.is_allowed("file_delete"));
    }

    #[test]
    fn test_allow_deny_deny_wins() {
        let filter = ToolFilter::AllowDeny {
            allow: HashSet::from(["http_get".to_string(), "shell_exec".to_string()]),
            deny: HashSet::from(["shell_exec".to_string()]),
        };
        assert!(filter.is_allowed("http_get"));
        assert!(!filter.is_allowed("shell_exec")); // deny wins
        assert!(!filter.is_allowed("unknown")); // not in allow
    }

    #[test]
    fn test_default_is_allow_all() {
        let filter = ToolFilter::default();
        assert!(filter.is_allowed("anything"));
    }

    #[test]
    fn test_policy_default() {
        let policy = GuardrailPolicy::default();
        assert!(policy.tool_filter.is_allowed("any"));
        assert!(policy.max_message_size.is_none());
        assert!(policy.reject_on_violation);
    }

    #[test]
    fn test_serde_round_trip() {
        let policy = GuardrailPolicy {
            tool_filter: ToolFilter::allow_only(["http_get"]),
            max_message_size: Some(1024),
            reject_on_violation: false,
        };
        let json = serde_json::to_string(&policy).unwrap();
        let deserialized: GuardrailPolicy = serde_json::from_str(&json).unwrap();
        assert!(!deserialized.reject_on_violation);
        assert_eq!(deserialized.max_message_size, Some(1024));
        assert!(deserialized.tool_filter.is_allowed("http_get"));
        assert!(!deserialized.tool_filter.is_allowed("other"));
    }
}
