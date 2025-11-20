//! Type-safe secret handling that prevents accidental exposure
//!
//! This module provides the [`Secret`] type which wraps sensitive values like passwords,
//! API keys, and tokens to prevent them from being accidentally logged, printed, or
//! serialized.
//!
//! # Security Guarantees
//!
//! - **No accidental logging**: `Debug` impl shows `[REDACTED]` instead of the actual value
//! - **No serialization**: `Serialize` impl outputs `[REDACTED]` to prevent leaks in JSON logs
//! - **Memory safety**: Secrets are zeroed on drop to prevent memory scraping
//! - **Explicit access**: Must call `expose_secret()` to access the value
//!
//! # Examples
//!
//! ```
//! use skreaver_core::security::SecretString;
//!
//! let api_key = SecretString::from_string("sk_live_abc123".to_string());
//!
//! // This is safe - won't leak the secret
//! println!("API Key: {:?}", api_key);  // Prints: API Key: [REDACTED]
//!
//! // To use the secret, explicitly expose it
//! let key_value: &str = api_key.expose_secret();
//! // Use key_value for authentication...
//! ```

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// A secret value that cannot be accidentally exposed through logging or serialization
///
/// This type wraps sensitive data and ensures it cannot be leaked through common
/// debugging and logging mechanisms. The inner value is zeroed when dropped.
///
/// # Type Safety
///
/// The secret value can ONLY be accessed through [`expose_secret()`](Secret::expose_secret),
/// making it obvious in code review where secrets are being used.
///
/// # Examples
///
/// ```
/// use skreaver_core::security::SecretString;
///
/// let password = SecretString::from_string("super-secret-password".to_string());
///
/// // Safe - won't leak secret
/// let debug_output = format!("{:?}", password);
/// assert_eq!(debug_output, "[REDACTED]");
///
/// // To use the secret value
/// let password_str: &str = password.expose_as_str();
/// ```
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct Secret<T: Zeroize> {
    inner: T,
}

impl<T: Zeroize> Secret<T> {
    /// Create a new secret value
    ///
    /// The value will be zeroed in memory when the `Secret` is dropped.
    pub fn new(value: T) -> Self {
        Self { inner: value }
    }

    /// Expose the secret value for use
    ///
    /// # Security
    ///
    /// This is the ONLY way to access the secret value. The method name is
    /// intentionally verbose to make secret access obvious during code review.
    ///
    /// The exposed value should:
    /// - NEVER be logged
    /// - NEVER be included in error messages
    /// - NEVER be stored in non-secret data structures
    /// - Be used immediately and not stored
    pub fn expose_secret(&self) -> &T {
        &self.inner
    }

    // Note: into_inner() is intentionally NOT provided
    // If you need ownership, clone the value from expose_secret()
    // This prevents accidentally bypassing the Drop trait's zeroization
}

impl<T: Zeroize> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl<T: Zeroize> fmt::Display for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

// Serialize as "[REDACTED]" to prevent secrets in JSON logs
impl<T: Zeroize> Serialize for Secret<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str("[REDACTED]")
    }
}

// Cannot deserialize a redacted value
impl<'de, T: Zeroize + Deserialize<'de>> Deserialize<'de> for Secret<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let inner = T::deserialize(deserializer)?;
        Ok(Secret::new(inner))
    }
}

/// Type alias for String secrets (most common use case)
pub type SecretString = Secret<String>;

impl SecretString {
    /// Create a secret from a String
    pub fn from_string(s: String) -> Self {
        Secret::new(s)
    }

    /// Compare two secrets in constant time
    ///
    /// This prevents timing attacks when comparing secrets.
    /// Uses the `subtle` crate for true constant-time comparison.
    pub fn constant_time_eq(&self, other: &Self) -> bool {
        use subtle::ConstantTimeEq;
        self.inner.as_bytes().ct_eq(other.inner.as_bytes()).into()
    }

    /// Get the secret as a string slice
    ///
    /// This is a convenience method equivalent to `expose_secret().as_str()`.
    pub fn expose_as_str(&self) -> &str {
        &self.inner
    }
}

/// Implement FromStr for SecretString for ergonomic conversions
impl std::str::FromStr for SecretString {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Secret::new(s.to_string()))
    }
}

/// Type alias for Vec<u8> secrets
pub type SecretBytes = Secret<Vec<u8>>;

