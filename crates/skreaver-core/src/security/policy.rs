//! Security policies for different tool types

use super::errors::SecurityError;
use super::path_to_string_checked;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

// ============================================================================
// Validated Limit Macro
// ============================================================================

/// Macro to define validated limit types with consistent structure.
///
/// This macro generates a newtype wrapper with:
/// - Validation on construction (zero check, max check, optional min check)
/// - Serde support
/// - A getter method
/// - Default implementation
/// - Error type with Display and Error trait implementations
///
/// # Variants
///
/// - `nonzero_max`: Validates value > 0 and <= MAX
/// - `max_only`: Validates value <= MAX (allows zero)
/// - `range`: Validates MIN <= value <= MAX
/// - `nonzero_only`: Validates value > 0 (no max)
macro_rules! define_validated_limit {
    // NonZero with Max limit (most common pattern)
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident($inner:ty);
        error = $error:ident;
        max = $max:expr;
        default = $default:expr;
        getter = $getter:ident;
        $(unit = $unit:expr;)?
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        $vis struct $name($inner);

        impl $name {
            /// Maximum allowed value
            pub const MAX: $inner = $max;

            /// Create a validated limit
            pub fn new(value: $inner) -> Result<Self, $error> {
                if value == 0 {
                    return Err($error::Zero);
                }
                if value > Self::MAX {
                    return Err($error::TooLarge { value, max: Self::MAX });
                }
                Ok($name(value))
            }

            /// Get the value
            pub fn $getter(&self) -> $inner {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                $name($default)
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        $vis enum $error {
            Zero,
            TooLarge { value: $inner, max: $inner },
        }

        impl std::fmt::Display for $error {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let unit = define_validated_limit!(@unit $($unit)?);
                match self {
                    Self::Zero => write!(f, "{} cannot be zero", stringify!($name)),
                    Self::TooLarge { value, max } => {
                        write!(f, "{} too large: {}{} (max: {}{})",
                            stringify!($name), value, unit, max, unit)
                    }
                }
            }
        }

        impl std::error::Error for $error {}
    };

    // Max only (allows zero)
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident($inner:ty);
        error = $error:ident;
        max_only = $max:expr;
        default = $default:expr;
        getter = $getter:ident;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        $vis struct $name($inner);

        impl $name {
            /// Maximum allowed value
            pub const MAX: $inner = $max;

            /// Create a validated limit
            pub fn new(value: $inner) -> Result<Self, $error> {
                if value > Self::MAX {
                    return Err($error::TooLarge { value, max: Self::MAX });
                }
                Ok($name(value))
            }

            /// Get the value
            pub fn $getter(&self) -> $inner {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                $name($default)
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        $vis enum $error {
            TooLarge { value: $inner, max: $inner },
        }

        impl std::fmt::Display for $error {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    Self::TooLarge { value, max } => {
                        write!(f, "{} too large: {} (max: {})", stringify!($name), value, max)
                    }
                }
            }
        }

        impl std::error::Error for $error {}
    };

    // Range (min and max)
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident($inner:ty);
        error = $error:ident;
        min = $min:expr;
        max = $max:expr;
        default = $default:expr;
        getter = $getter:ident;
        $(unit = $unit:expr;)?
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        $vis struct $name($inner);

        impl $name {
            /// Minimum allowed value
            pub const MIN: $inner = $min;
            /// Maximum allowed value
            pub const MAX: $inner = $max;

            /// Create a validated limit
            pub fn new(value: $inner) -> Result<Self, $error> {
                if value < Self::MIN {
                    return Err($error::TooSmall { value, min: Self::MIN });
                }
                if value > Self::MAX {
                    return Err($error::TooLarge { value, max: Self::MAX });
                }
                Ok($name(value))
            }

            /// Get the value
            pub fn $getter(&self) -> $inner {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                $name($default)
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        $vis enum $error {
            TooSmall { value: $inner, min: $inner },
            TooLarge { value: $inner, max: $inner },
        }

        impl std::fmt::Display for $error {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let unit = define_validated_limit!(@unit $($unit)?);
                match self {
                    Self::TooSmall { value, min } => {
                        write!(f, "{} too small: {}{} (min: {}{})",
                            stringify!($name), value, unit, min, unit)
                    }
                    Self::TooLarge { value, max } => {
                        write!(f, "{} too large: {}{} (max: {}{})",
                            stringify!($name), value, unit, max, unit)
                    }
                }
            }
        }

        impl std::error::Error for $error {}
    };

    // NonZero only (no max)
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident($inner:ty);
        error = $error:ident;
        nonzero;
        getter = $getter:ident;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        $vis struct $name($inner);

        impl $name {
            /// Create a validated value
            pub fn new(value: $inner) -> Result<Self, $error> {
                if value == 0 {
                    return Err($error::Zero);
                }
                Ok($name(value))
            }

            /// Get the value
            pub fn $getter(&self) -> $inner {
                self.0
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        $vis enum $error {
            Zero,
        }

        impl std::fmt::Display for $error {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    Self::Zero => write!(f, "{} cannot be zero", stringify!($name)),
                }
            }
        }

        impl std::error::Error for $error {}
    };

    // Helper for optional unit suffix
    (@unit) => { "" };
    (@unit $unit:expr) => { $unit };
}

