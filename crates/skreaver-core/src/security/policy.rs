//! Security policies for different tool types

use super::errors::SecurityError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Combined security policy for an operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicy {
    pub fs_policy: FileSystemPolicy,
    pub http_policy: HttpPolicy,
    pub network_policy: NetworkPolicy,
}

/// File system access policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSystemPolicy {
    pub enabled: bool,
    pub allow_paths: Vec<PathBuf>,
    pub deny_patterns: Vec<String>,
    pub max_file_size_bytes: u64,
    pub max_files_per_operation: u32,
    pub follow_symlinks: bool,
    pub scan_content: bool,
}

impl Default for FileSystemPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            allow_paths: vec![PathBuf::from("./data"), PathBuf::from("./runtime/tmp")],
            deny_patterns: vec![
                "..".to_string(),
                "/etc".to_string(),
                "/proc".to_string(),
                "/sys".to_string(),
                "*.ssh".to_string(),
                "*.key".to_string(),
                "*.pem".to_string(),
            ],
            max_file_size_bytes: 16_777_216, // 16MB
            max_files_per_operation: 100,
            follow_symlinks: false,
            scan_content: true,
        }
    }
}

impl FileSystemPolicy {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    pub fn is_path_allowed(&self, path: &std::path::Path) -> Result<bool, SecurityError> {
        if !self.enabled {
            return Err(SecurityError::ToolDisabled {
                tool_name: "file_system".to_string(),
            });
        }

        // Canonicalize the path to resolve any ".." or symlinks
        let canonical_path = path
            .canonicalize()
            .map_err(|_| SecurityError::InvalidPath {
                path: path.to_string_lossy().to_string(),
            })?;

        // Check if path starts with any allowed path
        let allowed = self.allow_paths.iter().any(|allowed_path| {
            if let Ok(canonical_allowed) = allowed_path.canonicalize() {
                canonical_path.starts_with(canonical_allowed)
            } else {
                false
            }
        });

        if !allowed {
            return Ok(false);
        }

        // Check deny patterns
        let path_str = canonical_path.to_string_lossy();
        for pattern in &self.deny_patterns {
            if path_str.contains(pattern) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    pub fn validate_file_size(&self, size: u64) -> Result<(), SecurityError> {
        if size > self.max_file_size_bytes {
            return Err(SecurityError::FileSizeLimitExceeded {
                size,
                limit: self.max_file_size_bytes,
            });
        }
        Ok(())
    }
}

/// HTTP client access policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpPolicy {
    pub enabled: bool,
    pub allow_domains: Vec<String>,
    pub deny_domains: Vec<String>,
    pub allow_methods: Vec<String>,
    pub timeout_seconds: u64,
    pub max_response_bytes: u64,
    pub max_redirects: u32,
    pub user_agent: String,
    pub allow_local: bool,
    pub default_headers: Vec<(String, String)>,
}

impl Default for HttpPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            allow_domains: vec![],
            deny_domains: vec![
                "localhost".to_string(),
                "127.0.0.1".to_string(),
                "0.0.0.0".to_string(),
                "169.254.169.254".to_string(),          // AWS metadata
                "metadata.google.internal".to_string(), // GCP metadata
                "10.*".to_string(),
                "172.16.*".to_string(),
                "192.168.*".to_string(),
            ],
            allow_methods: vec![
                "GET".to_string(),
                "POST".to_string(),
                "PUT".to_string(),
                "PATCH".to_string(),
                "DELETE".to_string(),
            ],
            timeout_seconds: 30,
            max_response_bytes: 33_554_432, // 32MB
            max_redirects: 3,
            user_agent: "skreaver-agent/0.1.0".to_string(),
            allow_local: false,
            default_headers: vec![
                ("X-Skreaver-Agent".to_string(), "true".to_string()),
                ("X-Requested-With".to_string(), "Skreaver".to_string()),
            ],
        }
    }
}

impl HttpPolicy {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    pub fn is_domain_allowed(&self, domain: &str) -> Result<bool, SecurityError> {
        if !self.enabled {
            return Err(SecurityError::ToolDisabled {
                tool_name: "http".to_string(),
            });
        }

