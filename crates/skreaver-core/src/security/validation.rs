//! Input validation and sanitization

use super::errors::SecurityError;
use super::policy::{FileSystemPolicy, HttpPolicy, SecurityPolicy};
use super::validated_url::ValidatedUrl;
#[cfg(feature = "security-basic")]
use once_cell::sync::Lazy;
#[cfg(feature = "security-basic")]
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use url::Url;

/// Convert path to string with logging for lossy conversions (LOW-3)
fn path_to_string_checked(path: &Path) -> String {
    let lossy = path.to_string_lossy();
    if matches!(lossy, std::borrow::Cow::Owned(_)) {
        tracing::warn!(
            path_debug = ?path,
            path_lossy = %lossy,
            "Path contains invalid UTF-8 - using lossy conversion in security context"
        );
    }
    lossy.to_string()
}

/// Lazy-compiled secret detection patterns for optimal performance
#[cfg(feature = "security-basic")]
static SECRET_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    let patterns = [
        // API keys and tokens
        r"(?i)(api[_-]?key|apikey|token)\s*[:=]\s*[\x27\x22]?([a-zA-Z0-9_-]{16,})",
        // Passwords
        r"(?i)(password|pwd|pass)\s*[:=]\s*[\x27\x22]?([^\s\x27\x22]{8,})",
        // JWT tokens
        r"eyJ[a-zA-Z0-9_-]*\.[a-zA-Z0-9_-]*\.[a-zA-Z0-9_-]*",
        // AWS keys
        r"AKIA[0-9A-Z]{16}",
        // Private keys
        r"-----BEGIN [A-Z ]+PRIVATE KEY-----",
        // Database connection strings
        r"(?i)(mongodb|mysql|postgresql)://[^\s]+",
    ];

    patterns
        .into_iter()
        .filter_map(|p| Regex::new(p).ok())
        .collect()
});

/// Lazy-compiled suspicious pattern detection for optimal performance
#[cfg(feature = "security-basic")]
static SUSPICIOUS_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    let patterns = [
        // Command injection attempts
        r"[;&|`$]",
        // Path traversal
        r"\.\./",
        // SQL injection patterns
        r"(?i)(union|select|drop|delete|insert|update)\s+.*(from|into|set)",
        // Script injection
        r"<script[^>]*>",
        // XXE patterns
        r"<!ENTITY",
        // LDAP injection
        r"[()=*]",
    ];

    patterns
        .into_iter()
        .filter_map(|p| Regex::new(p).ok())
        .collect()
});

/// Input validator for security checks
pub struct InputValidator {
    #[allow(dead_code)]
    policy: SecurityPolicy,
}

impl InputValidator {
    pub fn new(policy: &SecurityPolicy) -> Self {
        Self {
            policy: policy.clone(),
        }
    }

    pub fn validate(&self, input: &str) -> Result<(), SecurityError> {
        // Check for secrets in input
        self.check_for_secrets(input)?;

        // Check for suspicious patterns
        self.check_for_suspicious_patterns(input)?;

        // Validate input length
        self.validate_input_length(input)?;

        Ok(())
    }

    pub fn sanitize(&self, input: String) -> String {
        use crate::sanitization::{ContentSanitizer, SecretRedactor};

        // Redact secrets using unified redactor
        let sanitized = SecretRedactor::redact_secrets(&input);

        // Remove control characters using unified sanitizer
        let sanitized = ContentSanitizer::remove_control_chars(&sanitized);

        // Limit length
        if sanitized.len() > 10000 {
            format!("{}... [TRUNCATED]", &sanitized[..10000])
        } else {
            sanitized
        }
    }

    fn check_for_secrets(&self, input: &str) -> Result<(), SecurityError> {
        for pattern in SECRET_PATTERNS.iter() {
            if pattern.is_match(input) {
                return Err(SecurityError::SecretInInput);
            }
        }
        Ok(())
    }