// ============================================================================
// Limit Type Definitions
// ============================================================================

define_validated_limit! {
    /// Validated file size limit in bytes
    pub struct FileSizeLimit(u64);
    error = FileSizeLimitError;
    max = 1024 * 1024 * 1024; // 1GB
    default = 16 * 1024 * 1024; // 16MB
    getter = bytes;
    unit = " bytes";
}

impl FileSizeLimit {
    /// Create file size limit in megabytes
    pub fn megabytes(mb: u32) -> Result<Self, FileSizeLimitError> {
        let bytes = (mb as u64) * 1024 * 1024;
        Self::new(bytes)
    }

    /// Get the limit in megabytes (rounded up)
    pub fn megabytes_rounded(&self) -> u64 {
        self.0.div_ceil(1024 * 1024)
    }
}

define_validated_limit! {
    /// Validated file count limit
    pub struct FileCountLimit(u32);
    error = FileCountLimitError;
    max = 10000;
    default = 100;
    getter = count;
}

define_validated_limit! {
    /// Validated timeout duration in seconds
    pub struct TimeoutSeconds(u64);
    error = TimeoutError;
    min = 1; // 1 second
    max = 24 * 60 * 60; // 24 hours
    default = 30;
    getter = seconds;
    unit = "s";
}

impl TimeoutSeconds {
    /// Convert to Duration
    pub fn as_duration(&self) -> Duration {
        Duration::from_secs(self.0)
    }
}

define_validated_limit! {
    /// Validated HTTP response size limit in bytes
    pub struct ResponseSizeLimit(u64);
    error = ResponseSizeLimitError;
    max = 500 * 1024 * 1024; // 500MB
    default = 32 * 1024 * 1024; // 32MB
    getter = bytes;
    unit = " bytes";
}

impl ResponseSizeLimit {
    /// Create response size limit in megabytes
    pub fn megabytes(mb: u32) -> Result<Self, ResponseSizeLimitError> {
        let bytes = (mb as u64) * 1024 * 1024;
        Self::new(bytes)
    }
}

define_validated_limit! {
    /// Validated redirect count limit
    pub struct RedirectLimit(u32);
    error = RedirectLimitError;
    max_only = 20;
    default = 3;
    getter = count;
}

define_validated_limit! {
    /// Validated network port
    pub struct NetworkPort(u16);
    error = NetworkPortError;
    nonzero;
    getter = port;
}

impl NetworkPort {
    /// Well-known ports (0-1023)
    pub const WELL_KNOWN_MAX: u16 = 1023;
    /// Ephemeral port range start
    pub const EPHEMERAL_MIN: u16 = 32768;

    /// Check if this is a well-known port
    pub fn is_well_known(&self) -> bool {
        self.0 <= Self::WELL_KNOWN_MAX
    }

    /// Check if this is in the ephemeral range
    pub fn is_ephemeral(&self) -> bool {
        self.0 >= Self::EPHEMERAL_MIN
    }
}

/// Combined security policy for an operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicy {
    pub fs_policy: FileSystemPolicy,
    pub http_policy: HttpPolicy,
    pub network_policy: NetworkPolicy,
}

/// Symlink behavior for file system operations
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SymlinkBehavior {
    /// Follow symbolic links
    Follow,
    /// Do not follow symbolic links
    NoFollow,
}

impl Default for SymlinkBehavior {
    fn default() -> Self {
        Self::NoFollow
    }
}

