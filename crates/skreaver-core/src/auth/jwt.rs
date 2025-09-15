//! JWT (JSON Web Token) authentication support

use super::{AuthError, AuthMethod, AuthResult, Principal};
use crate::auth::rbac::Role;
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
            roles: principal.roles.iter().map(|r| r.to_string()).collect(),
            custom: HashMap::new(),
        }
    }

    /// Check if the token is expired
    pub fn is_expired(&self) -> bool {
        let now = Utc::now().timestamp();
        now > self.exp
    }

    /// Check if the token is valid
    pub fn is_valid(&self) -> bool {
        let now = Utc::now().timestamp();
        !self.is_expired() && now >= self.nbf
    }

    /// Convert role strings back to Role enums
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
}

impl JwtManager {
    /// Create a new JWT manager
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
        }
    }

    /// Generate a new JWT token for a principal
    pub async fn generate(&self, principal: &Principal) -> AuthResult<JwtToken> {
        // Create access token claims
        let access_claims = JwtClaims::new(principal, &self.config, "access");

        // Encode access token
        let header = Header::new(self.config.algorithm);
        let access_token = encode(&header, &access_claims, &self.encoding_key)
            .map_err(|e| AuthError::ValidationError(format!("Failed to encode JWT: {}", e)))?;

        // Create refresh token if enabled
        let refresh_token = if self.config.allow_refresh {
            let refresh_claims = JwtClaims::new(principal, &self.config, "refresh");
            Some(
                encode(&header, &refresh_claims, &self.encoding_key).map_err(|e| {
                    AuthError::ValidationError(format!("Failed to encode refresh token: {}", e))
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
    pub async fn authenticate(&self, token: &str) -> AuthResult<Principal> {
        // Decode and validate the token
        let token_data = decode::<JwtClaims>(token, &self.decoding_key, &self.validation).map_err(
            |e| match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
                _ => AuthError::InvalidToken(format!("JWT validation failed: {}", e)),
            },
        )?;

        let claims = token_data.claims;

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
    pub async fn refresh(&self, refresh_token: &str) -> AuthResult<JwtToken> {
        if !self.config.allow_refresh {
            return Err(AuthError::ValidationError(
                "Token refresh not allowed".to_string(),
            ));
        }

        // Decode the refresh token
        let token_data =
            decode::<JwtClaims>(refresh_token, &self.decoding_key, &self.validation)
                .map_err(|e| AuthError::InvalidToken(format!("Invalid refresh token: {}", e)))?;

        let claims = token_data.claims;

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
    pub async fn verify(&self, token: &str) -> AuthResult<JwtClaims> {
        let token_data = decode::<JwtClaims>(token, &self.decoding_key, &self.validation)
            .map_err(|e| AuthError::InvalidToken(format!("JWT verification failed: {}", e)))?;

        Ok(token_data.claims)
    }

    /// Revoke a token (would require a blacklist/cache in production)
    pub async fn revoke(&self, token: &str) -> AuthResult<()> {
        // In production, this would add the token JTI to a blacklist
        // For now, we just verify it's a valid token
        self.verify(token).await?;

        // TODO: Implement token blacklist with Redis or similar

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
}