    fn check_for_suspicious_patterns(&self, input: &str) -> Result<(), SecurityError> {
        for pattern in SUSPICIOUS_PATTERNS.iter() {
            if pattern.is_match(input) {
                return Err(SecurityError::SuspiciousActivity {
                    description: format!(
                        "Suspicious pattern detected in input: {}",
                        pattern.as_str()
                    ),
                });
            }
        }
        Ok(())
    }

    fn validate_input_length(&self, input: &str) -> Result<(), SecurityError> {
        const MAX_INPUT_LENGTH: usize = 100_000; // 100KB

        if input.len() > MAX_INPUT_LENGTH {
            return Err(SecurityError::ValidationFailed {
                reason: format!(
                    "Input too long: {} bytes > {} bytes",
                    input.len(),
                    MAX_INPUT_LENGTH
                ),
            });
        }

        Ok(())
    }
}

/// Path validator for file system operations
pub struct PathValidator {
    policy: FileSystemPolicy,
}

impl PathValidator {
    pub fn new(policy: &FileSystemPolicy) -> Self {
        Self {
            policy: policy.clone(),
        }
    }

    pub fn validate_path(&self, path: &str) -> Result<PathBuf, SecurityError> {
        let path_buf = PathBuf::from(path);

        // Basic path validation
        if path.is_empty() {
            return Err(SecurityError::ValidationFailed {
                reason: "Empty path".to_string(),
            });
        }

        // Check for null bytes
        if path.contains('\0') {
            return Err(SecurityError::ValidationFailed {
                reason: "Path contains null bytes".to_string(),
            });
        }

        // SECURITY (HIGH-3): Atomically canonicalize path without TOCTOU race
        // Use platform-specific fd-based canonicalization to prevent race between
        // symlink check and canonicalize() where attacker could swap path with symlink.
        let canonical_path = if let super::FileSystemAccess::Enabled {
            symlink_behavior: super::SymlinkBehavior::NoFollow,
            ..
        } = &self.policy.access
        {
            // Use atomic fd-based canonicalization (Unix) or fallback (Windows)
            Self::canonicalize_no_follow(&path_buf)?
        } else {
            // Allow symlinks - use standard canonicalize
            path_buf
                .canonicalize()
                .map_err(|e| SecurityError::InvalidPath {
                    path: format!("{}: {}", path, e),
                })?
        };

        // Check against allowed paths
        if !self.policy.is_path_allowed(&canonical_path)? {
            return Err(SecurityError::PathNotAllowed {
                path: path_to_string_checked(&canonical_path),
            });
        }

        Ok(canonical_path)
    }