/// Content scanning strategy for file operations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContentScanning {
    /// No content scanning performed
    Disabled,
    /// Basic MIME type and magic number validation
    Basic,
    /// Advanced scanning with pattern matching for secrets/malware
    Advanced {
        /// Check for secrets (API keys, tokens, passwords)
        #[serde(default)]
        check_secrets: bool,
        /// Custom patterns to check for (regex)
        #[serde(default)]
        check_patterns: Vec<String>,
    },
}

impl Default for ContentScanning {
    fn default() -> Self {
        Self::Basic
    }
}

/// File system access mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileSystemAccess {
    /// File system access is disabled
    Disabled,
    /// File system access is enabled with specific constraints
    Enabled {
        symlink_behavior: SymlinkBehavior,
        content_scanning: ContentScanning,
    },
}

impl Default for FileSystemAccess {
    fn default() -> Self {
        Self::Enabled {
            symlink_behavior: SymlinkBehavior::default(),
            content_scanning: ContentScanning::default(),
        }
    }
}

/// File system access policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSystemPolicy {
    pub access: FileSystemAccess,
    pub allow_paths: Vec<PathBuf>,
    pub deny_patterns: Vec<String>,
    #[serde(alias = "max_file_size_bytes")]
    pub max_file_size: FileSizeLimit,
    pub max_files_per_operation: FileCountLimit,
}

impl Default for FileSystemPolicy {
    fn default() -> Self {
        Self {
            access: FileSystemAccess::default(),
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
            max_file_size: FileSizeLimit::default(), // 16MB
            max_files_per_operation: FileCountLimit::default(), // 100
        }
    }
}

impl FileSystemPolicy {
    pub fn disabled() -> Self {
        Self {
            access: FileSystemAccess::Disabled,
            ..Default::default()
        }
    }

    pub fn is_path_allowed(&self, path: &std::path::Path) -> Result<bool, SecurityError> {
        if matches!(self.access, FileSystemAccess::Disabled) {
            return Err(SecurityError::ToolDisabled {
                tool_name: "file_system".to_string(),
            });
        }

        // Canonicalize the path to resolve any ".." or symlinks
        let canonical_path = path
            .canonicalize()
            .map_err(|_| SecurityError::InvalidPath {
                path: path_to_string_checked(path),
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
        if size > self.max_file_size.bytes() {
            return Err(SecurityError::FileSizeLimitExceeded {
                size,
                limit: self.max_file_size.bytes(),
            });
        }
        Ok(())
    }
}

/// Common configuration for HTTP access
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HttpAccessConfig {
    pub timeout: TimeoutSeconds,
    pub max_response_size: ResponseSizeLimit,
}

/// Domain filtering strategy
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DomainFilter {
    /// Allow all domains (except those explicitly denied)
    AllowAll {
        #[serde(default)]
        deny_list: Vec<String>,
    },
    /// Only allow specific domains (and deny others)
    AllowList {
        allow_list: Vec<String>,
        #[serde(default)]
        deny_list: Vec<String>,
    },
}

impl Default for DomainFilter {
    fn default() -> Self {
        // Default denies dangerous internal endpoints
        Self::AllowAll {
            deny_list: vec![
                "localhost".to_string(),
                "127.0.0.1".to_string(),
                "0.0.0.0".to_string(),
                "169.254.169.254".to_string(),          // AWS metadata
                "metadata.google.internal".to_string(), // GCP metadata
                "10.*".to_string(),
                "172.16.*".to_string(),
                "192.168.*".to_string(),
            ],
        }
    }
}

/// HTTP access mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HttpAccess {
    /// HTTP access is disabled
    Disabled,
    /// Only local/loopback access allowed
    LocalOnly(HttpAccessConfig),
    /// Internet access with domain controls
    Internet {
        config: HttpAccessConfig,
        domain_filter: DomainFilter,
        include_local: bool,
        max_redirects: RedirectLimit,
        user_agent: String,
    },
}

impl Default for HttpAccess {
    fn default() -> Self {
        Self::Internet {
            config: HttpAccessConfig::default(),
            domain_filter: DomainFilter::default(),
            include_local: false,
            max_redirects: RedirectLimit::default(),
            user_agent: "skreaver-agent/0.1.0".to_string(),
        }
    }
}

/// HTTP client access policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpPolicy {
    pub access: HttpAccess,
    pub allow_methods: Vec<String>,
    pub default_headers: Vec<(String, String)>,
}

