//! JWT (JSON Web Token) authentication support

use super::{AuthError, AuthMethod, AuthResult, Principal, TokenBlacklist};
use crate::auth::rbac::Role;
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

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

/// JWT Claims structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    /// Subject (user/principal ID)
    pub sub: String,
    /// Principal name
    pub name: String,
    /// Issuer
    pub iss: String,
    /// Audience
    pub aud: Vec<String>,
    /// Expiration time (Unix timestamp)
    pub exp: i64,
    /// Issued at (Unix timestamp)
    pub iat: i64,
    /// Not before (Unix timestamp)
    pub nbf: i64,
    /// JWT ID
    pub jti: String,
    /// Token type (access/refresh)
    pub typ: String,
    /// User roles
    pub roles: Vec<String>,
    /// Additional custom claims
    pub custom: HashMap<String, serde_json::Value>,
}

impl JwtClaims {
    /// Create new claims for a principal
    #[must_use]
    pub fn new(principal: &Principal, config: &JwtConfig, token_type: &str) -> Self {
        let now = Utc::now();
        let expiry = if token_type == "refresh" {
            now + Duration::days(config.refresh_expiry_days)
        } else {
            now + Duration::minutes(config.expiry_minutes)
        };

        Self {
            sub: principal.id.clone(),
            name: principal.name.clone(),
            iss: config.issuer.clone(),
            aud: config.audience.clone(),
            exp: expiry.timestamp(),
            iat: now.timestamp(),
            nbf: now.timestamp(),
            jti: uuid::Uuid::new_v4().to_string(),
            typ: token_type.to_string(),
            roles: principal.roles.iter().map(ToString::to_string).collect(),
            custom: HashMap::new(),
        }
    }

    /// Check if the token is expired
    #[must_use]
    pub fn is_expired(&self) -> bool {
        let now = Utc::now().timestamp();
        now > self.exp
    }

    /// Check if the token is valid
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let now = Utc::now().timestamp();
        !self.is_expired() && now >= self.nbf
    }

    /// Convert role strings back to Role enums
    #[must_use]
    pub fn get_roles(&self) -> Vec<Role> {
        self.roles
            .iter()
            .filter_map(|r| match r.as_str() {
                "admin" => Some(Role::Admin),
                "agent" => Some(Role::Agent),
                "viewer" => Some(Role::Viewer),
                _ => None,
            })
            .collect()
    }
}

/// JWT token wrapper
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

/// JWT Manager for token operations
pub struct JwtManager {
    config: JwtConfig,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    validation: Validation,
    blacklist: Option<Arc<dyn TokenBlacklist>>,
}

impl JwtManager {
    /// Create a new JWT manager without token revocation
    #[must_use]
    pub fn new(config: JwtConfig) -> Self {
        let encoding_key = EncodingKey::from_secret(config.secret.as_bytes());
        let decoding_key = DecodingKey::from_secret(config.secret.as_bytes());

        let mut validation = Validation::new(config.algorithm);
        validation.set_issuer(std::slice::from_ref(&config.issuer));
        validation.set_audience(&config.audience);
        validation.validate_exp = true;
        validation.validate_nbf = true;

        Self {
            config,
            encoding_key,
            decoding_key,
            validation,
            blacklist: None,
        }
    }

    /// Create a new JWT manager with token revocation support
    ///
    /// # Example
    ///
    /// ```ignore
    /// use skreaver_core::auth::{JwtConfig, JwtManager, InMemoryBlacklist};
    /// use std::sync::Arc;
    ///
    /// let config = JwtConfig::default();
    /// let blacklist = Arc::new(InMemoryBlacklist::new());
    /// let manager = JwtManager::with_blacklist(config, blacklist);
    /// ```
    #[must_use]
    pub fn with_blacklist(config: JwtConfig, blacklist: Arc<dyn TokenBlacklist>) -> Self {
        let mut manager = Self::new(config);
        manager.blacklist = Some(blacklist);
        manager
    }

