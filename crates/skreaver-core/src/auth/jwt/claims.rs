//! JWT claims structures and validation

use super::config::JwtConfig;
use crate::auth::{Principal, rbac::Role};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