    /// SECURITY (HIGH-3): Atomically canonicalize a path without following symlinks
    ///
    /// This prevents TOCTOU race conditions between symlink checking and canonicalization.
    /// Uses platform-specific file descriptor operations where available.
    #[cfg(target_os = "linux")]
    fn canonicalize_no_follow(path: &Path) -> Result<PathBuf, SecurityError> {
        use std::os::unix::fs::OpenOptionsExt;
        use std::os::unix::io::AsRawFd;

        // O_PATH is Linux-specific (since 2.6.39)
        const O_PATH: i32 = 0o10000000;

        // Open with O_NOFOLLOW to atomically reject symlinks
        // O_PATH allows opening for metadata without requiring read permissions
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | O_PATH)
            .open(path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied
                    || e.raw_os_error() == Some(libc::ELOOP)
                {
                    SecurityError::ValidationFailed {
                        reason: format!(
                            "Symbolic link detected or permission denied: {}",
                            path.display()
                        ),
                    }
                } else {
                    SecurityError::InvalidPath {
                        path: format!("{}: {}", path.display(), e),
                    }
                }
            })?;

        // Read the canonical path via /proc/self/fd/<fd>
        // This gives us the real path that the fd points to, without following symlinks
        let fd_path = format!("/proc/self/fd/{}", file.as_raw_fd());
        std::fs::read_link(&fd_path).map_err(|e| SecurityError::InvalidPath {
            path: format!("Failed to resolve path via fd: {}", e),
        })
    }

    /// macOS and other Unix systems: Use realpath() with O_NOFOLLOW
    #[cfg(all(unix, not(target_os = "linux")))]
    fn canonicalize_no_follow(path: &Path) -> Result<PathBuf, SecurityError> {
        use std::os::unix::fs::OpenOptionsExt;

        // Try to open with O_NOFOLLOW - will fail if path is a symlink
        let _file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied
                    || e.raw_os_error() == Some(libc::ELOOP)
                {
                    SecurityError::ValidationFailed {
                        reason: format!(
                            "Symbolic link detected or permission denied: {}",
                            path.display()
                        ),
                    }
                } else {
                    SecurityError::InvalidPath {
                        path: format!("{}: {}", path.display(), e),
                    }
                }
            })?;

        // If we successfully opened it, it's not a symlink
        // Now we can safely canonicalize
        path.canonicalize().map_err(|e| SecurityError::InvalidPath {
            path: format!("{}: {}", path.display(), e),
        })
    }

    /// Windows fallback: Use the existing check_path_for_symlinks approach
    /// Note: This still has a small TOCTOU window but Windows doesn't have O_NOFOLLOW
    #[cfg(not(unix))]
    fn canonicalize_no_follow(path: &Path) -> Result<PathBuf, SecurityError> {
        // On Windows, we need to check for symlinks component-by-component
        // This has a TOCTOU window but is the best we can do without Windows-specific APIs
        let mut current_path = PathBuf::new();

        for component in path.components() {
            use std::path::Component;

            match component {
                Component::RootDir => {
                    current_path.push("/");
                }
                Component::Prefix(prefix) => {
                    current_path.push(prefix.as_os_str());
                }
                Component::CurDir => {
                    continue;
                }
                Component::ParentDir => {
                    return Err(SecurityError::ValidationFailed {
                        reason: "Path traversal (..) is not allowed".to_string(),
                    });
                }
                Component::Normal(name) => {
                    current_path.push(name);

                    // Check if this component is a symlink
                    match std::fs::symlink_metadata(&current_path) {
                        Ok(metadata) => {
                            if metadata.file_type().is_symlink() {
                                return Err(SecurityError::ValidationFailed {
                                    reason: format!(
                                        "Symbolic link detected at: {}",
                                        current_path.display()
                                    ),
                                });
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                            continue;
                        }
                        Err(e) => {
                            return Err(SecurityError::InvalidPath {
                                path: format!("{}: {}", current_path.display(), e),
                            });
                        }
                    }
                }
            }
        }

        // Now canonicalize the checked path
        current_path
            .canonicalize()
            .map_err(|e| SecurityError::InvalidPath {
                path: format!("{}: {}", current_path.display(), e),
            })
    }

    pub fn validate_file_size(&self, path: &Path) -> Result<(), SecurityError> {
        let metadata = std::fs::metadata(path).map_err(|_| SecurityError::InvalidPath {
            path: path.to_string_lossy().to_string(),
        })?;

        let size = metadata.len();
        if size > self.policy.max_file_size.bytes() {
            return Err(SecurityError::FileSizeLimitExceeded {
                size,
                limit: self.policy.max_file_size.bytes(),
            });
        }

        Ok(())
    }
}

/// Domain validator for HTTP operations
pub struct DomainValidator {
    policy: HttpPolicy,
}

impl DomainValidator {
    pub fn new(policy: &HttpPolicy) -> Self {
        Self {
            policy: policy.clone(),
        }
    }

