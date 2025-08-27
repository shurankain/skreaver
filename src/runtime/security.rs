//! # Security Management
//!
//! Comprehensive security module handling secret management, input sanitization,
//! and security headers for the HTTP runtime.

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Security configuration for the HTTP runtime
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// JWT signing key (should be loaded from environment or secure storage)
    pub jwt_secret: SecretKey,
    /// API keys storage
    pub api_keys: Arc<RwLock<HashMap<String, ApiKeyData>>>,
    /// Content Security Policy configuration
    pub csp: ContentSecurityPolicy,
    /// Input validation configuration
    pub input_validation: InputValidationConfig,
    /// Security headers configuration
    pub security_headers: SecurityHeadersConfig,
}

/// Secure wrapper for sensitive data
#[derive(Debug, Clone)]
pub struct SecretKey {
    value: String,
}

impl SecretKey {
    /// Create a new secret key (prefer loading from environment)
    pub fn new(value: String) -> Self {
        if value.len() < 32 {
            tracing::warn!(
                "JWT secret key is shorter than recommended 32 characters. \
                This may compromise security."
            );
        }
        Self { value }
    }

    /// Load from environment variable with fallback
    pub fn from_env_or_default(env_var: &str, default: Option<&str>) -> Self {
        match std::env::var(env_var) {
            Ok(secret) => {
                if secret.is_empty() {
                    tracing::error!("Environment variable {} is empty", env_var);
                    Self::generate_random()
                } else {
                    tracing::info!("Loaded JWT secret from environment variable {}", env_var);
                    Self::new(secret)
                }
            }
            Err(_) => {
                if let Some(default_val) = default {
                    tracing::warn!(
                        "Environment variable {} not found. Using provided default. \
                        This should only be used in development!",
                        env_var
                    );
                    Self::new(default_val.to_string())
                } else {
                    tracing::warn!(
                        "Environment variable {} not found. Generating random key. \
                        This will invalidate existing tokens!",
                        env_var
                    );
                    Self::generate_random()
                }
            }
        }
    }

    /// Generate a cryptographically secure random key
    pub fn generate_random() -> Self {
        use rand::RngCore;
        let mut rng = rand::thread_rng();
        let mut random_bytes = [0u8; 64];
        rng.fill_bytes(&mut random_bytes);
        let key = base64::engine::general_purpose::STANDARD.encode(random_bytes);
        tracing::info!("Generated new random JWT secret (64 bytes, base64 encoded)");
        Self::new(key)
    }

    /// Get the key value (use sparingly and never log)
    pub fn as_bytes(&self) -> &[u8] {
        self.value.as_bytes()
    }

    /// Check if key meets security requirements
    pub fn is_secure(&self) -> bool {
        self.value.len() >= 32 && !self.value.chars().all(|c| c.is_alphanumeric()) // Should contain special characters
    }
}

// Prevent accidental logging of secret keys
impl std::fmt::Display for SecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[REDACTED {} bytes]", self.value.len())
    }
}

/// API key data with enhanced security metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyData {
    /// Human-readable name for the API key
    pub name: String,
    /// Permissions granted to this API key
    pub permissions: Vec<String>,
    /// When the API key was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Optional expiration date
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Usage tracking
    pub usage_count: u64,
    /// Last time this key was used
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    /// IP addresses that have used this key (for security monitoring)
    pub used_from_ips: Vec<std::net::IpAddr>,
    /// Whether the key is currently active
    pub is_active: bool,
    /// Rate limit overrides for this specific key (serialized as JSON)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_overrides: Option<serde_json::Value>,
}

impl ApiKeyData {
    /// Create a new API key data
    pub fn new(name: String, permissions: Vec<String>) -> Self {
        Self {
            name,
            permissions,
            created_at: chrono::Utc::now(),
            expires_at: None,
            usage_count: 0,
            last_used_at: None,
            used_from_ips: Vec::new(),
            is_active: true,
            rate_limit_overrides: None,
        }
    }

