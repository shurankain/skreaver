//! # HTTP Authentication
//!
//! This module provides authentication middleware for the HTTP runtime,
//! supporting JWT tokens and API keys for secure agent access.

use crate::runtime::security::SecretKey;
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
use skreaver_core::{ApiKeyConfig, ApiKeyManager, Role};
use std::sync::Arc;

/// JWT secret key - uses SecretKey with graceful fallback in debug builds
/// In production, uses environment variable or generates random key (invalidates existing tokens)
static JWT_SECRET: Lazy<SecretKey> = Lazy::new(|| {
    #[cfg(any(test, debug_assertions))]
    {
        SecretKey::from_env_or_default(
            "SKREAVER_JWT_SECRET",
            Some("test-secret-for-development-only-generate-real-secret-for-production"),
        )
    }

    #[cfg(not(any(test, debug_assertions)))]
    {
        // In production: load from env or generate random (with warning)
        // This prevents panics but will invalidate existing tokens if env var missing
        SecretKey::from_env_or_default("SKREAVER_JWT_SECRET", None)
    }
});

/// Create API key manager with test key in debug builds only
/// In production, keys should be generated dynamically and stored securely
pub fn create_api_key_manager() -> Arc<ApiKeyManager> {
    let config = ApiKeyConfig {
        prefix: "sk-".to_string(),
        min_length: 32,
        rotation: skreaver_core::auth::api_key::RotationPolicy::Manual,
        default_expiry_days: Some(90), // 90-day expiry by default
        max_keys_per_principal: 10,    // Max 10 keys per user/service
    };

    let manager = Arc::new(ApiKeyManager::new(config));

    // In debug builds, create test key for convenience
    #[cfg(debug_assertions)]
    {
        let manager_clone = Arc::clone(&manager);
        tokio::spawn(async move {
            match manager_clone
                .generate("Test Key (DEBUG BUILD ONLY)".to_string(), vec![Role::Agent])
                .await
            {
                Ok(key) => {
                    // SECURITY: Never log actual API keys, even partial ones
                    tracing::info!(
                        "🔑 Debug API key generated: [REDACTED] (id: {}, name: {})",
                        key.id,
                        key.name
                    );
                }
                Err(e) => {
                    tracing::warn!("Failed to generate debug API key: {}", e);
                }
            }
        });
    }

    // In release builds, warn if test key env var is set (but don't create it)
    #[cfg(not(debug_assertions))]
    {
        if std::env::var("SKREAVER_ENABLE_TEST_KEY").is_ok() {
            tracing::error!(
                "⚠️  SECURITY ERROR: SKREAVER_ENABLE_TEST_KEY is set in RELEASE BUILD. \
                 This variable is IGNORED in release mode. Generate keys via API instead."
            );
        }
    }

    manager
}