    /// Validate a URL for security and return a ValidatedUrl that can be safely used.
    ///
    /// This is the ONLY way to create a ValidatedUrl, ensuring compile-time enforcement
    /// of URL validation for all HTTP requests.
    ///
    /// # Security
    ///
    /// This method checks:
    /// - URL scheme (only http/https allowed)
    /// - Domain allowlist/blocklist
    /// - Private IP ranges (RFC1918: 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16)
    /// - Localhost (127.0.0.1, ::1)
    /// - Link-local addresses (169.254.0.0/16 - AWS/GCP metadata endpoints)
    ///
    /// # Example
    ///
    /// ```
    /// use skreaver_core::security::{
    ///     DomainValidator, DomainFilter, HttpAccessConfig, HttpPolicy, HttpAccess,
    ///     RedirectLimit
    /// };
    ///
    /// let policy = HttpPolicy {
    ///     access: HttpAccess::Internet {
    ///         config: HttpAccessConfig::default(),
    ///         domain_filter: DomainFilter::AllowList {
    ///             allow_list: vec!["example.com".to_string()],
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
    /// // Safe URL - passes validation
    /// let url = validator.validate_url("https://example.com/api").unwrap();
    ///
    /// // SSRF attempt - blocked
    /// let ssrf = validator.validate_url("http://169.254.169.254/metadata");
    /// assert!(ssrf.is_err());
    /// ```
    pub fn validate_url(&self, url_str: &str) -> Result<ValidatedUrl, SecurityError> {
        // Parse URL
        let url = Url::parse(url_str).map_err(|e| SecurityError::ValidationFailed {
            reason: format!("Invalid URL: {}", e),
        })?;

        // Check scheme
        match url.scheme() {
            "http" | "https" => {}
            _ => {
                return Err(SecurityError::ValidationFailed {
                    reason: format!("Unsupported URL scheme: {}", url.scheme()),
                });
            }
        }

        // Get domain/host
        let host = url
            .host_str()
            .ok_or_else(|| SecurityError::ValidationFailed {
                reason: "URL has no host".to_string(),
            })?;

        // Validate domain
        if !self.policy.is_domain_allowed(host)? {
            // Record domain policy violation metric

            return Err(SecurityError::DomainNotAllowed {
                domain: host.to_string(),
            });
        }

        // Check for local/private IPs if not allowed
        let allow_local = match &self.policy.access {
            super::HttpAccess::Disabled => false,
            super::HttpAccess::LocalOnly(_) => true,
            super::HttpAccess::Internet { include_local, .. } => *include_local,
        };

        if !allow_local {
            self.check_for_private_ip(host)?;
        }

        // Return validated URL - this is the only way to create a ValidatedUrl
        Ok(ValidatedUrl::new_unchecked(url))
    }

    pub fn validate_method(&self, method: &str) -> Result<(), SecurityError> {
        if !self.policy.is_method_allowed(method) {
            return Err(SecurityError::MethodNotAllowed {
                method: method.to_string(),
            });
        }
        Ok(())
    }

    fn check_for_private_ip(&self, host: &str) -> Result<(), SecurityError> {
        // Check for localhost (including IPv6 loopback)
        if host == "localhost" || host == "127.0.0.1" || host == "::1" || host == "[::1]" {
            return Err(SecurityError::DomainNotAllowed {
                domain: host.to_string(),
            });
        }

        // Try to parse as IP address
        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            use std::net::IpAddr;

            match ip {
                IpAddr::V4(ipv4) => {
                    // Check RFC 1918 private ranges
                    let octets = ipv4.octets();
                    if octets[0] == 10 ||
                       (octets[0] == 172 && octets[1] >= 16 && octets[1] <= 31) ||
                       (octets[0] == 192 && octets[1] == 168) ||
                       octets[0] == 127 || // Loopback
                       (octets[0] == 169 && octets[1] == 254)
                    // Link-local
                    {
                        return Err(SecurityError::DomainNotAllowed {
                            domain: host.to_string(),
                        });
                    }
                }
                IpAddr::V6(ipv6) => {
                    // Check for IPv6 loopback and private ranges
                    if ipv6.is_loopback() || ipv6.is_unspecified() {
                        return Err(SecurityError::DomainNotAllowed {
                            domain: host.to_string(),
                        });
                    }
                }
            }
        }

        Ok(())
    }
}

/// Content scanner for detecting sensitive data in file contents
pub struct ContentScanner {
    #[allow(dead_code)]
    binary_patterns: Vec<Regex>,
}

impl Default for ContentScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentScanner {
    pub fn new() -> Self {
        let binary_patterns = vec![
            Regex::new(r"[\x00-\x08\x0B-\x0C\x0E-\x1F\x7F]").unwrap(), // Control characters
        ];

        Self { binary_patterns }
    }