    /// Generate a new JWT token for a principal
    ///
    /// # Errors
    ///
    /// Returns an error if token encoding fails.
    #[allow(clippy::unused_async)]
    pub async fn generate(&self, principal: &Principal) -> AuthResult<JwtToken> {
        // Create access token claims
        let access_claims = JwtClaims::new(principal, &self.config, "access");

        // Encode access token
        let header = Header::new(self.config.algorithm);
        let access_token = encode(&header, &access_claims, &self.encoding_key)
            .map_err(|e| AuthError::ValidationError(format!("Failed to encode JWT: {e}")))?;

        // Create refresh token if enabled
        let refresh_token = if self.config.allow_refresh {
            let refresh_claims = JwtClaims::new(principal, &self.config, "refresh");
            Some(
                encode(&header, &refresh_claims, &self.encoding_key).map_err(|e| {
                    AuthError::ValidationError(format!("Failed to encode refresh token: {e}"))
                })?,
            )
        } else {
            None
        };

        Ok(JwtToken {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.config.expiry_minutes * 60,
            issued_at: Utc::now(),
        })
    }

    /// Authenticate with a JWT token
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The token is malformed or has invalid signature
    /// - The token is expired (`AuthError::TokenExpired`)
    /// - The token has been revoked (`AuthError::InvalidToken`)
    /// - The token is not yet valid
    pub async fn authenticate(&self, token: &str) -> AuthResult<Principal> {
        // Decode and validate the token
        let token_data = decode::<JwtClaims>(token, &self.decoding_key, &self.validation).map_err(
            |e| match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
                _ => AuthError::InvalidToken(format!("JWT validation failed: {e}")),
            },
        )?;

        let claims = token_data.claims;

        // Check if token is blacklisted (revoked)
        if let Some(ref blacklist) = self.blacklist
            && blacklist.is_revoked(&claims.jti).await?
        {
            return Err(AuthError::InvalidToken(
                "Token has been revoked".to_string(),
            ));
        }

        // Additional validation
        if !claims.is_valid() {
            return Err(AuthError::InvalidToken(
                "Token is not yet valid or expired".to_string(),
            ));
        }

        // Create principal from claims
        let mut principal = Principal::new(
            claims.sub.clone(),
            claims.name.clone(),
            AuthMethod::Bearer(claims.jti.clone()),
        );

        // Add roles
        for role in claims.get_roles() {
            principal = principal.with_role(role);
        }

        // Add metadata
        principal = principal.with_metadata("token_type".to_string(), claims.typ);
        principal = principal.with_metadata("issued_at".to_string(), claims.iat.to_string());

