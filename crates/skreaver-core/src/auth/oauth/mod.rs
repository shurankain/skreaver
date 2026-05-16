//! OAuth 2.1 + PKCE authentication support.
//!
//! Implements the authorization code flow with PKCE as mandated by OAuth 2.1.
//! This module provides the core types and logic; HTTP endpoint integration
//! is handled in skreaver-http.

pub mod config;
pub mod pkce;
pub mod state;
pub mod types;

use std::sync::Arc;
use std::time::Duration;

pub use config::OAuthConfig;
pub use pkce::PkceChallenge;
pub use state::{InMemoryStateStore, StateStore};
pub use types::{AuthorizationRequest, GrantType, OAuthError, TokenRequest, TokenResponse};

/// Manages OAuth 2.1 authorization flows.
///
/// Generates authorization URLs with PKCE, stores state parameters,
/// and builds token exchange requests.
pub struct OAuthManager {
    config: OAuthConfig,
    state_store: Arc<dyn StateStore>,
}

impl OAuthManager {
    /// Create a new OAuth manager with the given config and state store.
    pub fn new(config: OAuthConfig, state_store: Arc<dyn StateStore>) -> Result<Self, OAuthError> {
        config.validate()?;
        Ok(Self {
            config,
            state_store,
        })
    }

    /// Create a new OAuth manager with an in-memory state store.
    pub fn with_in_memory_store(config: OAuthConfig) -> Result<Self, OAuthError> {
        Self::new(config, Arc::new(InMemoryStateStore::new()))
    }

    /// Build an authorization URL to redirect the user to.
    ///
    /// Generates a PKCE challenge and CSRF state token, stores them,
    /// and returns the full authorization URL.
    pub fn authorization_url(&self) -> Result<AuthorizationRequest, OAuthError> {
        let pkce = PkceChallenge::generate();
        let state = uuid::Uuid::new_v4().to_string();

        // Store state → PKCE mapping for later verification
        let ttl = Duration::from_secs(self.config.state_ttl_seconds);
        self.state_store.store(&state, pkce.clone(), ttl)?;

        // Build the authorization URL
        let mut url = format!(
            "{}?response_type=code&client_id={}&redirect_uri={}&state={}",
            self.config.authorization_endpoint,
            urlencoded(&self.config.client_id),
            urlencoded(&self.config.redirect_uri),
            urlencoded(&state),
        );

        if !self.config.scopes.is_empty() {
            url.push_str(&format!(
                "&scope={}",
                urlencoded(&self.config.scopes.join(" "))
            ));
        }

        if self.config.pkce_required {
            url.push_str(&format!(
                "&code_challenge={}&code_challenge_method={}",
                urlencoded(&pkce.code_challenge),
                pkce.method,
            ));
        }

        Ok(AuthorizationRequest {
            authorization_url: url,
            state,
        })
    }

    /// Build a token exchange request from an authorization callback.
    ///
    /// Validates the state parameter, retrieves the stored PKCE verifier,
    /// and constructs the token request.
    pub fn build_token_request(&self, code: &str, state: &str) -> Result<TokenRequest, OAuthError> {
        // Retrieve and consume the stored PKCE challenge
        let pkce = self.state_store.take(state)?;

        Ok(TokenRequest {
            grant_type: GrantType::AuthorizationCode,
            code: Some(code.to_string()),
            redirect_uri: self.config.redirect_uri.clone(),
            code_verifier: Some(pkce.code_verifier),
            refresh_token: None,
            client_id: self.config.client_id.clone(),
            client_secret: self.config.client_secret.clone(),
        })
    }

    /// Build a refresh token request.
    pub fn build_refresh_request(&self, refresh_token: &str) -> TokenRequest {
        TokenRequest {
            grant_type: GrantType::RefreshToken,
            code: None,
            redirect_uri: self.config.redirect_uri.clone(),
            code_verifier: None,
            refresh_token: Some(crate::security::SecretString::from_string(
                refresh_token.to_string(),
            )),
            client_id: self.config.client_id.clone(),
            client_secret: self.config.client_secret.clone(),
        }
    }

    /// Access the configuration.
    pub fn config(&self) -> &OAuthConfig {
        &self.config
    }
}

/// Minimal percent-encoding for URL query parameters.
fn urlencoded(s: &str) -> String {
    s.replace('%', "%25")
        .replace('+', "%2B")
        .replace('&', "%26")
        .replace('=', "%3D")
        .replace(' ', "+")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> OAuthConfig {
        OAuthConfig {
            client_id: "test-client".to_string(),
            client_secret: None,
            authorization_endpoint: "https://auth.example.com/authorize".to_string(),
            token_endpoint: "https://auth.example.com/token".to_string(),
            redirect_uri: "http://localhost:8080/callback".to_string(),
            scopes: vec!["openid".to_string(), "profile".to_string()],
            pkce_required: true,
            state_ttl_seconds: 600,
        }
    }

    #[test]
    fn test_authorization_url_contains_required_params() {
        let manager = OAuthManager::with_in_memory_store(test_config()).unwrap();
        let req = manager.authorization_url().unwrap();

        assert!(req.authorization_url.contains("response_type=code"));
        assert!(req.authorization_url.contains("client_id=test-client"));
        assert!(req.authorization_url.contains("redirect_uri="));
        assert!(req.authorization_url.contains("state="));
        assert!(req.authorization_url.contains("code_challenge="));
        assert!(req.authorization_url.contains("code_challenge_method=S256"));
        assert!(req.authorization_url.contains("scope=openid+profile"));
    }

    #[test]
    fn test_authorization_url_starts_with_endpoint() {
        let manager = OAuthManager::with_in_memory_store(test_config()).unwrap();
        let req = manager.authorization_url().unwrap();
        assert!(
            req.authorization_url
                .starts_with("https://auth.example.com/authorize?")
        );
    }

    #[test]
    fn test_build_token_request_round_trip() {
        let manager = OAuthManager::with_in_memory_store(test_config()).unwrap();
        let auth_req = manager.authorization_url().unwrap();

        let token_req = manager
            .build_token_request("auth-code-123", &auth_req.state)
            .unwrap();
        assert_eq!(token_req.grant_type, GrantType::AuthorizationCode);
        assert_eq!(token_req.code.as_deref(), Some("auth-code-123"));
        assert!(token_req.code_verifier.is_some());
        assert_eq!(token_req.client_id, "test-client");
    }

    #[test]
    fn test_state_consumed_on_token_request() {
        let manager = OAuthManager::with_in_memory_store(test_config()).unwrap();
        let auth_req = manager.authorization_url().unwrap();

        // First use works
        manager
            .build_token_request("code", &auth_req.state)
            .unwrap();

        // Second use fails (replay protection)
        assert!(
            manager
                .build_token_request("code", &auth_req.state)
                .is_err()
        );
    }

    #[test]
    fn test_invalid_state_rejected() {
        let manager = OAuthManager::with_in_memory_store(test_config()).unwrap();
        assert!(manager.build_token_request("code", "bogus-state").is_err());
    }

    #[test]
    fn test_refresh_request() {
        let manager = OAuthManager::with_in_memory_store(test_config()).unwrap();
        let req = manager.build_refresh_request("rt_abc123");
        assert_eq!(req.grant_type, GrantType::RefreshToken);
        assert!(req.code.is_none());
        assert!(req.code_verifier.is_none());
        assert!(req.refresh_token.is_some());
    }

    #[test]
    fn test_invalid_config_rejected() {
        let mut config = test_config();
        config.client_id = String::new();
        assert!(OAuthManager::with_in_memory_store(config).is_err());
    }
}
