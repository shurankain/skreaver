//! # Skreaver Guardrails
//!
//! Runtime guardrails for AI agent execution. Wraps any `UnifiedAgent`
//! with configurable pre/post execution checks: tool allowlists/denylists,
//! message size limits, input validation, rate limiting, output scanning,
//! human approval hooks, anomaly detection, and dynamic policy switching.
//!
//! Built on top of `skreaver-core::security` — reuses `InputValidator`,
//! `RateLimiter`, and `SecretRedactor` instead of duplicating logic.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use skreaver_guardrails::{GuardedAgentBuilder, Preset};
//!
//! // Simple: use a preset
//! let guarded = GuardedAgentBuilder::new(my_agent)
//!     .with_preset(Preset::Standard)
//!     .build();
//!
//! // Advanced: approval hooks + anomaly detection
//! use skreaver_guardrails::approval::ApprovalRule;
//! use skreaver_guardrails::anomaly::ThresholdDetector;
//! use skreaver_guardrails::dynamic::DynamicPolicy;
//!
//! let dp = Arc::new(DynamicPolicy::new(normal_policy)
//!     .with_level(ThreatLevel::Critical, lockdown_policy));
//!
//! let guarded = GuardedAgentBuilder::new(my_agent)
//!     .with_preset(Preset::Strict)
//!     .with_async_rule(ApprovalRule::on_tools(hook, ["dangerous_tool"], timeout))
//!     .with_dynamic_policy(dp)
//!     .with_anomaly_detector(Arc::new(ThresholdDetector::default_config()))
//!     .build();
//! ```

pub mod agent;
pub mod anomaly;
pub mod approval;
pub mod builder;
pub mod config;
pub mod dynamic;
pub mod error;
pub mod policy;
pub mod preset;
pub mod rule;
pub mod rules;

pub use agent::GuardedAgent;
pub use anomaly::{AnomalyDetector, ThreatLevel, ThreatScore};
pub use approval::{ApprovalDecision, ApprovalHook, ApprovalRule};
pub use builder::GuardedAgentBuilder;
pub use config::GuardrailConfig;
pub use dynamic::DynamicPolicy;
pub use error::{GuardrailError, GuardrailResult};
pub use policy::{GuardrailPolicy, ToolFilter};
pub use preset::Preset;
pub use rule::{AsyncRule, Rule, RuleContext, RuleResult, RuleSet};
