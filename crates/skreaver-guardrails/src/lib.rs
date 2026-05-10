//! # Skreaver Guardrails
//!
//! Runtime guardrails for AI agent execution. Wraps any `UnifiedAgent`
//! with configurable pre/post execution checks: tool allowlists/denylists,
//! message size limits, input validation, rate limiting, and output scanning.
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
//! // Advanced: compose individual rules
//! use skreaver_guardrails::rules::{InputValidationRule, RateLimitRule, MessageSizeRule};
//!
//! let guarded = GuardedAgentBuilder::new(my_agent)
//!     .allow_tools(["http_get", "file_read"])
//!     .with_rule(MessageSizeRule::new(1024 * 1024))
//!     .with_rule(InputValidationRule::new())
//!     .with_rule(RateLimitRule::per_minute(60))
//!     .build();
//! ```

pub mod agent;
pub mod builder;
pub mod config;
pub mod error;
pub mod policy;
pub mod preset;
pub mod rule;
pub mod rules;

pub use agent::GuardedAgent;
pub use builder::GuardedAgentBuilder;
pub use config::GuardrailConfig;
pub use error::{GuardrailError, GuardrailResult};
pub use policy::{GuardrailPolicy, ToolFilter};
pub use preset::Preset;
pub use rule::{Rule, RuleContext, RuleResult, RuleSet};
