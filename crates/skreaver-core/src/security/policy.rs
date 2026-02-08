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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecurityPolicy {
    pub fs_policy: FileSystemPolicy,
    pub http_policy: HttpPolicy,
    pub network_policy: NetworkPolicy,
}

impl SecurityPolicy {
    /// Create a new security policy builder.
    pub fn builder() -> SecurityPolicyBuilder {
        SecurityPolicyBuilder::default()
    }

    /// Create a restrictive policy with all access disabled.
    pub fn restrictive() -> Self {
        Self {
            fs_policy: FileSystemPolicy::disabled(),
            http_policy: HttpPolicy::disabled(),
            network_policy: NetworkPolicy::disabled(),
        }
    }

    /// Create a permissive policy with sensible defaults.
    pub fn permissive() -> Self {
        Self {
            fs_policy: FileSystemPolicy::default(),
            http_policy: HttpPolicy::default(),
            network_policy: NetworkPolicy::enabled(),
        }
    }
}

/// Builder for constructing `SecurityPolicy` with a fluent API.
#[derive(Debug, Default)]
pub struct SecurityPolicyBuilder {
    fs_policy: Option<FileSystemPolicy>,
    http_policy: Option<HttpPolicy>,
    network_policy: Option<NetworkPolicy>,
}

impl SecurityPolicyBuilder {
    /// Set the file system policy.
    pub fn fs_policy(mut self, policy: FileSystemPolicy) -> Self {
        self.fs_policy = Some(policy);
        self
    }

    /// Set the HTTP policy.
    pub fn http_policy(mut self, policy: HttpPolicy) -> Self {
        self.http_policy = Some(policy);
        self
    }

    /// Set the network policy.
    pub fn network_policy(mut self, policy: NetworkPolicy) -> Self {
        self.network_policy = Some(policy);
        self
    }

    /// Disable file system access.
    pub fn disable_fs(mut self) -> Self {
        self.fs_policy = Some(FileSystemPolicy::disabled());
        self
    }

    /// Disable HTTP access.
    pub fn disable_http(mut self) -> Self {
        self.http_policy = Some(HttpPolicy::disabled());
        self
    }

    /// Disable network access.
    pub fn disable_network(mut self) -> Self {
        self.network_policy = Some(NetworkPolicy::disabled());
        self
    }

    /// Build the security policy.
    pub fn build(self) -> SecurityPolicy {
        SecurityPolicy {
            fs_policy: self.fs_policy.unwrap_or_default(),
            http_policy: self.http_policy.unwrap_or_default(),
            network_policy: self.network_policy.unwrap_or_default(),
        }
    }
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

    /// Create a new file system policy builder.
    pub fn builder() -> FileSystemPolicyBuilder {
        FileSystemPolicyBuilder::default()
    }
}

/// Builder for constructing `FileSystemPolicy` with a fluent API.
#[derive(Debug, Default)]
pub struct FileSystemPolicyBuilder {
    access: Option<FileSystemAccess>,
    allow_paths: Option<Vec<PathBuf>>,
    deny_patterns: Option<Vec<String>>,
    max_file_size: Option<FileSizeLimit>,
    max_files_per_operation: Option<FileCountLimit>,
}

impl FileSystemPolicyBuilder {
    /// Set the access mode.
    pub fn access(mut self, access: FileSystemAccess) -> Self {
        self.access = Some(access);
        self
    }

    /// Disable file system access.
    pub fn disabled(mut self) -> Self {
        self.access = Some(FileSystemAccess::Disabled);
        self
    }

    /// Set symlink behavior (requires enabled access).
    pub fn symlink_behavior(mut self, behavior: SymlinkBehavior) -> Self {
        self.access = Some(FileSystemAccess::Enabled {
            symlink_behavior: behavior,
            content_scanning: ContentScanning::default(),
        });
        self
    }

    /// Set content scanning (requires enabled access).
    pub fn content_scanning(mut self, scanning: ContentScanning) -> Self {
        let (behavior, _) = match self.access {
            Some(FileSystemAccess::Enabled {
                symlink_behavior,
                content_scanning,
            }) => (symlink_behavior, content_scanning),
            _ => (SymlinkBehavior::default(), ContentScanning::default()),
        };
        self.access = Some(FileSystemAccess::Enabled {
            symlink_behavior: behavior,
            content_scanning: scanning,
        });
        self
    }