    /// Check if the API key is valid and not expired
    pub fn is_valid(&self) -> bool {
        if !self.is_active {
            return false;
        }

        if let Some(expires_at) = self.expires_at {
            if chrono::Utc::now() > expires_at {
                return false;
            }
        }

        true
    }

    /// Record usage of this API key
    pub fn record_usage(&mut self, from_ip: std::net::IpAddr) {
        self.usage_count += 1;
        self.last_used_at = Some(chrono::Utc::now());

        // Track unique IPs (limit to last 100 for memory efficiency)
        if !self.used_from_ips.contains(&from_ip) {
            self.used_from_ips.push(from_ip);
            if self.used_from_ips.len() > 100 {
                self.used_from_ips.remove(0);
            }
        }
    }

    /// Set expiration date
    pub fn with_expiration(mut self, expires_at: chrono::DateTime<chrono::Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Deactivate the API key
    pub fn deactivate(&mut self) {
        self.is_active = false;
    }
}

/// Content Security Policy configuration
#[derive(Debug, Clone)]
pub struct ContentSecurityPolicy {
    pub default_src: Vec<String>,
    pub script_src: Vec<String>,
    pub style_src: Vec<String>,
    pub img_src: Vec<String>,
    pub connect_src: Vec<String>,
    pub font_src: Vec<String>,
    pub object_src: Vec<String>,
    pub media_src: Vec<String>,
    pub frame_src: Vec<String>,
}

impl Default for ContentSecurityPolicy {
    fn default() -> Self {
        Self {
            default_src: vec!["'self'".to_string()],
            script_src: vec!["'self'".to_string(), "'unsafe-inline'".to_string()], // For Swagger UI
            style_src: vec![
                "'self'".to_string(),
                "'unsafe-inline'".to_string(),
                "https://unpkg.com".to_string(),
            ],
            img_src: vec!["'self'".to_string(), "data:".to_string()],
            connect_src: vec!["'self'".to_string()],
            font_src: vec!["'self'".to_string(), "https://unpkg.com".to_string()],
            object_src: vec!["'none'".to_string()],
            media_src: vec!["'self'".to_string()],
            frame_src: vec!["'none'".to_string()],
        }
    }
}

impl ContentSecurityPolicy {
    /// Generate CSP header value
    pub fn to_header_value(&self) -> String {
        let mut directives = Vec::new();

        if !self.default_src.is_empty() {
            directives.push(format!("default-src {}", self.default_src.join(" ")));
        }
        if !self.script_src.is_empty() {
            directives.push(format!("script-src {}", self.script_src.join(" ")));
        }
        if !self.style_src.is_empty() {
            directives.push(format!("style-src {}", self.style_src.join(" ")));
        }
        if !self.img_src.is_empty() {
            directives.push(format!("img-src {}", self.img_src.join(" ")));
        }
        if !self.connect_src.is_empty() {
            directives.push(format!("connect-src {}", self.connect_src.join(" ")));
        }
        if !self.font_src.is_empty() {
            directives.push(format!("font-src {}", self.font_src.join(" ")));
        }
        if !self.object_src.is_empty() {
            directives.push(format!("object-src {}", self.object_src.join(" ")));
        }
        if !self.media_src.is_empty() {
            directives.push(format!("media-src {}", self.media_src.join(" ")));
        }
        if !self.frame_src.is_empty() {
            directives.push(format!("frame-src {}", self.frame_src.join(" ")));
        }

        directives.join("; ")
    }
}

/// Input validation configuration
#[derive(Debug, Clone)]
pub struct InputValidationConfig {
    /// Maximum length for string inputs
    pub max_string_length: usize,
    /// Maximum number of items in arrays
    pub max_array_length: usize,
    /// Maximum recursion depth for nested objects
    pub max_object_depth: usize,
    /// Allowed characters for agent IDs
    pub agent_id_pattern: regex::Regex,
    /// Forbidden patterns in user input
    pub forbidden_patterns: Vec<regex::Regex>,
}

impl Default for InputValidationConfig {
    fn default() -> Self {
        Self {
            max_string_length: 10_000,
            max_array_length: 100,
            max_object_depth: 10,
            agent_id_pattern: regex::Regex::new(r"^[a-zA-Z0-9_-]{1,64}$").unwrap(),
            forbidden_patterns: vec![
                // SQL injection patterns
                regex::Regex::new(r"(?i)(union|select|insert|update|delete|drop|exec|script)")
                    .unwrap(),
                // Script injection patterns
                regex::Regex::new(r"(?i)<script|javascript:|on\w+\s*=").unwrap(),
                // Path traversal patterns
                regex::Regex::new(r"\.\./|\.\.\\\|%2e%2e").unwrap(),
            ],
        }
    }
}

impl InputValidationConfig {
    /// Validate a string input
    pub fn validate_string(&self, input: &str, field_name: &str) -> Result<(), String> {
        if input.len() > self.max_string_length {
            return Err(format!(
                "Field '{}' exceeds maximum length of {} characters",
                field_name, self.max_string_length
            ));
        }

        for (i, pattern) in self.forbidden_patterns.iter().enumerate() {
            if pattern.is_match(input) {
                return Err(format!(
                    "Field '{}' contains forbidden pattern (rule {})",
                    field_name,
                    i + 1
                ));
            }
        }

        Ok(())
    }