        Ok(principal)
    }

    /// Refresh a token
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Token refresh is not allowed
    /// - The refresh token is invalid or malformed
    /// - The refresh token has been revoked
    /// - The token is not a refresh token type
    pub async fn refresh(&self, refresh_token: &str) -> AuthResult<JwtToken> {
        if !self.config.allow_refresh {
            return Err(AuthError::ValidationError(
                "Token refresh not allowed".to_string(),
            ));
        }

        // Decode the refresh token
        let token_data =
            decode::<JwtClaims>(refresh_token, &self.decoding_key, &self.validation)
                .map_err(|e| AuthError::InvalidToken(format!("Invalid refresh token: {e}")))?;

        let claims = token_data.claims;

        // Check if refresh token is blacklisted (revoked)
        if let Some(ref blacklist) = self.blacklist
            && blacklist.is_revoked(&claims.jti).await?
        {
            return Err(AuthError::InvalidToken(
                "Refresh token has been revoked".to_string(),
            ));
        }

        // Verify it's a refresh token
        if claims.typ != "refresh" {
            return Err(AuthError::InvalidToken("Not a refresh token".to_string()));
        }

        // Create a new principal from refresh token claims
        let mut principal = Principal::new(
            claims.sub.clone(),
            claims.name.clone(),
            AuthMethod::Bearer(claims.jti.clone()),
        );

        for role in claims.get_roles() {
            principal = principal.with_role(role);
        }

        // Generate new tokens
        self.generate(&principal).await
    }

    /// Verify a token without full authentication
    ///
    /// # Errors
    ///
    /// Returns `AuthError::InvalidToken` if the token is malformed or has an invalid signature.
    pub fn verify(&self, token: &str) -> AuthResult<JwtClaims> {
        let token_data = decode::<JwtClaims>(token, &self.decoding_key, &self.validation)
            .map_err(|e| AuthError::InvalidToken(format!("JWT verification failed: {e}")))?;

        Ok(token_data.claims)
    }

    /// Revoke a token by adding it to the blacklist
    ///
    /// The token is added to the blacklist with TTL equal to its remaining validity period.
    /// After the token expires naturally, it will be automatically removed from the blacklist.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The token is malformed or invalid
    /// - No blacklist is configured (`AuthError::ValidationError`)
    /// - The blacklist operation fails (`AuthError::StorageError`)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use skreaver_core::auth::{JwtConfig, JwtManager, InMemoryBlacklist};
    /// use std::sync::Arc;
    ///
    /// let config = JwtConfig::default();
    /// let blacklist = Arc::new(InMemoryBlacklist::new());
    /// let manager = JwtManager::with_blacklist(config, blacklist);
    ///
    /// // Generate token
    /// let token = manager.generate(&principal).await?;
    ///
    /// // Revoke it
    /// manager.revoke(&token.access_token).await?;
    ///
    /// // Subsequent authentication will fail
    /// assert!(manager.authenticate(&token.access_token).await.is_err());
    /// ```
    pub async fn revoke(&self, token: &str) -> AuthResult<()> {
        // Verify token is valid before revoking
        let claims = self.verify(token)?;

        // Check if blacklist is configured
        let blacklist = self.blacklist.as_ref().ok_or_else(|| {
            AuthError::ValidationError(
                "Token revocation not enabled (no blacklist configured)".to_string(),
            )
        })?;

        // Calculate TTL: time until token expires
        let now = Utc::now().timestamp();
        let ttl_seconds = claims.exp - now;

        // Only add to blacklist if token hasn't expired yet
        if ttl_seconds > 0 {
            blacklist.revoke(&claims.jti, ttl_seconds).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_jwt_generation() {
        let config = JwtConfig::default();
        let manager = JwtManager::new(config);

        let principal = Principal::new(
            "user-123".to_string(),
            "Test User".to_string(),
            AuthMethod::ApiKey("test".to_string()),
        )
        .with_role(Role::Agent);

        let token = manager.generate(&principal).await.unwrap();
        assert!(!token.access_token.is_empty());
        assert_eq!(token.token_type, "Bearer");
        assert!(token.refresh_token.is_some());
    }

    #[tokio::test]
    async fn test_jwt_authentication() {
        let config = JwtConfig::default();
        let manager = JwtManager::new(config);

        let principal = Principal::new(
            "user-123".to_string(),
            "Test User".to_string(),
            AuthMethod::ApiKey("test".to_string()),
        )
        .with_role(Role::Agent);

        let token = manager.generate(&principal).await.unwrap();

        let authenticated = manager.authenticate(&token.access_token).await.unwrap();
        assert_eq!(authenticated.id, "user-123");
        assert_eq!(authenticated.name, "Test User");
        assert!(authenticated.has_role(&Role::Agent));
    }

    #[tokio::test]
    async fn test_jwt_refresh() {
        let config = JwtConfig::default();
        let manager = JwtManager::new(config);

        let principal = Principal::new(
            "user-123".to_string(),
            "Test User".to_string(),
            AuthMethod::ApiKey("test".to_string()),
        )
        .with_role(Role::Admin);

        let token = manager.generate(&principal).await.unwrap();
        let refresh_token = token.refresh_token.unwrap();

        let new_token = manager.refresh(&refresh_token).await.unwrap();
        assert!(!new_token.access_token.is_empty());
        assert_ne!(new_token.access_token, token.access_token);
    }

    #[tokio::test]
    async fn test_jwt_expiration() {
        use jsonwebtoken::{Header, encode};

        let config = JwtConfig::default();
        let manager = JwtManager::new(config.clone());

        // Manually create an expired token
        let expired_claims = JwtClaims {
            sub: "user-123".to_string(),
            name: "Test User".to_string(),
            iss: config.issuer.clone(),
            aud: config.audience.clone(),
            exp: (Utc::now() - Duration::minutes(10)).timestamp(), // Expired 10 minutes ago
            iat: (Utc::now() - Duration::minutes(20)).timestamp(),
            nbf: (Utc::now() - Duration::minutes(20)).timestamp(),
            jti: uuid::Uuid::new_v4().to_string(),
            typ: "access".to_string(),
            roles: vec!["agent".to_string()],
            custom: HashMap::new(),
        };

        let header = Header::new(config.algorithm);
        let expired_token = encode(&header, &expired_claims, &manager.encoding_key).unwrap();

        let result = manager.authenticate(&expired_token).await;
        assert!(matches!(result, Err(AuthError::TokenExpired)));
    }

    #[tokio::test]
    async fn test_jwt_revocation_basic() {
        use crate::auth::InMemoryBlacklist;

        let config = JwtConfig::default();
        let blacklist = Arc::new(InMemoryBlacklist::new());
        let manager = JwtManager::with_blacklist(config, blacklist.clone());

        let principal = Principal::new(
            "user-123".to_string(),
            "Test User".to_string(),
            AuthMethod::ApiKey("test".to_string()),
        )
        .with_role(Role::Agent);

        // Generate token
        let token = manager.generate(&principal).await.unwrap();

        // Token should work initially
        let auth1 = manager.authenticate(&token.access_token).await;
        assert!(auth1.is_ok());

        // Revoke token
        manager.revoke(&token.access_token).await.unwrap();

        // Token should now fail authentication
        let auth2 = manager.authenticate(&token.access_token).await;
        assert!(auth2.is_err());
        assert!(matches!(
            auth2,
            Err(AuthError::InvalidToken(ref msg)) if msg.contains("revoked")
        ));

        // Check blacklist contains the token
        let claims = manager.verify(&token.access_token).unwrap();
        assert!(blacklist.is_revoked(&claims.jti).await.unwrap());
    }

    #[tokio::test]
    async fn test_jwt_revocation_without_blacklist() {
        let config = JwtConfig::default();
        let manager = JwtManager::new(config);

        let principal = Principal::new(
            "user-123".to_string(),
            "Test User".to_string(),
            AuthMethod::ApiKey("test".to_string()),
        );

        let token = manager.generate(&principal).await.unwrap();

        // Attempting to revoke without blacklist should fail
        let result = manager.revoke(&token.access_token).await;
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(AuthError::ValidationError(ref msg)) if msg.contains("not enabled")
        ));

        // Token should still work (revocation failed)
        assert!(manager.authenticate(&token.access_token).await.is_ok());
    }

    #[tokio::test]
    async fn test_jwt_revocation_multiple_tokens() {
        use crate::auth::InMemoryBlacklist;

        let config = JwtConfig::default();
        let blacklist = Arc::new(InMemoryBlacklist::new());
        let manager = JwtManager::with_blacklist(config, blacklist.clone());

        let principal1 = Principal::new(
            "user-1".to_string(),
            "User One".to_string(),
            AuthMethod::ApiKey("test".to_string()),
        );

        let principal2 = Principal::new(
            "user-2".to_string(),
            "User Two".to_string(),
            AuthMethod::ApiKey("test".to_string()),
        );

        // Generate tokens for both users
        let token1 = manager.generate(&principal1).await.unwrap();
        let token2 = manager.generate(&principal2).await.unwrap();

        // Both tokens should work
        assert!(manager.authenticate(&token1.access_token).await.is_ok());
        assert!(manager.authenticate(&token2.access_token).await.is_ok());

        // Revoke only token1
        manager.revoke(&token1.access_token).await.unwrap();

        // token1 should fail, token2 should still work
        assert!(manager.authenticate(&token1.access_token).await.is_err());
        assert!(manager.authenticate(&token2.access_token).await.is_ok());

        // Blacklist should contain only 1 token
        assert_eq!(blacklist.count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_jwt_revocation_with_ttl() {
        use crate::auth::InMemoryBlacklist;

        let config = JwtConfig {
            expiry_minutes: 5, // 5 minutes
            ..Default::default()
        };

        let blacklist = Arc::new(InMemoryBlacklist::new());
        let manager = JwtManager::with_blacklist(config, blacklist.clone());

        let principal = Principal::new(
            "user-123".to_string(),
            "Test User".to_string(),
            AuthMethod::ApiKey("test".to_string()),
        );

        let token = manager.generate(&principal).await.unwrap();
        manager.revoke(&token.access_token).await.unwrap();

        // Verify token is blacklisted
        let claims = manager.verify(&token.access_token).unwrap();
        assert!(blacklist.is_revoked(&claims.jti).await.unwrap());

        // TTL should be approximately 5 minutes (300 seconds)
        // We can't check exact TTL in InMemoryBlacklist, but we verified it was added
        assert_eq!(blacklist.count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_jwt_revocation_refresh_token() {
        use crate::auth::InMemoryBlacklist;

        let config = JwtConfig::default();
        let blacklist = Arc::new(InMemoryBlacklist::new());
        let manager = JwtManager::with_blacklist(config, blacklist.clone());

        let principal = Principal::new(
            "user-123".to_string(),
            "Test User".to_string(),
            AuthMethod::ApiKey("test".to_string()),
        );

        let token = manager.generate(&principal).await.unwrap();
        let refresh_token = token.refresh_token.unwrap();

        // Revoke refresh token
        manager.revoke(&refresh_token).await.unwrap();

        // Refresh should fail
        let result = manager.refresh(&refresh_token).await;
        assert!(result.is_err());

        // Access token should still work (not revoked)
        assert!(manager.authenticate(&token.access_token).await.is_ok());
    }

    #[tokio::test]
    async fn test_jwt_revocation_invalid_token() {
        use crate::auth::InMemoryBlacklist;

        let config = JwtConfig::default();
        let blacklist = Arc::new(InMemoryBlacklist::new());
        let manager = JwtManager::with_blacklist(config, blacklist);

        // Try to revoke invalid token
        let result = manager.revoke("invalid.token.string").await;
        assert!(result.is_err());
        assert!(matches!(result, Err(AuthError::InvalidToken(_))));
    }

    #[tokio::test]
    async fn test_jwt_revocation_already_expired() {
        use crate::auth::InMemoryBlacklist;
        use jsonwebtoken::{Header, encode};

        let config = JwtConfig::default();
        let blacklist = Arc::new(InMemoryBlacklist::new());
        let manager = JwtManager::with_blacklist(config.clone(), blacklist.clone());

        // Create already expired token
        let expired_claims = JwtClaims {
            sub: "user-123".to_string(),
            name: "Test User".to_string(),
            iss: config.issuer.clone(),
            aud: config.audience.clone(),
            exp: (Utc::now() - Duration::minutes(10)).timestamp(),
            iat: (Utc::now() - Duration::minutes(20)).timestamp(),
            nbf: (Utc::now() - Duration::minutes(20)).timestamp(),
            jti: uuid::Uuid::new_v4().to_string(),
            typ: "access".to_string(),
            roles: vec![],
            custom: HashMap::new(),
        };

        let header = Header::new(config.algorithm);
        let encoding_key = EncodingKey::from_secret(config.secret.as_bytes());
        let expired_token = encode(&header, &expired_claims, &encoding_key).unwrap();

        // Revoking expired token should still succeed (verify will pass)
        let result = manager.revoke(&expired_token).await;
        // Note: This will fail validation during verify() due to expiration
        assert!(result.is_err());
    }
}