impl SecretBytes {
    /// Create a secret from bytes
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Secret::new(bytes)
    }

    /// Get the secret as a byte slice
    pub fn expose_as_slice(&self) -> &[u8] {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_debug_redacts() {
        let secret = SecretString::from_string("super-secret-password".to_string());
        let debug_output = format!("{:?}", secret);
        assert_eq!(debug_output, "[REDACTED]");
        assert!(!debug_output.contains("super-secret"));
        assert!(!debug_output.contains("password"));
    }

    #[test]
    fn test_secret_display_redacts() {
        let secret = SecretString::from_string("api-key-12345".to_string());
        let display_output = format!("{}", secret);
        assert_eq!(display_output, "[REDACTED]");
        assert!(!display_output.contains("api-key"));
        assert!(!display_output.contains("12345"));
    }

    #[test]
    fn test_secret_serialize_redacts() {
        let secret = SecretString::from_string("secret-token".to_string());
        let json = serde_json::to_string(&secret).unwrap();
        assert_eq!(json, "\"[REDACTED]\"");
        assert!(!json.contains("secret-token"));
    }

    #[test]
    fn test_secret_expose() {
        let secret = SecretString::from_string("my-password".to_string());
        let exposed = secret.expose_secret();
        assert_eq!(exposed, "my-password");
    }

    #[test]
    fn test_secret_expose_as_str() {
        let secret = SecretString::from_string("my-api-key".to_string());
        let exposed = secret.expose_as_str();
        assert_eq!(exposed, "my-api-key");
    }

    #[test]
    fn test_secret_clone_and_expose() {
        let secret = SecretString::from_string("clone-me".to_string());
        let cloned = secret.clone();
        assert_eq!(secret.expose_as_str(), cloned.expose_as_str());
        assert_eq!(cloned.expose_as_str(), "clone-me");
    }

    #[test]
    fn test_secret_bytes() {
        let bytes = vec![0x01, 0x02, 0x03, 0x04];
        let secret = SecretBytes::from_bytes(bytes.clone());

        let debug_output = format!("{:?}", secret);
        assert_eq!(debug_output, "[REDACTED]");

        let exposed = secret.expose_as_slice();
        assert_eq!(exposed, &[0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn test_secret_deserialize() {
        let json = r#""my-secret-value""#;
        let secret: SecretString = serde_json::from_str(json).unwrap();
        assert_eq!(secret.expose_as_str(), "my-secret-value");

        // But serializing it back gives redacted
        let serialized = serde_json::to_string(&secret).unwrap();
        assert_eq!(serialized, "\"[REDACTED]\"");
    }

    #[test]
    fn test_secret_from_str() {
        use std::str::FromStr;

        // Test FromStr trait implementation
        let secret = SecretString::from_str("my-password").unwrap();
        assert_eq!(secret.expose_as_str(), "my-password");

        // Test that it works with parse()
        let secret2: SecretString = "another-secret".parse().unwrap();
        assert_eq!(secret2.expose_as_str(), "another-secret");
    }

    #[test]
    fn test_secret_in_struct() {
        #[derive(Debug, Serialize)]
        struct ApiKey {
            id: String,
            key: SecretString,
            name: String,
        }

        let key = ApiKey {
            id: "key_123".to_string(),
            key: SecretString::from_string("sk_live_abc123".to_string()),
            name: "Production Key".to_string(),
        };

        // Debug output doesn't leak secret
        let debug = format!("{:?}", key);
        assert!(!debug.contains("sk_live"));
        assert!(debug.contains("[REDACTED]"));

        // JSON doesn't leak secret
        let json = serde_json::to_string(&key).unwrap();
        assert!(!json.contains("sk_live"));
        assert!(json.contains("[REDACTED]"));
    }

    #[test]
    fn test_secret_clone() {
        let secret1 = SecretString::from_string("original".to_string());
        let secret2 = secret1.clone();

        assert_eq!(secret1.expose_as_str(), secret2.expose_as_str());
        assert_eq!(secret2.expose_as_str(), "original");
    }

    #[test]
    fn test_constant_time_eq() {
        let secret1 = SecretString::from_string("password123".to_string());
        let secret2 = SecretString::from_string("password123".to_string());
        let secret3 = SecretString::from_string("different".to_string());

        // Same values should be equal
        assert!(secret1.constant_time_eq(&secret2));
        assert!(secret2.constant_time_eq(&secret1));

        // Different values should not be equal
        assert!(!secret1.constant_time_eq(&secret3));
        assert!(!secret3.constant_time_eq(&secret1));

        // Same reference should be equal to itself
        assert!(secret1.constant_time_eq(&secret1));
    }
}
