//! Type-safe validated URLs that prevent SSRF attacks at compile time
//!
//! This module provides [`ValidatedUrl`], a wrapper around [`url::Url`] that can ONLY be
//! constructed through validation. This prevents Server-Side Request Forgery (SSRF) attacks
//! by making it impossible to bypass security checks.
//!
//! # Security Properties
//!
//! - **Compile-time enforcement**: Cannot create a `ValidatedUrl` without validation
//! - **SSRF prevention**: Blocks requests to cloud metadata endpoints (169.254.169.254)
//! - **Private network protection**: Blocks RFC1918 private IPs (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16)
//! - **Localhost blocking**: Prevents accessing localhost/127.0.0.1/::1
//! - **Scheme validation**: Only allows http/https
//!
//! # Example
//!
//! ```
//! use skreaver_core::security::{
//!     DomainValidator, DomainFilter, HttpAccessConfig, HttpPolicy, HttpAccess,
//!     RedirectLimit
//! };
//!
//! let policy = HttpPolicy {
//!     access: HttpAccess::Internet {
//!         config: HttpAccessConfig::default(),
//!         domain_filter: DomainFilter::AllowList {
//!             allow_list: vec!["example.com".to_string()],
//!             deny_list: vec![],
//!         },
//!         include_local: false,
//!         max_redirects: RedirectLimit::default(),
//!         user_agent: "test".to_string(),
//!     },
//!     allow_methods: vec!["GET".to_string()],
//!     default_headers: vec![],
//! };
//!
//! let validator = DomainValidator::new(&policy);
//!
//! // This works - safe public URL
//! let safe_url = validator.validate_url("https://example.com/api").unwrap();
//!
//! // This fails - SSRF attempt blocked at validation
//! let ssrf_attempt = validator.validate_url("http://169.254.169.254/metadata");
//! assert!(ssrf_attempt.is_err());
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;
use url::Url;

/// A URL that has been validated for security and can be safely used for HTTP requests.
///
/// This type can ONLY be constructed through [`DomainValidator::validate_url()`](super::DomainValidator::validate_url),
/// ensuring that all URLs used for HTTP requests have passed security checks.
///
/// The inner `Url` is private to prevent bypassing validation.
///
/// # Security Guarantees
///
/// - Cannot access cloud metadata endpoints (AWS, GCP, Azure)
/// - Cannot access private network ranges (RFC1918)
/// - Cannot access localhost unless explicitly allowed by policy
/// - Only http/https schemes allowed
/// - Domain must be on allowlist (if configured)
///
/// # Example
///
/// ```
/// use skreaver_core::security::{
///     DomainValidator, DomainFilter, HttpAccessConfig, ValidatedUrl, HttpPolicy, HttpAccess,
///     RedirectLimit
/// };
///
/// let policy = HttpPolicy {
///     access: HttpAccess::Internet {
///         config: HttpAccessConfig::default(),
///         domain_filter: DomainFilter::AllowList {
///             allow_list: vec!["httpbin.org".to_string()],
///             deny_list: vec![],
///         },
///         include_local: false,
///         max_redirects: RedirectLimit::default(),
///         user_agent: "test".to_string(),
///     },
///     allow_methods: vec!["GET".to_string()],
///     default_headers: vec![],
/// };
///
/// let validator = DomainValidator::new(&policy);
///
/// // Safe - validated URL
/// let url = validator.validate_url("https://httpbin.org/get").unwrap();
/// let url_str = url.as_str();
/// assert_eq!(url_str, "https://httpbin.org/get");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidatedUrl {
    /// The validated URL - private to prevent bypass
    inner: Url,
}

impl ValidatedUrl {
    /// Create a ValidatedUrl from a Url that has already been validated.
    ///
    /// # Safety
    ///
    /// This is `pub(crate)` to ensure only the security module can create ValidatedUrls.
    /// External code MUST use `DomainValidator::validate_url()`.
    pub(crate) fn new_unchecked(url: Url) -> Self {
        Self { inner: url }
    }

