//! Input validation and sanitization

use super::errors::SecurityError;
use super::policy::{FileSystemPolicy, HttpPolicy, SecurityPolicy};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use url::Url;

/// Input validator for security checks
pub struct InputValidator {
    #[allow(dead_code)]
    policy: SecurityPolicy,
    secret_patterns: Vec<Regex>,
    suspicious_patterns: Vec<Regex>,
}

impl InputValidator {
    pub fn new(policy: &SecurityPolicy) -> Self {
        let secret_patterns = Self::compile_secret_patterns();
        let suspicious_patterns = Self::compile_suspicious_patterns();

        Self {
            policy: policy.clone(),
            secret_patterns,
            suspicious_patterns,
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
        let mut sanitized = input;

        // Remove or mask potential secrets
        for pattern in &self.secret_patterns {
            sanitized = pattern.replace_all(&sanitized, "[REDACTED]").to_string();
        }

        // Remove control characters
        sanitized = sanitized
            .chars()
            .filter(|c| !c.is_control() || c.is_whitespace())
            .collect();

        // Limit length
        if sanitized.len() > 10000 {
            sanitized.truncate(10000);
            sanitized.push_str("... [TRUNCATED]");
        }

        sanitized
    }

    fn check_for_secrets(&self, input: &str) -> Result<(), SecurityError> {
        for pattern in &self.secret_patterns {
            if pattern.is_match(input) {
                return Err(SecurityError::SecretInInput);
            }
        }
        Ok(())
    }

    fn check_for_suspicious_patterns(&self, input: &str) -> Result<(), SecurityError> {
        for pattern in &self.suspicious_patterns {
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

    fn compile_secret_patterns() -> Vec<Regex> {
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
    }

    fn compile_suspicious_patterns() -> Vec<Regex> {
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

        // Canonicalize path to resolve .. and symlinks
        let canonical_path = path_buf
            .canonicalize()
            .map_err(|e| SecurityError::InvalidPath {
                path: format!("{}: {}", path, e),
            })?;

        // Check if symlinks are allowed
        if !self.policy.follow_symlinks {
            let metadata =
                std::fs::symlink_metadata(&path_buf).map_err(|_| SecurityError::InvalidPath {
                    path: path.to_string(),
                })?;

            if metadata.file_type().is_symlink() {
                return Err(SecurityError::ValidationFailed {
                    reason: "Symbolic links are not allowed".to_string(),
                });
            }
        }

        // Check against allowed paths
        if !self.policy.is_path_allowed(&canonical_path)? {
            return Err(SecurityError::PathNotAllowed {
                path: canonical_path.to_string_lossy().to_string(),
            });
        }

        Ok(canonical_path)
    }

    pub fn validate_file_size(&self, path: &Path) -> Result<(), SecurityError> {
        let metadata = std::fs::metadata(path).map_err(|_| SecurityError::InvalidPath {
            path: path.to_string_lossy().to_string(),
        })?;

        let size = metadata.len();
        if size > self.policy.max_file_size_bytes {
            return Err(SecurityError::FileSizeLimitExceeded {
                size,
                limit: self.policy.max_file_size_bytes,
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

    pub fn validate_url(&self, url_str: &str) -> Result<Url, SecurityError> {
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
            return Err(SecurityError::DomainNotAllowed {
                domain: host.to_string(),
            });
        }

        // Check for local/private IPs if not allowed
        if !self.policy.allow_local {
            self.check_for_private_ip(host)?;
        }

        Ok(url)
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
        // Check for localhost
        if host == "localhost" || host == "127.0.0.1" || host == "::1" {
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
    secret_patterns: Vec<Regex>,
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
        let secret_patterns = InputValidator::compile_secret_patterns();
        let binary_patterns = vec![
            Regex::new(r"[\x00-\x08\x0B-\x0C\x0E-\x1F\x7F]").unwrap(), // Control characters
        ];

        Self {
            secret_patterns,
            binary_patterns,
        }
    }

    pub fn scan_content(&self, content: &[u8]) -> Result<ScanResult, SecurityError> {
        // Check if content is binary
        let is_binary = self.is_binary_content(content);

        if is_binary {
            return Ok(ScanResult {
                is_safe: true, // Allow binary files but don't scan content
                issues: vec!["Binary file detected".to_string()],
                redacted_content: None,
            });
        }

        // Convert to string for text scanning
        let text = String::from_utf8_lossy(content);
        let mut issues = Vec::new();

        // Check for secrets
        for pattern in &self.secret_patterns {
            if pattern.is_match(&text) {
                issues.push("Potential secret detected".to_string());
                break;
            }
        }

        // Create redacted version
        let mut redacted = text.to_string();
        for pattern in &self.secret_patterns {
            redacted = pattern.replace_all(&redacted, "[REDACTED]").to_string();
        }

        Ok(ScanResult {
            is_safe: issues.is_empty(),
            issues,
            redacted_content: Some(redacted),
        })
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub is_safe: bool,
    pub issues: Vec<String>,
    pub redacted_content: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

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
            allow_domains: vec!["*.example.com".to_string()],
            allow_local: false,
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
        assert!(result.is_safe);

        // Test content with potential secret
        let secret_content = b"api_key=abc123def456ghi789";
        let result = scanner.scan_content(secret_content).unwrap();
        assert!(!result.is_safe);
        assert!(!result.issues.is_empty());
    }
}
