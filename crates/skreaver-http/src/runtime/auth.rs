//! # HTTP Authentication
//!
//! This module provides authentication middleware for the HTTP runtime,
//! supporting JWT tokens and API keys for secure agent access.

use axum::{
    Json,
    extract::Request,
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::Response,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// JWT secret key - MUST be set via SKREAVER_JWT_SECRET environment variable in production
/// In debug/test builds, uses a default test secret for convenience
static JWT_SECRET: Lazy<String> = Lazy::new(|| {
    std::env::var("SKREAVER_JWT_SECRET").unwrap_or_else(|_| {
        // In test/debug builds, provide a default. In release builds, panic.
        #[cfg(any(test, debug_assertions))]
        {
            "test-secret-for-development-only-generate-real-secret-for-production".to_string()
        }

        #[cfg(not(any(test, debug_assertions)))]
        {
            panic!("SKREAVER_JWT_SECRET environment variable must be set in production. Generate with: openssl rand -base64 32")
        }
    })
});

/// API keys storage - Hardcoded test key in debug builds, empty in release builds
/// In production, this should be backed by a database (see skreaver-core::auth::AuthManager)
static API_KEYS: Lazy<HashMap<String, ApiKeyData>> = Lazy::new(|| {
    let mut keys = HashMap::new();

    // In debug builds, include test key for convenience
    #[cfg(debug_assertions)]
    {
        keys.insert(
            "sk-test-key-123".to_string(),
            ApiKeyData {
                name: "Test Key (DEBUG BUILD ONLY)".to_string(),
                permissions: vec!["read".to_string(), "write".to_string()],
                created_at: Utc::now(),
            },
        );
    }

    // In release builds, only add test key if explicitly enabled
    #[cfg(not(debug_assertions))]
    {
        if std::env::var("SKREAVER_ENABLE_TEST_KEY").is_ok() {
            tracing::warn!(
                "⚠️  SECURITY WARNING: Test API key 'sk-test-key-123' is enabled in RELEASE BUILD. \
                 DO NOT USE IN PRODUCTION. Unset SKREAVER_ENABLE_TEST_KEY to disable."
            );
            keys.insert(
                "sk-test-key-123".to_string(),
                ApiKeyData {
                    name: "Test Key (DANGER: ENABLED IN RELEASE)".to_string(),
                    permissions: vec!["read".to_string(), "write".to_string()],
                    created_at: Utc::now(),
                },
            );
        }
    }

    keys
});

/// JWT claims structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,              // Subject (user identifier)
    pub exp: usize,               // Expiration time
    pub iat: usize,               // Issued at
    pub permissions: Vec<String>, // User permissions
}

/// API key metadata
#[derive(Debug, Clone)]
pub struct ApiKeyData {
    pub name: String,
    pub permissions: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Authentication context passed to handlers
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: String,
    pub permissions: Vec<String>,
    pub auth_method: AuthMethod,
}

/// Authentication method used
#[derive(Debug, Clone)]
pub enum AuthMethod {
    JWT,
    ApiKey(String),
}

/// Error response for authentication failures
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct AuthError {
    pub error: String,
    pub message: String,
}

impl Claims {
    /// Create new JWT claims with 24-hour expiration
    pub fn new(user_id: String, permissions: Vec<String>) -> Self {
        let now = Utc::now();
        Self {
            sub: user_id,
            iat: now.timestamp() as usize,
            exp: (now + Duration::hours(24)).timestamp() as usize,
            permissions,
        }
    }
}

/// Generate a JWT token for a user
pub fn create_jwt_token(
    user_id: String,
    permissions: Vec<String>,
) -> Result<String, jsonwebtoken::errors::Error> {
    let claims = Claims::new(user_id, permissions);
    let encoding_key = EncodingKey::from_secret(JWT_SECRET.as_bytes());
    encode(&Header::default(), &claims, &encoding_key)
}

/// Validate a JWT token and extract claims
pub fn validate_jwt_token(token: &str) -> Result<TokenData<Claims>, jsonwebtoken::errors::Error> {
    let decoding_key = DecodingKey::from_secret(JWT_SECRET.as_bytes());
    let validation = Validation::default();
    decode::<Claims>(token, &decoding_key, &validation)
}