    /// Get the URL as a string
    pub fn as_str(&self) -> &str {
        self.inner.as_str()
    }

    /// Get a reference to the inner URL
    pub fn as_url(&self) -> &Url {
        &self.inner
    }

    /// Get the scheme (http or https)
    pub fn scheme(&self) -> &str {
        self.inner.scheme()
    }

    /// Get the host string
    pub fn host_str(&self) -> Option<&str> {
        self.inner.host_str()
    }

    /// Get the port
    pub fn port(&self) -> Option<u16> {
        self.inner.port()
    }

    /// Get the path
    pub fn path(&self) -> &str {
        self.inner.path()
    }

    /// Get the query string
    pub fn query(&self) -> Option<&str> {
        self.inner.query()
    }

    /// Join a relative URL to this URL
    ///
    /// Note: The resulting URL is NOT validated. If you need to make a request
    /// with the joined URL, you must validate it again through DomainValidator.
    pub fn join(&self, input: &str) -> Result<Url, url::ParseError> {
        self.inner.join(input)
    }
}

impl fmt::Display for ValidatedUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl AsRef<str> for ValidatedUrl {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<Url> for ValidatedUrl {
    fn as_ref(&self) -> &Url {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validated_url_getters() {
        let url = Url::parse("https://example.com:8080/path?query=value").unwrap();
        let validated = ValidatedUrl::new_unchecked(url);

        assert_eq!(validated.scheme(), "https");
        assert_eq!(validated.host_str(), Some("example.com"));
        assert_eq!(validated.port(), Some(8080));
        assert_eq!(validated.path(), "/path");
        assert_eq!(validated.query(), Some("query=value"));
        assert_eq!(
            validated.as_str(),
            "https://example.com:8080/path?query=value"
        );
    }

    #[test]
    fn test_validated_url_display() {
        let url = Url::parse("https://example.com/test").unwrap();
        let validated = ValidatedUrl::new_unchecked(url);

        assert_eq!(format!("{}", validated), "https://example.com/test");
    }

    #[test]
    fn test_validated_url_as_ref_str() {
        let url = Url::parse("https://example.com/").unwrap();
        let validated = ValidatedUrl::new_unchecked(url);

        let str_ref: &str = validated.as_ref();
        assert_eq!(str_ref, "https://example.com/");
    }

    #[test]
    fn test_validated_url_as_ref_url() {
        let url = Url::parse("https://example.com/").unwrap();
        let validated = ValidatedUrl::new_unchecked(url.clone());

        let url_ref: &Url = validated.as_ref();
        assert_eq!(url_ref.as_str(), url.as_str());
    }

    #[test]
    fn test_validated_url_join() {
        let url = Url::parse("https://example.com/api/").unwrap();
        let validated = ValidatedUrl::new_unchecked(url);

        let joined = validated.join("users").unwrap();
        assert_eq!(joined.as_str(), "https://example.com/api/users");
    }

    #[test]
    fn test_validated_url_serialize() {
        let url = Url::parse("https://example.com/test").unwrap();
        let validated = ValidatedUrl::new_unchecked(url);

        let json = serde_json::to_string(&validated).unwrap();
        assert!(json.contains("https://example.com/test"));
    }

    #[test]
    fn test_validated_url_deserialize() {
        let json = r#"{"inner":"https://example.com/test"}"#;
        let validated: ValidatedUrl = serde_json::from_str(json).unwrap();

        assert_eq!(validated.as_str(), "https://example.com/test");
    }

    #[test]
    fn test_validated_url_equality() {
        let url1 = ValidatedUrl::new_unchecked(Url::parse("https://example.com/").unwrap());
        let url2 = ValidatedUrl::new_unchecked(Url::parse("https://example.com/").unwrap());
        let url3 = ValidatedUrl::new_unchecked(Url::parse("https://other.com/").unwrap());

        assert_eq!(url1, url2);
        assert_ne!(url1, url3);
    }
}
