//! JWT configuration

use jsonwebtoken::Algorithm;

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
    /// Allow token refresh
    pub allow_refresh: bool,
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
            allow_refresh: true,
        }
    }
}