    /// Validate an agent ID
    pub fn validate_agent_id(&self, agent_id: &str) -> Result<(), String> {
        if !self.agent_id_pattern.is_match(agent_id) {
            return Err(format!(
                "Agent ID '{}' contains invalid characters. Must be alphanumeric with hyphens/underscores, 1-64 characters",
                agent_id
            ));
        }
        Ok(())
    }

    /// Sanitize user input by removing or escaping dangerous content
    pub fn sanitize_string(&self, input: &str) -> String {
        let mut sanitized = input.to_string();

        // Remove null bytes
        sanitized = sanitized.replace('\0', "");

        // Escape HTML entities
        sanitized = html_escape::encode_text(&sanitized).to_string();

        // Truncate if too long
        if sanitized.len() > self.max_string_length {
            sanitized.truncate(self.max_string_length);
            sanitized.push_str("...[truncated]");
        }

        sanitized
    }
}

/// Security headers configuration
#[derive(Debug, Clone)]
pub struct SecurityHeadersConfig {
    pub enable_hsts: bool,
    pub enable_frame_options: bool,
    pub enable_content_type_options: bool,
    pub enable_xss_protection: bool,
    pub enable_referrer_policy: bool,
    pub enable_permissions_policy: bool,
}

impl Default for SecurityHeadersConfig {
    fn default() -> Self {
        Self {
            enable_hsts: true,
            enable_frame_options: true,
            enable_content_type_options: true,
            enable_xss_protection: true,
            enable_referrer_policy: true,
            enable_permissions_policy: true,
        }
    }
}

impl SecurityHeadersConfig {
    /// Get all security headers as key-value pairs
    pub fn to_headers(&self) -> Vec<(&'static str, &'static str)> {
        let mut headers = Vec::new();

        if self.enable_hsts {
            headers.push((
                "Strict-Transport-Security",
                "max-age=31536000; includeSubDomains",
            ));
        }
        if self.enable_frame_options {
            headers.push(("X-Frame-Options", "DENY"));
        }
        if self.enable_content_type_options {
            headers.push(("X-Content-Type-Options", "nosniff"));
        }
        if self.enable_xss_protection {
            headers.push(("X-XSS-Protection", "1; mode=block"));
        }
        if self.enable_referrer_policy {
            headers.push(("Referrer-Policy", "strict-origin-when-cross-origin"));
        }
        if self.enable_permissions_policy {
            headers.push((
                "Permissions-Policy",
                "camera=(), microphone=(), geolocation=(), payment=()",
            ));
        }

        headers
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            jwt_secret: SecretKey::from_env_or_default(
                "SKREAVER_JWT_SECRET",
                Some("dev-only-insecure-key-change-in-production!"),
            ),
            api_keys: Arc::new(RwLock::new({
                let mut keys = HashMap::new();
                // Add default test key for development
                keys.insert(
                    "sk-test-key-123".to_string(),
                    ApiKeyData::new(
                        "Development Test Key".to_string(),
                        vec!["read".to_string(), "write".to_string()],
                    ),
                );
                keys
            })),
            csp: ContentSecurityPolicy::default(),
            input_validation: InputValidationConfig::default(),
            security_headers: SecurityHeadersConfig::default(),
        }
    }
}