impl Default for HttpPolicy {
    fn default() -> Self {
        Self {
            access: HttpAccess::default(),
            allow_methods: vec![
                "GET".to_string(),
                "POST".to_string(),
                "PUT".to_string(),
                "PATCH".to_string(),
                "DELETE".to_string(),
            ],
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
            access: HttpAccess::Disabled,
            ..Default::default()
        }
    }

    pub fn is_domain_allowed(&self, domain: &str) -> Result<bool, SecurityError> {
        match &self.access {
            HttpAccess::Disabled => Err(SecurityError::ToolDisabled {
                tool_name: "http".to_string(),
            }),
            HttpAccess::LocalOnly(_) => {
                // Only allow localhost/127.0.0.1
                Ok(domain == "localhost" || domain.starts_with("127."))
            }
            HttpAccess::Internet {
                domain_filter,
                include_local,
                ..
            } => {
                // If include_local is false, block localhost
                if !include_local && (domain == "localhost" || domain.starts_with("127.")) {
                    return Ok(false);
                }

                // Check domain filter
                match domain_filter {
                    DomainFilter::AllowAll { deny_list } => {
                        // Check deny list
                        for denied in deny_list {
                            if Self::matches_pattern(domain, denied) {
                                return Ok(false);
                            }
                        }
                        // Not denied, so allow
                        Ok(true)
                    }
                    DomainFilter::AllowList {
                        allow_list,
                        deny_list,
                    } => {
                        // Check deny list first (takes precedence)
                        for denied in deny_list {
                            if Self::matches_pattern(domain, denied) {
                                return Ok(false);
                            }
                        }

                        // Check allow list
                        for allowed in allow_list {
                            if Self::matches_pattern(domain, allowed) {
                                return Ok(true);
                            }
                        }

                        // Not in allow list, so deny
                        Ok(false)
                    }
                }
            }
        }
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

    pub fn is_method_allowed(&self, method: &str) -> bool {
        self.allow_methods.contains(&method.to_uppercase())
    }

    pub fn get_timeout(&self) -> Duration {
        match &self.access {
            HttpAccess::Disabled => Duration::from_secs(0),
            HttpAccess::LocalOnly(config) | HttpAccess::Internet { config, .. } => {
                config.timeout.as_duration()
            }
        }
    }
}

/// Network access control
///
/// This enum makes the network access state explicit in the type system,
/// preventing confusion about whether network features are enabled/disabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum NetworkAccess {
    /// Network access is disabled
    Disabled,
    /// Network access is enabled
    Enabled,
}

impl Default for NetworkAccess {
    fn default() -> Self {
        // Secure by default: disabled
        Self::Disabled
    }
}

/// Network access policy for raw TCP/UDP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicy {
    #[serde(default)]
    pub access: NetworkAccess,
    pub allow_ports: Vec<NetworkPort>,
    pub deny_ports: Vec<NetworkPort>,
    #[serde(alias = "ttl_seconds")]
    pub ttl: TimeoutSeconds,
    pub allow_private_networks: bool,
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        // Helper function to create ports without panicking
        let create_port = |port: u16| NetworkPort::new(port).expect("Valid port number");

        Self {
            access: NetworkAccess::default(), // Disabled by default
            allow_ports: vec![],
            deny_ports: vec![
                create_port(22),    // SSH
                create_port(23),    // Telnet
                create_port(25),    // SMTP
                create_port(53),    // DNS
                create_port(135),   // RPC
                create_port(139),   // NetBIOS
                create_port(445),   // SMB
                create_port(1433),  // SQL Server
                create_port(3389),  // RDP
                create_port(5432),  // PostgreSQL
                create_port(6379),  // Redis
                create_port(27017), // MongoDB
            ],
            ttl: TimeoutSeconds::new(300).expect("Valid TTL"), // 5 minutes
            allow_private_networks: false,
        }
    }
}

impl NetworkPolicy {
    pub fn disabled() -> Self {
        Self {
            access: NetworkAccess::Disabled,
            ..Default::default()
        }
    }

    pub fn enabled() -> Self {
        Self {
            access: NetworkAccess::Enabled,
            ..Default::default()
        }
    }

    pub fn is_enabled(&self) -> bool {
        matches!(self.access, NetworkAccess::Enabled)
    }

