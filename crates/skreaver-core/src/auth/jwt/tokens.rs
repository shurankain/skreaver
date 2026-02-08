//! Type-safe JWT tokens using phantom types

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use subtle::ConstantTimeEq;

// ============================================================================
// Phantom type markers for compile-time token discrimination
// ============================================================================

/// Marker type for Access tokens
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessToken;

/// Marker type for Refresh tokens
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefreshToken;

// ============================================================================
// Type-safe Token<T> wrapper
// ============================================================================

/// Type-safe token wrapper using phantom types.
/// The type parameter `T` represents the token type and provides
/// compile-time guarantees about token usage.
#[derive(Debug, Clone)]
pub struct Token<T> {
    value: String,
    expires_at: DateTime<Utc>,
    issued_at: DateTime<Utc>,
    _phantom: PhantomData<T>,
}

impl<T> Token<T> {
    /// Create a new token
    pub(crate) fn new(value: String, expires_at: DateTime<Utc>, issued_at: DateTime<Utc>) -> Self {
        Self {
            value,
            expires_at,
            issued_at,
            _phantom: PhantomData,
        }
    }

    /// Create a token from a raw string for validation purposes.
    ///
    /// The timestamps are set to placeholder values since they will be
    /// extracted from the JWT claims during validation. This is useful
    /// when you have a token string and need to validate it.
    pub(crate) fn from_raw(value: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            value: value.into(),
            expires_at: now,
            issued_at: now,
            _phantom: PhantomData,
        }
    }

    /// Get the token value as a string
    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// Get the expiration time
    pub fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }

    /// Get when the token was issued
    pub fn issued_at(&self) -> DateTime<Utc> {
        self.issued_at
    }

    /// Check if the token is expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Time until expiration
    pub fn time_until_expiry(&self) -> Duration {
        self.expires_at - Utc::now()
    }
}

// ============================================================================
// Token type-specific methods
// ============================================================================

impl Token<AccessToken> {
    /// Get expiration time in seconds (common for access tokens)
    pub fn expiry_seconds(&self) -> i64 {
        self.time_until_expiry().num_seconds().max(0)
    }

    /// Check if token will expire soon (within 5 minutes)
    pub fn expires_soon(&self) -> bool {
        self.expiry_seconds() < 300
    }
}

impl Token<RefreshToken> {
    /// Get expiration time in days (common for refresh tokens)
    pub fn expiry_days(&self) -> i64 {
        self.time_until_expiry().num_days().max(0)
    }

    /// Check if token will expire soon (within 7 days)
    pub fn expires_soon(&self) -> bool {
        self.expiry_days() < 7
    }
}

impl<T> PartialEq for Token<T> {
    fn eq(&self, other: &Self) -> bool {
        // LOW-47: Use constant-time comparison to prevent timing attacks
        // Standard == operator can leak token length/content through timing differences
        self.value.as_bytes().ct_eq(other.value.as_bytes()).into()
    }
}

impl<T> Eq for Token<T> {}

// ============================================================================
// TokenPair - Type-safe pair of access and refresh tokens
// ============================================================================

/// Type-safe token pair containing access and optional refresh token
#[derive(Debug, Clone)]
pub struct TokenPair {
    /// Access token for authentication
    pub access: Token<AccessToken>,
    /// Optional refresh token for obtaining new access tokens
    pub refresh: Option<Token<RefreshToken>>,
    /// Token type (always "Bearer" for JWT)
    pub token_type: &'static str,
}

impl TokenPair {
    /// Create a new token pair
    pub fn new(access: Token<AccessToken>, refresh: Option<Token<RefreshToken>>) -> Self {
        Self {
            access,
            refresh,
            token_type: "Bearer",
        }
    }

    /// Check if refresh token is available
    pub fn has_refresh_token(&self) -> bool {
        self.refresh.is_some()
    }

    /// Get time until access token expires (in seconds)
    pub fn expires_in(&self) -> i64 {
        self.access.expiry_seconds()
    }
}

// ============================================================================
// Backward compatibility: Legacy JWT token structure
// ============================================================================

/// JWT token wrapper (legacy structure for backward compatibility)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtToken {
    /// Access token
    pub access_token: String,
    /// Refresh token (if enabled)
    pub refresh_token: Option<String>,
    /// Token type (Bearer)
    pub token_type: String,
    /// Expiration time in seconds
    pub expires_in: i64,
    /// Issued at timestamp
    pub issued_at: DateTime<Utc>,
}

impl From<TokenPair> for JwtToken {
    fn from(pair: TokenPair) -> Self {
        let expires_in = pair.expires_in();
        let issued_at = pair.access.issued_at;
        let access_token = pair.access.value;
        let refresh_token = pair.refresh.map(|t| t.value);

        JwtToken {
            access_token,
            refresh_token,
            token_type: pair.token_type.to_string(),
            expires_in,
            issued_at,
        }
    }
}