/// Extract auth context from request headers
pub fn extract_auth_context(
    headers: &HeaderMap,
) -> Result<AuthContext, (StatusCode, Json<AuthError>)> {
    // Check for Authorization header
    if let Some(auth_header) = headers.get(AUTHORIZATION) {
        let auth_str = auth_header.to_str().map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(AuthError {
                    error: "invalid_header".to_string(),
                    message: "Invalid Authorization header format".to_string(),
                }),
            )
        })?;

        // Check Bearer token - could be JWT or API Key
        if let Some(token) = auth_str.strip_prefix("Bearer ") {
            // Try JWT first
            if let Ok(token_data) = validate_jwt_token(token) {
                return Ok(AuthContext {
                    user_id: token_data.claims.sub,
                    permissions: token_data.claims.permissions,
                    auth_method: AuthMethod::JWT,
                });
            }

            // If JWT validation failed, check if it's an API key (starts with sk-)
            if token.starts_with("sk-") {
                if let Some(key_data) = API_KEYS.get(token) {
                    return Ok(AuthContext {
                        user_id: format!("api-key-{}", &token[3..std::cmp::min(11, token.len())]), // First 8 chars after sk-
                        permissions: key_data.permissions.clone(),
                        auth_method: AuthMethod::ApiKey(token.to_string()),
                    });
                } else {
                    return Err((
                        StatusCode::UNAUTHORIZED,
                        Json(AuthError {
                            error: "invalid_api_key".to_string(),
                            message: "Invalid API key".to_string(),
                        }),
                    ));
                }
            }

            // Neither valid JWT nor API key
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(AuthError {
                    error: "invalid_token".to_string(),
                    message: "Invalid or expired JWT token or API key".to_string(),
                }),
            ));
        }
    }

    // Check for X-API-Key header
    if let Some(api_key_header) = headers.get("X-API-Key") {
        let api_key = api_key_header.to_str().map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(AuthError {
                    error: "invalid_header".to_string(),
                    message: "Invalid X-API-Key header format".to_string(),
                }),
            )
        })?;

        if let Some(key_data) = API_KEYS.get(api_key) {
            return Ok(AuthContext {
                user_id: format!("api-key-{}", &api_key[3..11]), // First 8 chars after sk-
                permissions: key_data.permissions.clone(),
                auth_method: AuthMethod::ApiKey(api_key.to_string()),
            });
        } else {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(AuthError {
                    error: "invalid_api_key".to_string(),
                    message: "Invalid API key".to_string(),
                }),
            ));
        }
    }

    // No authentication provided
    Err((
        StatusCode::UNAUTHORIZED,
        Json(AuthError {
            error: "authentication_required".to_string(),
            message: "Authentication is required. Use Authorization header with Bearer token or X-API-Key header".to_string(),
        }),
    ))
}

/// Middleware to require authentication for protected endpoints
pub async fn require_auth(
    mut request: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<AuthError>)> {
    let auth_context = extract_auth_context(request.headers())?;

    // Add auth context to request extensions for handlers to access
    request.extensions_mut().insert(auth_context);

    Ok(next.run(request).await)
}

/// Result type for auth middleware
type AuthResult = Result<Response, (StatusCode, Json<AuthError>)>;

/// Future type for auth middleware
type AuthFuture = std::pin::Pin<Box<dyn std::future::Future<Output = AuthResult> + Send>>;

/// Middleware to require specific permissions
pub fn require_permissions(
    required_permissions: Vec<&'static str>,
) -> impl Fn(Request, Next) -> AuthFuture + Clone {
    move |mut request: Request, next: Next| {
        let required_perms = required_permissions.clone();
        Box::pin(async move {
            let auth_context = extract_auth_context(request.headers())?;

            // Check if user has required permissions
            let has_permission = required_perms
                .iter()
                .all(|perm| auth_context.permissions.contains(&perm.to_string()));

            if !has_permission {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(AuthError {
                        error: "insufficient_permissions".to_string(),
                        message: format!("Required permissions: {}", required_perms.join(", ")),
                    }),
                ));
            }

            request.extensions_mut().insert(auth_context);
            Ok(next.run(request).await)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_create_and_validate_jwt() {
        let user_id = "test-user".to_string();
        let permissions = vec!["read".to_string(), "write".to_string()];

        // Create token
        let token = create_jwt_token(user_id.clone(), permissions.clone()).unwrap();

        // Validate token
        let token_data = validate_jwt_token(&token).unwrap();
        assert_eq!(token_data.claims.sub, user_id);
        assert_eq!(token_data.claims.permissions, permissions);
    }

    #[test]
    fn test_api_key_validation() {
        let mut headers = HeaderMap::new();
        headers.insert("X-API-Key", HeaderValue::from_static("sk-test-key-123"));

        let auth_context = extract_auth_context(&headers).unwrap();
        assert!(auth_context.user_id.starts_with("api-key-"));
        assert!(auth_context.permissions.contains(&"read".to_string()));
    }
}
