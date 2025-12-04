//! JWT configuration

use jsonwebtoken::Algorithm;

/// JWT token refresh policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshPolicy {
    /// Refresh disabled - tokens cannot be refreshed
    Disabled,
    /// Manual refresh - user must explicitly request refresh
    Manual,
    /// Automatic refresh with sliding window
    Automatic {
        /// Minutes before expiry to auto-refresh
        window_minutes: i64,
    },
}

impl RefreshPolicy {
    /// Check if refresh is allowed
    pub fn is_allowed(self) -> bool {
        !matches!(self, Self::Disabled)
    }

    /// Check if automatic refresh is enabled
    pub fn is_automatic(self) -> bool {
        matches!(self, Self::Automatic { .. })
    }

    /// Get refresh window in minutes (if automatic)
    pub fn window_minutes(self) -> Option<i64> {
        match self {
            Self::Automatic { window_minutes } => Some(window_minutes),
            _ => None,
        }
    }
}

impl Default for RefreshPolicy {
    fn default() -> Self {
        Self::Manual
    }
}

/// JWT configuration
#[derive(Debug, Clone)]
pub struct JwtConfig {
    /// Secret key for HMAC signing
    pub secret: String,
    /// Token issuer
    pub issuer: String,
    /// Token audience
    pub audience: Vec<String>,
    /// Token expiration in minutes
    pub expiry_minutes: i64,
    /// Refresh token expiration in days
    pub refresh_expiry_days: i64,
    /// Algorithm to use (HS256, HS384, HS512)
    pub algorithm: Algorithm,
    /// Token refresh policy
    pub refresh: RefreshPolicy,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: "change-me-in-production".to_string(),
            issuer: "skreaver".to_string(),
            audience: vec!["skreaver-api".to_string()],
            expiry_minutes: 60,
            refresh_expiry_days: 30,
            algorithm: Algorithm::HS256,
            refresh: RefreshPolicy::default(),
        }
    }
}

impl JwtConfig {
    /// Create config with automatic refresh
    pub fn with_auto_refresh(window_minutes: i64) -> Self {
        Self {
            refresh: RefreshPolicy::Automatic { window_minutes },
            ..Default::default()
        }
    }

    /// Create config with refresh disabled
    pub fn no_refresh() -> Self {
        Self {
            refresh: RefreshPolicy::Disabled,
            ..Default::default()
        }
    }
}