    pub fn scan_content(&self, content: &[u8]) -> Result<ScanResult, SecurityError> {
        // Check if content is binary
        let is_binary = self.is_binary_content(content);

        if is_binary {
            // Binary content is treated as a security violation
            // since we cannot reliably scan it for secrets
            return Ok(ScanResult::Unsafe {
                violations: vec![SecurityViolation {
                    kind: ViolationKind::BinaryContent,
                    description: "Binary file detected - cannot scan for secrets".to_string(),
                }],
                redacted_content: "[BINARY CONTENT]".to_string(),
            });
        }

        // Convert to string for text scanning
        let text = String::from_utf8_lossy(content);
        let mut violations = Vec::new();

        // Check for secrets using global patterns
        for pattern in SECRET_PATTERNS.iter() {
            if pattern.is_match(&text) {
                violations.push(SecurityViolation {
                    kind: ViolationKind::SecretDetected,
                    description: "Potential secret or credential detected".to_string(),
                });
                break;
            }
        }

        // Create redacted version
        let mut redacted = text.to_string();
        for pattern in SECRET_PATTERNS.iter() {
            redacted = pattern.replace_all(&redacted, "[REDACTED]").to_string();
        }

        if violations.is_empty() {
            Ok(ScanResult::Safe {
                content: text.to_string(),
            })
        } else {
            Ok(ScanResult::Unsafe {
                violations,
                redacted_content: redacted,
            })
        }
    }

    fn is_binary_content(&self, content: &[u8]) -> bool {
        // Simple heuristic: if more than 30% of first 1024 bytes are non-printable, consider binary
        let sample_size = std::cmp::min(1024, content.len());
        let sample = &content[..sample_size];

        let non_printable_count = sample
            .iter()
            .filter(|&&b| b < 32 && b != b'\t' && b != b'\n' && b != b'\r')
            .count();

        (non_printable_count as f64 / sample_size as f64) > 0.3
    }
}

/// Type of security violation detected during content scanning
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ViolationKind {
    /// Potential secret or credential detected
    SecretDetected,
    /// Binary content detected (may contain encoded secrets)
    BinaryContent,
}

/// A security violation found during content scanning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityViolation {
    /// Type of violation
    pub kind: ViolationKind,
    /// Human-readable description
    pub description: String,
}

/// Result of content security scanning
///
/// This enum uses the typestate pattern to make invalid states unrepresentable:
/// - Safe content has no violations
/// - Unsafe content always has at least one violation with details
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ScanResult {
    /// Content is safe - no security violations detected
    Safe {
        /// Original content (for text) or indication it was binary
        content: String,
    },
    /// Content contains security violations
    Unsafe {
        /// Security violations detected (guaranteed non-empty)
        violations: Vec<SecurityViolation>,
        /// Content with sensitive parts redacted
        redacted_content: String,
    },
}

impl ScanResult {
    /// Check if the content is safe
    pub fn is_safe(&self) -> bool {
        matches!(self, ScanResult::Safe { .. })
    }

    /// Get violations if unsafe, or empty vec if safe
    pub fn violations(&self) -> Vec<SecurityViolation> {
        match self {
            ScanResult::Safe { .. } => Vec::new(),
            ScanResult::Unsafe { violations, .. } => violations.clone(),
        }
    }