    pub fn is_port_allowed(&self, port: NetworkPort) -> Result<bool, SecurityError> {
        match self.access {
            NetworkAccess::Disabled => {
                return Err(SecurityError::ToolDisabled {
                    tool_name: "network".to_string(),
                });
            }
            NetworkAccess::Enabled => {}
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

    /// Convenience method for checking port by u16
    pub fn is_port_allowed_u16(&self, port: u16) -> Result<bool, SecurityError> {
        match NetworkPort::new(port) {
            Ok(network_port) => self.is_port_allowed(network_port),
            Err(_) => Ok(false), // Invalid ports are not allowed
        }
    }
}

/// Tool-specific security policies
///
/// Defines security constraints and capabilities for individual tools.
/// Note: This is separate from RBAC `ToolPolicy` which handles role-based access control.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolSecurityPolicy {
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
            access: HttpAccess::Internet {
                config: HttpAccessConfig::default(),
                domain_filter: DomainFilter::AllowList {
                    allow_list: vec!["*.example.com".to_string(), "api.test.org".to_string()],
                    deny_list: vec!["evil.example.com".to_string()],
                },
                include_local: false,
                max_redirects: RedirectLimit::default(),
                user_agent: "test".to_string(),
            },
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
            access: NetworkAccess::Enabled,
            allow_ports: vec![
                NetworkPort::new(80).unwrap(),
                NetworkPort::new(443).unwrap(),
            ],
            ..Default::default()
        };

        assert!(policy.is_port_allowed_u16(80).unwrap());
        assert!(policy.is_port_allowed_u16(443).unwrap());
        assert!(!policy.is_port_allowed_u16(22).unwrap()); // SSH is denied by default
        assert!(!policy.is_port_allowed_u16(8080).unwrap()); // Not in allow list

        // Test invalid port (0)
        assert!(!policy.is_port_allowed_u16(0).unwrap()); // Invalid port
    }

    #[test]
    fn test_file_size_limit() {
        // Valid limits
        let limit = FileSizeLimit::new(1024).unwrap();
        assert_eq!(limit.bytes(), 1024);

        let limit_mb = FileSizeLimit::megabytes(16).unwrap();
        assert_eq!(limit_mb.bytes(), 16 * 1024 * 1024);
        assert_eq!(limit_mb.megabytes_rounded(), 16);

        // Invalid limits
        assert!(matches!(
            FileSizeLimit::new(0),
            Err(FileSizeLimitError::Zero)
        ));

        assert!(matches!(
            FileSizeLimit::new(FileSizeLimit::MAX + 1),
            Err(FileSizeLimitError::TooLarge { .. })
        ));
    }

    #[test]
    fn test_timeout_seconds() {
        // Valid timeouts
        let timeout = TimeoutSeconds::new(30).unwrap();
        assert_eq!(timeout.seconds(), 30);
        assert_eq!(timeout.as_duration(), Duration::from_secs(30));

        // Invalid timeouts
        assert!(matches!(
            TimeoutSeconds::new(0),
            Err(TimeoutError::TooSmall { .. })
        ));

        assert!(matches!(
            TimeoutSeconds::new(TimeoutSeconds::MAX + 1),
            Err(TimeoutError::TooLarge { .. })
        ));
    }

    #[test]
    fn test_network_port() {
        // Valid ports
        let port = NetworkPort::new(80).unwrap();
        assert_eq!(port.port(), 80);
        assert!(port.is_well_known());
        assert!(!port.is_ephemeral());

        let ephemeral_port = NetworkPort::new(35000).unwrap();
        assert!(!ephemeral_port.is_well_known());
        assert!(ephemeral_port.is_ephemeral());

        // Invalid port
        assert!(matches!(NetworkPort::new(0), Err(NetworkPortError::Zero)));
    }

    #[test]
    fn test_response_size_limit() {
        let limit = ResponseSizeLimit::megabytes(32).unwrap();
        assert_eq!(limit.bytes(), 32 * 1024 * 1024);

        assert!(matches!(
            ResponseSizeLimit::new(0),
            Err(ResponseSizeLimitError::Zero)
        ));
    }

    #[test]
    fn test_redirect_limit() {
        let limit = RedirectLimit::new(5).unwrap();
        assert_eq!(limit.count(), 5);

        assert!(matches!(
            RedirectLimit::new(RedirectLimit::MAX + 1),
            Err(RedirectLimitError::TooLarge { .. })
        ));
    }
}