impl SecurityConfig {
    /// Create a production-ready security configuration
    pub fn production() -> Self {
        Self {
            jwt_secret: SecretKey::from_env_or_default("SKREAVER_JWT_SECRET", None),
            api_keys: Arc::new(RwLock::new(HashMap::new())), // No default keys in production
            csp: ContentSecurityPolicy::default(),
            input_validation: InputValidationConfig::default(),
            security_headers: SecurityHeadersConfig::default(),
        }
    }

    /// Add an API key to the configuration
    pub fn add_api_key(&self, key: String, data: ApiKeyData) {
        if let Ok(mut keys) = self.api_keys.write() {
            keys.insert(key, data);
        } else {
            tracing::error!("Failed to acquire write lock for API keys");
        }
    }

    /// Get API key data if valid
    pub fn get_api_key(&self, key: &str) -> Option<ApiKeyData> {
        if let Ok(keys) = self.api_keys.read() {
            keys.get(key).filter(|data| data.is_valid()).cloned()
        } else {
            tracing::error!("Failed to acquire read lock for API keys");
            None
        }
    }

    /// Record API key usage
    pub fn record_api_key_usage(&self, key: &str, from_ip: std::net::IpAddr) -> Result<(), String> {
        if let Ok(mut keys) = self.api_keys.write() {
            if let Some(data) = keys.get_mut(key) {
                data.record_usage(from_ip);
                Ok(())
            } else {
                Err("API key not found".to_string())
            }
        } else {
            Err("Failed to acquire write lock for API keys".to_string())
        }
    }

    /// List all API keys (for management endpoints)
    pub fn list_api_keys(&self) -> Vec<(String, ApiKeyData)> {
        if let Ok(keys) = self.api_keys.read() {
            keys.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        } else {
            tracing::error!("Failed to acquire read lock for API keys");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_key_security() {
        let weak_key = SecretKey::new("short".to_string());
        assert!(!weak_key.is_secure());

        let strong_key =
            SecretKey::new("this-is-a-much-longer-key-with-special-chars-!@#$".to_string());
        assert!(strong_key.is_secure());
    }

    #[test]
    fn test_api_key_validation() {
        let mut key_data = ApiKeyData::new("Test Key".to_string(), vec!["read".to_string()]);

        assert!(key_data.is_valid());

        key_data.deactivate();
        assert!(!key_data.is_valid());
    }

    #[test]
    fn test_input_validation() {
        let config = InputValidationConfig::default();

        // Valid agent ID
        assert!(config.validate_agent_id("valid-agent-123").is_ok());

        // Invalid agent ID with special characters
        assert!(config.validate_agent_id("invalid@agent").is_err());

        // SQL injection attempt
        assert!(
            config
                .validate_string("'; DROP TABLE users; --", "input")
                .is_err()
        );

        // Valid normal input
        assert!(config.validate_string("Hello, world!", "input").is_ok());
    }

    #[test]
    fn test_csp_header_generation() {
        let csp = ContentSecurityPolicy::default();
        let header = csp.to_header_value();

        assert!(header.contains("default-src 'self'"));
        assert!(header.contains("object-src 'none'"));
    }

    #[test]
    fn test_input_sanitization() {
        let config = InputValidationConfig::default();

        let dangerous_input = "<script>alert('xss')</script>";
        let sanitized = config.sanitize_string(dangerous_input);

        assert!(!sanitized.contains("<script>"));
        assert!(sanitized.contains("&lt;script&gt;"));
    }
}