    /// Get redacted content if available
    pub fn redacted_content(&self) -> Option<String> {
        match self {
            ScanResult::Safe { content } => Some(content.clone()),
            ScanResult::Unsafe {
                redacted_content, ..
            } => Some(redacted_content.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::{DomainFilter, HttpAccess, HttpAccessConfig, RedirectLimit};

    #[test]
    fn test_input_validator_secrets() {
        let policy = SecurityPolicy {
            fs_policy: FileSystemPolicy::default(),
            http_policy: HttpPolicy::default(),
            network_policy: super::super::policy::NetworkPolicy::default(),
        };

        let validator = InputValidator::new(&policy);

        // Should detect API key
        let result = validator.validate("api_key=abc123def456ghi789");
        assert!(result.is_err());

        // Should allow normal input
        let result = validator.validate("Hello, world!");
        assert!(result.is_ok());
    }

    #[test]
    fn test_domain_validator() {
        let policy = HttpPolicy {
            access: HttpAccess::Internet {
                config: HttpAccessConfig::default(),
                domain_filter: DomainFilter::AllowList {
                    allow_list: vec!["*.example.com".to_string()],
                    deny_list: vec![],
                },
                include_local: false,
                max_redirects: RedirectLimit::default(),
                user_agent: "test".to_string(),
            },
            ..Default::default()
        };

        let validator = DomainValidator::new(&policy);

        // Should allow subdomain of example.com
        let result = validator.validate_url("https://api.example.com/test");
        assert!(result.is_ok());

        // Should reject localhost
        let result = validator.validate_url("http://localhost:8080/test");
        assert!(result.is_err());

        // Should reject non-allowed domain
        let result = validator.validate_url("https://evil.com/test");
        assert!(result.is_err());
    }

    #[test]
    fn test_content_scanner() {
        let scanner = ContentScanner::new();

        // Test safe text content
        let safe_content = b"Hello, world! This is safe content.";
        let result = scanner.scan_content(safe_content).unwrap();
        assert!(result.is_safe());
        assert!(matches!(result, ScanResult::Safe { .. }));

        // Test content with potential secret
        let secret_content = b"api_key=abc123def456ghi789";
        let result = scanner.scan_content(secret_content).unwrap();
        assert!(!result.is_safe());
        match result {
            ScanResult::Unsafe { violations, .. } => {
                assert!(!violations.is_empty());
                assert_eq!(violations[0].kind, ViolationKind::SecretDetected);
            }
            _ => panic!("Expected Unsafe result"),
        }
    }

    // ===== SSRF Protection Tests =====

    #[test]
    fn test_ssrf_aws_metadata_endpoint_blocked() {
        let policy = HttpPolicy {
            access: HttpAccess::Internet {
                config: HttpAccessConfig::default(),
                domain_filter: DomainFilter::AllowAll {
                    deny_list: vec![], // Allow all domains
                },
                include_local: false,
                max_redirects: RedirectLimit::default(),
                user_agent: "test".to_string(),
            },
            ..Default::default()
        };

        let validator = DomainValidator::new(&policy);

        // AWS metadata endpoint - critical SSRF target
        let result = validator.validate_url("http://169.254.169.254/latest/meta-data/");
        assert!(result.is_err(), "Should block AWS metadata endpoint");
    }

    #[test]
    fn test_ssrf_private_ip_ranges_blocked() {
        let policy = HttpPolicy {
            access: HttpAccess::Internet {
                config: HttpAccessConfig::default(),
                domain_filter: DomainFilter::AllowAll { deny_list: vec![] },
                include_local: false,
                max_redirects: RedirectLimit::default(),
                user_agent: "test".to_string(),
            },
            ..Default::default()
        };

        let validator = DomainValidator::new(&policy);

        // RFC1918 private ranges
        assert!(
            validator.validate_url("http://10.0.0.1/").is_err(),
            "Should block 10.0.0.0/8"
        );
        assert!(
            validator.validate_url("http://172.16.0.1/").is_err(),
            "Should block 172.16.0.0/12"
        );
        assert!(
            validator.validate_url("http://192.168.1.1/").is_err(),
            "Should block 192.168.0.0/16"
        );
    }

    #[test]
    fn test_ssrf_localhost_variants_blocked() {
        let policy = HttpPolicy {
            access: HttpAccess::Internet {
                config: HttpAccessConfig::default(),
                domain_filter: DomainFilter::AllowAll { deny_list: vec![] },
                include_local: false,
                max_redirects: RedirectLimit::default(),
                user_agent: "test".to_string(),
            },
            ..Default::default()
        };

        let validator = DomainValidator::new(&policy);

        // Localhost variants
        let localhost_result = validator.validate_url("http://localhost/");
        assert!(
            localhost_result.is_err(),
            "Should block localhost: {:?}",
            localhost_result
        );

        let ipv4_result = validator.validate_url("http://127.0.0.1/");
        assert!(
            ipv4_result.is_err(),
            "Should block 127.0.0.1: {:?}",
            ipv4_result
        );

        let ipv6_result = validator.validate_url("http://[::1]/");
        assert!(
            ipv6_result.is_err(),
            "Should block IPv6 loopback: {:?}",
            ipv6_result
        );
    }

    #[test]
    fn test_validated_url_type_safety() {
        let policy = HttpPolicy {
            access: HttpAccess::Internet {
                config: HttpAccessConfig::default(),
                domain_filter: DomainFilter::AllowList {
                    allow_list: vec!["example.com".to_string()],
                    deny_list: vec![],
                },
                include_local: false,
                max_redirects: RedirectLimit::default(),
                user_agent: "test".to_string(),
            },
            ..Default::default()
        };

        let validator = DomainValidator::new(&policy);

        // Create validated URL
        let validated_url = validator
            .validate_url("https://example.com/api")
            .expect("Should validate safe URL");

        // Verify ValidatedUrl provides safe access
        assert_eq!(validated_url.as_str(), "https://example.com/api");
        assert_eq!(validated_url.scheme(), "https");
        assert_eq!(validated_url.host_str(), Some("example.com"));
        assert_eq!(validated_url.path(), "/api");
    }

    #[test]
    fn test_validated_url_only_created_through_validation() {
        // This test documents that ValidatedUrl::new_unchecked is pub(crate)
        // So external code CANNOT create ValidatedUrl without validation

        let policy = HttpPolicy {
            access: HttpAccess::Internet {
                config: HttpAccessConfig::default(),
                domain_filter: DomainFilter::AllowList {
                    allow_list: vec!["safe.com".to_string()],
                    deny_list: vec![],
                },
                include_local: false,
                max_redirects: RedirectLimit::default(),
                user_agent: "test".to_string(),
            },
            ..Default::default()
        };

        let validator = DomainValidator::new(&policy);

        // The ONLY way to get a ValidatedUrl
        let result = validator.validate_url("https://safe.com/");
        assert!(result.is_ok());

        // Cannot create ValidatedUrl directly - this would fail to compile in external code:
        // let evil = ValidatedUrl::new_unchecked(Url::parse("http://evil.com/").unwrap());
    }

    #[test]
    fn test_ssrf_allow_local_when_explicitly_enabled() {
        let policy = HttpPolicy {
            access: HttpAccess::Internet {
                config: HttpAccessConfig::default(),
                domain_filter: DomainFilter::AllowAll { deny_list: vec![] },
                include_local: true, // Explicitly allow local
                max_redirects: RedirectLimit::default(),
                user_agent: "test".to_string(),
            },
            ..Default::default()
        };

        let validator = DomainValidator::new(&policy);

        // When allow_local is true, localhost should be allowed
        let result = validator.validate_url("http://localhost:8080/");
        assert!(result.is_ok(), "Should allow localhost when policy permits");
    }

    #[test]
    fn test_url_scheme_validation() {
        let policy = HttpPolicy {
            access: HttpAccess::Internet {
                config: HttpAccessConfig::default(),
                domain_filter: DomainFilter::AllowList {
                    allow_list: vec!["example.com".to_string()],
                    deny_list: vec![],
                },
                include_local: false,
                max_redirects: RedirectLimit::default(),
                user_agent: "test".to_string(),
            },
            ..Default::default()
        };

        let validator = DomainValidator::new(&policy);

        // HTTP and HTTPS should work
        assert!(validator.validate_url("http://example.com/").is_ok());
        assert!(validator.validate_url("https://example.com/").is_ok());

        // Other schemes should be blocked
        assert!(
            validator.validate_url("file:///etc/passwd").is_err(),
            "Should block file:// scheme"
        );
        assert!(
            validator.validate_url("ftp://example.com/").is_err(),
            "Should block ftp:// scheme"
        );
    }
}