/// JWT claims structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,              // Subject (user identifier)
    pub exp: usize,               // Expiration time
    pub iat: usize,               // Issued at
    pub permissions: Vec<String>, // User permissions
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
pub async fn extract_auth_context(
    headers: &HeaderMap,
    api_key_manager: &ApiKeyManager,
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
                // Record successful JWT authentication
                if let Some(registry) = skreaver_observability::get_metrics_registry() {
                    registry
                        .core_metrics()
                        .security_auth_attempts_total
                        .with_label_values(&["success"])
                        .inc();
                }

                return Ok(AuthContext {
                    user_id: token_data.claims.sub,
                    permissions: token_data.claims.permissions,
                    auth_method: AuthMethod::JWT,
                });
            }

            // If JWT validation failed, check if it's an API key (starts with sk-)
            if token.starts_with("sk-") {
                match api_key_manager.authenticate(token).await {
                    Ok(principal) => {
                        // Record successful API key authentication
                        if let Some(registry) = skreaver_observability::get_metrics_registry() {
                            registry
                                .core_metrics()
                                .security_auth_attempts_total
                                .with_label_values(&["success"])
                                .inc();
                        }

                        // Extract permissions from roles
                        let permissions = principal
                            .roles
                            .iter()
                            .map(|role| format!("{:?}", role).to_lowercase())
                            .collect();

                        return Ok(AuthContext {
                            user_id: principal.id,
                            permissions,
                            auth_method: AuthMethod::ApiKey(token.to_string()),
                        });
                    }
                    Err(_e) => {
                        // Record failed API key authentication
                        if let Some(registry) = skreaver_observability::get_metrics_registry() {
                            registry
                                .core_metrics()
                                .security_auth_attempts_total
                                .with_label_values(&["failure"])
                                .inc();
                        }

                        return Err((
                            StatusCode::UNAUTHORIZED,
                            Json(AuthError {
                                error: "invalid_api_key".to_string(),
                                message: "Invalid or revoked API key".to_string(),
                            }),
                        ));
                    }
                }
            }

            // Neither valid JWT nor API key - invalid token
            if let Some(registry) = skreaver_observability::get_metrics_registry() {
                registry
                    .core_metrics()
                    .security_auth_attempts_total
                    .with_label_values(&["invalid"])
                    .inc();
            }

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

        match api_key_manager.authenticate(api_key).await {
            Ok(principal) => {
                // Record successful API key authentication
                if let Some(registry) = skreaver_observability::get_metrics_registry() {
                    registry
                        .core_metrics()
                        .security_auth_attempts_total
                        .with_label_values(&["success"])
                        .inc();
                }

                // Extract permissions from roles
                let permissions = principal
                    .roles
                    .iter()
                    .map(|role| format!("{:?}", role).to_lowercase())
                    .collect();

                return Ok(AuthContext {
                    user_id: principal.id,
                    permissions,
                    auth_method: AuthMethod::ApiKey(api_key.to_string()),
                });
            }
            Err(_e) => {
                // Record failed API key authentication
                if let Some(registry) = skreaver_observability::get_metrics_registry() {
                    registry
                        .core_metrics()
                        .security_auth_attempts_total
                        .with_label_values(&["failure"])
                        .inc();
                }

                return Err((
                    StatusCode::UNAUTHORIZED,
                    Json(AuthError {
                        error: "invalid_api_key".to_string(),
                        message: "Invalid or revoked API key".to_string(),
                    }),
                ));
            }
        }
    }

    // No authentication provided - record as failure
    if let Some(registry) = skreaver_observability::get_metrics_registry() {
        registry
            .core_metrics()
            .security_auth_attempts_total
            .with_label_values(&["failure"])
            .inc();
    }

    Err((
        StatusCode::UNAUTHORIZED,
        Json(AuthError {
            error: "authentication_required".to_string(),
            message: "Authentication is required. Use Authorization header with Bearer token or X-API-Key header".to_string(),
        }),
    ))
}

/// Middleware to inject API key manager into request extensions
pub async fn inject_api_key_manager(
    axum::extract::State(api_key_manager): axum::extract::State<Arc<ApiKeyManager>>,
    mut request: Request,
    next: Next,
) -> Response {
    // Add API key manager to request extensions
    request.extensions_mut().insert(api_key_manager);
    next.run(request).await
}

/// Middleware to require authentication for protected endpoints
/// Note: The API key manager should be added to request extensions by inject_api_key_manager middleware
pub async fn require_auth(
    mut request: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<AuthError>)> {
    // Get API key manager from request extensions
    let api_key_manager = request
        .extensions()
        .get::<Arc<ApiKeyManager>>()
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthError {
                    error: "missing_api_key_manager".to_string(),
                    message: "API key manager not configured".to_string(),
                }),
            )
        })?
        .clone();

    let auth_context = extract_auth_context(request.headers(), &api_key_manager).await?;

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
            // Get API key manager from request extensions
            let api_key_manager = request
                .extensions()
                .get::<Arc<ApiKeyManager>>()
                .ok_or_else(|| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(AuthError {
                            error: "missing_api_key_manager".to_string(),
                            message: "API key manager not configured".to_string(),
                        }),
                    )
                })?
                .clone();

            let auth_context = extract_auth_context(request.headers(), &api_key_manager).await?;

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

    #[tokio::test]
    async fn test_api_key_validation() {
        // Create API key manager
        let manager = create_api_key_manager();

        // Generate a test key
        let key = manager
            .generate("Test Key".to_string(), vec![Role::Agent])
            .await
            .unwrap();

        let mut headers = HeaderMap::new();
        headers.insert(
            "X-API-Key",
            HeaderValue::from_str(key.expose_key()).unwrap(),
        );

        let auth_context = extract_auth_context(&headers, &manager).await.unwrap();
        assert!(!auth_context.user_id.is_empty());
        assert!(!auth_context.permissions.is_empty());
    }
}
