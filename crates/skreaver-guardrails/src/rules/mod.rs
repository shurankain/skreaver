//! Built-in guardrail rules.
//!
//! Each rule wraps an existing skreaver-core component to avoid
//! duplicating validation logic.

pub mod input_validation;
pub mod message_size;
pub mod output_scan;
pub mod rate_limit;
pub mod tool_filter;

pub use input_validation::InputValidationRule;
pub use message_size::MessageSizeRule;
pub use output_scan::OutputScanRule;
pub use rate_limit::RateLimitRule;
pub use tool_filter::ToolFilterRule;
