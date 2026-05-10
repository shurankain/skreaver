//! Rate limit rule — enforces per-agent request rate.
//!
//! Wraps `skreaver_core::security::limits::RateLimiter`.

use std::time::Duration;

use skreaver_core::security::limits::RateLimiter;

use crate::rule::{Rule, RuleContext, RuleResult};

/// Limits the number of messages an agent can process within a time window.
pub struct RateLimitRule {
    limiter: RateLimiter,
}

impl RateLimitRule {
    /// Create a rate limit of `n` requests per minute.
    pub fn per_minute(n: u32) -> Self {
        Self {
            limiter: RateLimiter::new(n, Duration::from_secs(60)),
        }
    }

    /// Create a rate limit with a custom window.
    pub fn new(requests: u32, window: Duration) -> Self {
        Self {
            limiter: RateLimiter::new(requests, window),
        }
    }
}

impl Rule for RateLimitRule {
    fn name(&self) -> &str {
        "rate_limit"
    }

    fn check_pre(
        &self,
        ctx: &RuleContext<'_>,
        _message: &skreaver_agent::types::UnifiedMessage,
    ) -> RuleResult {
        let agent_id = &ctx.agent_info.id;
        match self.limiter.check_rate_limit(agent_id) {
            Ok(()) => RuleResult::Allow,
            Err(e) => RuleResult::Deny(format!("Rate limit exceeded: {}", e)),
        }
    }
}