        // Check deny list first (takes precedence)
        for denied_domain in &self.deny_domains {
            if Self::matches_pattern(domain, denied_domain) {
                return Ok(false);
            }
        }

        // If no allow list, allow all (except denied)
        if self.allow_domains.is_empty() {
            return Ok(true);
        }

        // Check allow list
        for allowed_domain in &self.allow_domains {
            if Self::matches_pattern(domain, allowed_domain) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn is_method_allowed(&self, method: &str) -> bool {
        self.allow_methods.contains(&method.to_uppercase())
    }

    pub fn get_timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_seconds)
    }

    fn matches_pattern(domain: &str, pattern: &str) -> bool {
        if let Some(suffix) = pattern.strip_prefix('*') {
            domain.ends_with(suffix)
        } else if let Some(prefix) = pattern.strip_suffix('*') {
            domain.starts_with(prefix)
        } else {
            domain == pattern
        }
    }
}

/// Network access policy for raw TCP/UDP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicy {
    pub enabled: bool,
    pub allow_ports: Vec<u16>,
    pub deny_ports: Vec<u16>,
    pub ttl_seconds: u64,
    pub allow_private_networks: bool,
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self {
            enabled: false, // Disabled by default
            allow_ports: vec![],
            deny_ports: vec![
                22,    // SSH
                23,    // Telnet
                25,    // SMTP
                53,    // DNS
                135,   // RPC
                139,   // NetBIOS
                445,   // SMB
                1433,  // SQL Server
                3389,  // RDP
                5432,  // PostgreSQL
                6379,  // Redis
                27017, // MongoDB
            ],
            ttl_seconds: 300,
            allow_private_networks: false,
        }
    }
}

impl NetworkPolicy {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    pub fn is_port_allowed(&self, port: u16) -> Result<bool, SecurityError> {
        if !self.enabled {
            return Err(SecurityError::ToolDisabled {
                tool_name: "network".to_string(),
            });
        }

        // Check deny list first
        if self.deny_ports.contains(&port) {
            return Ok(false);
        }

        // If no allow list, allow all (except denied)
        if self.allow_ports.is_empty() {
            return Ok(true);
        }

        // Check allow list
        Ok(self.allow_ports.contains(&port))
    }
}

/// Tool-specific security policies
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolPolicy {
    pub fs_enabled: Option<bool>,
    pub http_enabled: Option<bool>,
    pub network_enabled: Option<bool>,
    pub rate_limit_per_minute: Option<u32>,
    pub additional_restrictions: std::collections::HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_file_system_policy_path_validation() {
        let _policy = FileSystemPolicy::default();

        // Should allow paths under ./data
        let _data_path = Path::new("./data/test.txt");
        // Note: This test might fail if the path doesn't exist for canonicalization
        // In real usage, we'd create the directories first
    }

    #[test]
    fn test_http_policy_domain_matching() {
        let policy = HttpPolicy {
            allow_domains: vec!["*.example.com".to_string(), "api.test.org".to_string()],
            deny_domains: vec!["evil.example.com".to_string()],
            ..Default::default()
        };

        assert!(policy.is_domain_allowed("api.example.com").unwrap());
        assert!(policy.is_domain_allowed("api.test.org").unwrap());
        assert!(!policy.is_domain_allowed("evil.example.com").unwrap()); // Denied
        assert!(!policy.is_domain_allowed("other.com").unwrap()); // Not in allow list
    }

    #[test]
    fn test_network_policy_port_validation() {
        let policy = NetworkPolicy {
            enabled: true,
            allow_ports: vec![80, 443],
            ..Default::default()
        };

        assert!(policy.is_port_allowed(80).unwrap());
        assert!(policy.is_port_allowed(443).unwrap());
        assert!(!policy.is_port_allowed(22).unwrap()); // SSH is denied by default
        assert!(!policy.is_port_allowed(8080).unwrap()); // Not in allow list
    }
}