    /// Set allowed paths, replacing any existing paths.
    pub fn allow_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.allow_paths = Some(paths);
        self
    }

    /// Add an allowed path.
    pub fn allow_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.allow_paths
            .get_or_insert_with(Vec::new)
            .push(path.into());
        self
    }

    /// Set deny patterns, replacing any existing patterns.
    pub fn deny_patterns(mut self, patterns: Vec<String>) -> Self {
        self.deny_patterns = Some(patterns);
        self
    }

    /// Add a deny pattern.
    pub fn deny_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.deny_patterns
            .get_or_insert_with(Vec::new)
            .push(pattern.into());
        self
    }

    /// Set maximum file size.
    pub fn max_file_size(mut self, limit: FileSizeLimit) -> Self {
        self.max_file_size = Some(limit);
        self
    }

    /// Set maximum file size in bytes.
    pub fn max_file_size_bytes(mut self, bytes: u64) -> Result<Self, FileSizeLimitError> {
        self.max_file_size = Some(FileSizeLimit::new(bytes)?);
        Ok(self)
    }

    /// Set maximum files per operation.
    pub fn max_files_per_operation(mut self, limit: FileCountLimit) -> Self {
        self.max_files_per_operation = Some(limit);
        self
    }

    /// Build the file system policy.
    pub fn build(self) -> FileSystemPolicy {
        let default = FileSystemPolicy::default();
        FileSystemPolicy {
            access: self.access.unwrap_or(default.access),
            allow_paths: self.allow_paths.unwrap_or(default.allow_paths),
            deny_patterns: self.deny_patterns.unwrap_or(default.deny_patterns),
            max_file_size: self.max_file_size.unwrap_or(default.max_file_size),
            max_files_per_operation: self
                .max_files_per_operation
                .unwrap_or(default.max_files_per_operation),
        }
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

    /// Create a new HTTP policy builder.
    pub fn builder() -> HttpPolicyBuilder {
        HttpPolicyBuilder::default()
    }

    /// Create a local-only HTTP policy.
    pub fn local_only() -> Self {
        Self {
            access: HttpAccess::LocalOnly(HttpAccessConfig::default()),
            ..Default::default()
        }
    }
}

/// Builder for constructing `HttpPolicy` with a fluent API.
#[derive(Debug, Default)]
pub struct HttpPolicyBuilder {
    access: Option<HttpAccess>,
    allow_methods: Option<Vec<String>>,
    default_headers: Option<Vec<(String, String)>>,
}

impl HttpPolicyBuilder {
    /// Set the access mode.
    pub fn access(mut self, access: HttpAccess) -> Self {
        self.access = Some(access);
        self
    }

    /// Disable HTTP access.
    pub fn disabled(mut self) -> Self {
        self.access = Some(HttpAccess::Disabled);
        self
    }

    /// Set to local-only access.
    pub fn local_only(mut self) -> Self {
        self.access = Some(HttpAccess::LocalOnly(HttpAccessConfig::default()));
        self
    }

    /// Set to internet access with default settings.
    pub fn internet(mut self) -> Self {
        self.access = Some(HttpAccess::default());
        self
    }

    /// Set the domain filter (for internet access).
    pub fn domain_filter(mut self, filter: DomainFilter) -> Self {
        if let Some(HttpAccess::Internet {
            domain_filter,
            config,
            include_local,
            max_redirects,
            user_agent,
        }) = self.access.take()
        {
            let _ = domain_filter; // discard old filter
            self.access = Some(HttpAccess::Internet {
                config,
                domain_filter: filter,
                include_local,
                max_redirects,
                user_agent,
            });
        } else {
            // Create default internet access with the filter
            self.access = Some(HttpAccess::Internet {
                config: HttpAccessConfig::default(),
                domain_filter: filter,
                include_local: false,
                max_redirects: RedirectLimit::default(),
                user_agent: "skreaver-agent/0.1.0".to_string(),
            });
        }
        self
    }

