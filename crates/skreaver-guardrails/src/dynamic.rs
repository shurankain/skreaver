//! Dynamic policy switching based on threat levels.
//!
//! `DynamicPolicy` maps `ThreatLevel` to `GuardrailPolicy` and tracks
//! the current level via an `Arc<RwLock>`, allowing runtime escalation
//! from anomaly detectors.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::anomaly::ThreatLevel;
use crate::policy::GuardrailPolicy;

/// A policy that changes based on the current threat level.
///
/// Configure a policy per level; at runtime, the current level
/// determines which policy is active.
#[derive(Clone)]
pub struct DynamicPolicy {
    levels: HashMap<ThreatLevel, GuardrailPolicy>,
    default: GuardrailPolicy,
    current: Arc<RwLock<ThreatLevel>>,
}

impl DynamicPolicy {
    /// Create a dynamic policy with a default policy for `Normal` level.
    pub fn new(default: GuardrailPolicy) -> Self {
        Self {
            levels: HashMap::new(),
            default,
            current: Arc::new(RwLock::new(ThreatLevel::Normal)),
        }
    }

    /// Set the policy for a specific threat level (builder pattern).
    pub fn with_level(mut self, level: ThreatLevel, policy: GuardrailPolicy) -> Self {
        self.levels.insert(level, policy);
        self
    }

    /// Get the current threat level.
    pub fn current_level(&self) -> ThreatLevel {
        *self.current.read().unwrap_or_else(|e| e.into_inner())
    }

    /// Set the current threat level (called by anomaly detectors).
    pub fn set_level(&self, level: ThreatLevel) {
        if let Ok(mut current) = self.current.write() {
            *current = level;
        }
    }

    /// Get the policy for the current threat level.
    ///
    /// Falls back to the default policy if no policy is configured
    /// for the current level.
    pub fn policy(&self) -> GuardrailPolicy {
        let level = self.current_level();
        self.levels
            .get(&level)
            .cloned()
            .unwrap_or_else(|| self.default.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::ToolFilter;

    #[test]
    fn test_default_is_normal() {
        let dp = DynamicPolicy::new(GuardrailPolicy::default());
        assert_eq!(dp.current_level(), ThreatLevel::Normal);
    }

    #[test]
    fn test_level_switching() {
        let dp = DynamicPolicy::new(GuardrailPolicy::default());
        dp.set_level(ThreatLevel::High);
        assert_eq!(dp.current_level(), ThreatLevel::High);
    }

    #[test]
    fn test_policy_per_level() {
        let normal = GuardrailPolicy {
            tool_filter: ToolFilter::AllowAll,
            ..Default::default()
        };
        let critical = GuardrailPolicy {
            tool_filter: ToolFilter::allow_only(["http_get"]),
            max_message_size: Some(1024),
            ..Default::default()
        };

        let dp = DynamicPolicy::new(normal).with_level(ThreatLevel::Critical, critical);

        // Normal level → permissive
        assert!(dp.policy().tool_filter.is_allowed("shell_exec"));

        // Escalate → restrictive
        dp.set_level(ThreatLevel::Critical);
        assert!(!dp.policy().tool_filter.is_allowed("shell_exec"));
        assert!(dp.policy().tool_filter.is_allowed("http_get"));
        assert_eq!(dp.policy().max_message_size, Some(1024));
    }

    #[test]
    fn test_falls_back_to_default() {
        let dp = DynamicPolicy::new(GuardrailPolicy::default());
        dp.set_level(ThreatLevel::Elevated);
        // No policy configured for Elevated → falls back to default
        assert!(dp.policy().tool_filter.is_allowed("anything"));
    }
}