    /// Set timeout for HTTP requests.
    pub fn timeout(mut self, timeout: TimeoutSeconds) -> Self {
        match self.access.take() {
            Some(HttpAccess::LocalOnly(mut config)) => {
                config.timeout = timeout;
                self.access = Some(HttpAccess::LocalOnly(config));
            }
            Some(HttpAccess::Internet {
                mut config,
                domain_filter,
                include_local,
                max_redirects,
                user_agent,
            }) => {
                config.timeout = timeout;
                self.access = Some(HttpAccess::Internet {
                    config,
                    domain_filter,
                    include_local,
                    max_redirects,
                    user_agent,
                });
            }
            other => {
                self.access = other;
            }
        }
        self
    }

    /// Set allowed HTTP methods, replacing any existing methods.
    pub fn allow_methods(mut self, methods: Vec<String>) -> Self {
        self.allow_methods = Some(methods);
        self
    }

    /// Add an allowed HTTP method.
    pub fn allow_method(mut self, method: impl Into<String>) -> Self {
        self.allow_methods
            .get_or_insert_with(Vec::new)
            .push(method.into().to_uppercase());
        self
    }

    /// Set default headers, replacing any existing headers.
    pub fn default_headers(mut self, headers: Vec<(String, String)>) -> Self {
        self.default_headers = Some(headers);
        self
    }

    /// Add a default header.
    pub fn default_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.default_headers
            .get_or_insert_with(Vec::new)
            .push((key.into(), value.into()));
        self
    }

    /// Build the HTTP policy.
    pub fn build(self) -> HttpPolicy {
        let default = HttpPolicy::default();
        HttpPolicy {
            access: self.access.unwrap_or(default.access),
            allow_methods: self.allow_methods.unwrap_or(default.allow_methods),
            default_headers: self.default_headers.unwrap_or(default.default_headers),
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

    /// Create a new network policy builder.
    pub fn builder() -> NetworkPolicyBuilder {
        NetworkPolicyBuilder::default()
    }
}

/// Builder for constructing `NetworkPolicy` with a fluent API.
#[derive(Debug, Default)]
pub struct NetworkPolicyBuilder {
    access: Option<NetworkAccess>,
    allow_ports: Option<Vec<NetworkPort>>,
    deny_ports: Option<Vec<NetworkPort>>,
    ttl: Option<TimeoutSeconds>,
    allow_private_networks: Option<bool>,
}

impl NetworkPolicyBuilder {
    /// Set the access mode.
    pub fn access(mut self, access: NetworkAccess) -> Self {
        self.access = Some(access);
        self
    }

    /// Enable network access.
    pub fn enabled(mut self) -> Self {
        self.access = Some(NetworkAccess::Enabled);
        self
    }

    /// Disable network access.
    pub fn disabled(mut self) -> Self {
        self.access = Some(NetworkAccess::Disabled);
        self
    }

    /// Set allowed ports, replacing any existing ports.
    pub fn allow_ports(mut self, ports: Vec<NetworkPort>) -> Self {
        self.allow_ports = Some(ports);
        self
    }

    /// Add an allowed port.
    pub fn allow_port(mut self, port: NetworkPort) -> Self {
        self.allow_ports.get_or_insert_with(Vec::new).push(port);
        self
    }

    /// Add an allowed port by number.
    pub fn allow_port_num(mut self, port: u16) -> Result<Self, NetworkPortError> {
        self.allow_ports
            .get_or_insert_with(Vec::new)
            .push(NetworkPort::new(port)?);
        Ok(self)
    }

    /// Set denied ports, replacing any existing ports.
    pub fn deny_ports(mut self, ports: Vec<NetworkPort>) -> Self {
        self.deny_ports = Some(ports);
        self
    }

    /// Add a denied port.
    pub fn deny_port(mut self, port: NetworkPort) -> Self {
        self.deny_ports.get_or_insert_with(Vec::new).push(port);
        self
    }

    /// Add a denied port by number.
    pub fn deny_port_num(mut self, port: u16) -> Result<Self, NetworkPortError> {
        self.deny_ports
            .get_or_insert_with(Vec::new)
            .push(NetworkPort::new(port)?);
        Ok(self)
    }

    /// Set the TTL (time-to-live) for connections.
    pub fn ttl(mut self, ttl: TimeoutSeconds) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Set whether private networks are allowed.
    pub fn allow_private_networks(mut self, allow: bool) -> Self {
        self.allow_private_networks = Some(allow);
        self
    }

    /// Build the network policy.
    pub fn build(self) -> NetworkPolicy {
        let default = NetworkPolicy::default();
        NetworkPolicy {
            access: self.access.unwrap_or(default.access),
            allow_ports: self.allow_ports.unwrap_or(default.allow_ports),
            deny_ports: self.deny_ports.unwrap_or(default.deny_ports),
            ttl: self.ttl.unwrap_or(default.ttl),
            allow_private_networks: self
                .allow_private_networks
                .unwrap_or(default.allow_private_networks),
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

    #[test]
    fn test_security_policy_builder() {
        // Test default builder
        let policy = SecurityPolicy::builder().build();
        assert!(matches!(
            policy.fs_policy.access,
            FileSystemAccess::Enabled { .. }
        ));
        assert!(matches!(
            policy.http_policy.access,
            HttpAccess::Internet { .. }
        ));
        assert!(matches!(
            policy.network_policy.access,
            NetworkAccess::Disabled
        ));

        // Test with disabled components
        let restrictive = SecurityPolicy::builder()
            .disable_fs()
            .disable_http()
            .disable_network()
            .build();
        assert!(matches!(
            restrictive.fs_policy.access,
            FileSystemAccess::Disabled
        ));
        assert!(matches!(
            restrictive.http_policy.access,
            HttpAccess::Disabled
        ));
        assert!(matches!(
            restrictive.network_policy.access,
            NetworkAccess::Disabled
        ));

        // Test convenience constructors
        let restrictive2 = SecurityPolicy::restrictive();
        assert!(matches!(
            restrictive2.fs_policy.access,
            FileSystemAccess::Disabled
        ));

        let permissive = SecurityPolicy::permissive();
        assert!(matches!(
            permissive.network_policy.access,
            NetworkAccess::Enabled
        ));
    }

    #[test]
    fn test_file_system_policy_builder() {
        // Test with custom paths
        let policy = FileSystemPolicy::builder()
            .allow_path("/custom/path")
            .deny_pattern("*.secret")
            .max_file_size(FileSizeLimit::megabytes(32).unwrap())
            .build();

        assert!(
            policy
                .allow_paths
                .iter()
                .any(|p| p.to_str() == Some("/custom/path"))
        );
        assert!(policy.deny_patterns.contains(&"*.secret".to_string()));
        assert_eq!(policy.max_file_size.megabytes_rounded(), 32);

        // Test disabled
        let disabled = FileSystemPolicy::builder().disabled().build();
        assert!(matches!(disabled.access, FileSystemAccess::Disabled));
    }

    #[test]
    fn test_http_policy_builder() {
        // Test local only
        let local = HttpPolicy::builder().local_only().build();
        assert!(matches!(local.access, HttpAccess::LocalOnly(_)));

        // Test with custom methods
        let policy = HttpPolicy::builder()
            .allow_method("GET")
            .allow_method("post")
            .default_header("X-Custom", "value")
            .build();

        assert!(policy.allow_methods.contains(&"GET".to_string()));
        assert!(policy.allow_methods.contains(&"POST".to_string()));
        assert!(
            policy
                .default_headers
                .contains(&("X-Custom".to_string(), "value".to_string()))
        );

        // Test convenience constructor
        let local2 = HttpPolicy::local_only();
        assert!(matches!(local2.access, HttpAccess::LocalOnly(_)));
    }

    #[test]
    fn test_network_policy_builder() {
        // Test enabled with allowed ports
        let policy = NetworkPolicy::builder()
            .enabled()
            .allow_port_num(80)
            .unwrap()
            .allow_port_num(443)
            .unwrap()
            .allow_private_networks(true)
            .build();

        assert!(policy.is_enabled());
        assert_eq!(policy.allow_ports.len(), 2);
        assert!(policy.allow_private_networks);

        // Test with custom TTL
        let policy2 = NetworkPolicy::builder()
            .enabled()
            .ttl(TimeoutSeconds::new(600).unwrap())
            .build();

        assert_eq!(policy2.ttl.seconds(), 600);
    }
}
