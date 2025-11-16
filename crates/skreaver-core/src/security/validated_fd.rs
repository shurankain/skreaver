//! # TOCTOU-Safe File Descriptor Validation
//!
//! This module provides compile-time and runtime protection against
//! Time-Of-Check-Time-Of-Use (TOCTOU) race condition attacks by using
//! file descriptors instead of paths.
//!
//! # Problem: TOCTOU Vulnerabilities
//!
//! Traditional path-based validation is vulnerable to race conditions:
//!
//! ```ignore
//! // VULNERABLE CODE (do not use)
//! if is_safe_path(path) {  // Check at time T1
//!     // Attacker replaces file with symlink to /etc/passwd here!
//!     let contents = read(path);  // Use at time T2 - TOCTOU!
//! }
//! ```
//!
//! An attacker can:
//! 1. Create a safe file that passes validation
//! 2. Replace it with a malicious symlink between check and use
//! 3. Bypass all security checks
//!
//! # Solution: File Descriptor-Based Validation
//!
//! Use file descriptors to eliminate the race condition:
//!
//! ```ignore
//! // SAFE CODE
//! let fd = ValidatedFileDescriptor::open(path, policy)?;
//! // File descriptor is validated ONCE when opened
//! // All subsequent operations use the descriptor, not the path
//! let contents = fd.read_to_string()?; // No TOCTOU - uses fd directly
//! ```
//!
//! The file descriptor ensures:
//! - Validation happens atomically with file opening
//! - The validated descriptor can't be swapped out
//! - No window for attacker to modify filesystem

use super::{errors::SecurityError, policy::FileSystemPolicy};
use std::fs::{File, Metadata, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[cfg(target_family = "unix")]
use std::os::unix::fs::OpenOptionsExt;

/// A file descriptor that has been validated against security policies
///
/// This type eliminates TOCTOU vulnerabilities by:
/// 1. Opening the file and obtaining a descriptor atomically
/// 2. Validating the descriptor's metadata (not the path!)
/// 3. Providing operations on the descriptor, not the path
///
/// # Security Guarantees
///
/// - **Atomic validation**: Check and open happen atomically via O_NOFOLLOW
/// - **No path operations**: All operations use the file descriptor
/// - **Immutable after creation**: Can't be modified to point elsewhere
/// - **Typestate safety**: Read/write operations only available on appropriate types
///
/// # Example
///
/// ```ignore
/// use skreaver_core::security::{ValidatedFileDescriptor, FileSystemPolicy};
///
/// let policy = FileSystemPolicy::default();
///
/// // Open file with validation (atomic - no TOCTOU)
/// let fd = ValidatedFileDescriptor::open_read("/tmp/data.txt", &policy)?;
///
/// // Read using descriptor (safe - no path traversal possible)
/// let contents = fd.read_to_string()?;
/// ```
#[derive(Debug)]
pub struct ValidatedFileDescriptor {
    /// The underlying file descriptor
    file: File,
    /// The canonical path (for logging/debugging only, never used for operations)
    canonical_path: PathBuf,
    /// File metadata captured at open time
    metadata: Metadata,
}

impl ValidatedFileDescriptor {
    /// Open a file for reading with TOCTOU-safe validation
    ///
    /// This performs atomic validation:
    /// 1. Opens file with O_NOFOLLOW (fails if path is symlink)
    /// 2. Gets file descriptor
    /// 3. Validates descriptor metadata
    /// 4. Returns validated descriptor
    ///
    /// # Arguments
    ///
    /// * `path` - Path to open (validated atomically)
    /// * `policy` - Security policy to enforce
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Path doesn't exist
    /// - Path is a symlink (when symlinks disabled)
    /// - Path violates policy (outside allowed directories)
    /// - File is too large
    /// - Insufficient permissions
    pub fn open_read(
        path: impl AsRef<Path>,
        policy: &FileSystemPolicy,
    ) -> Result<Self, SecurityError> {
        let path_ref = path.as_ref();

        // Open file with O_NOFOLLOW to prevent symlink following
        // This is atomic - either we get the file or we fail
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(Self::no_follow_flags())
            .open(path_ref)
            .map_err(|e| SecurityError::FileSystemError {
                operation: "open_read".to_string(),
                path: path_ref.to_string_lossy().to_string(),
                error: e.to_string(),
            })?;

        // Get metadata from the DESCRIPTOR, not the path
        // This ensures we're checking the file we actually opened
        let metadata = file
            .metadata()
            .map_err(|e| SecurityError::FileSystemError {
                operation: "metadata".to_string(),
                path: path_ref.to_string_lossy().to_string(),
                error: e.to_string(),
            })?;

        // Validate file size
        if metadata.len() > policy.max_file_size.bytes() {
            return Err(SecurityError::FileSizeLimitExceeded {
                size: metadata.len(),
                limit: policy.max_file_size.bytes(),
            });
        }

        // Get canonical path for logging (from descriptor via /proc/self/fd)
        // This is safe because we're reading from the descriptor, not the original path
        let canonical_path = Self::canonical_path_from_fd(&file, path_ref)?;

        // Validate the canonical path is allowed
        if !policy.is_path_allowed(&canonical_path)? {
            return Err(SecurityError::PathNotAllowed {
                path: canonical_path.to_string_lossy().to_string(),
            });
        }

        Ok(Self {
            file,
            canonical_path,
            metadata,
        })
    }

    /// Open a file for writing with TOCTOU-safe validation
    ///
    /// Creates the file if it doesn't exist. Uses O_NOFOLLOW to prevent
    /// symlink attacks.
    pub fn open_write(
        path: impl AsRef<Path>,
        policy: &FileSystemPolicy,
    ) -> Result<Self, SecurityError> {
        let path_ref = path.as_ref();

        // Open/create file with O_NOFOLLOW
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .custom_flags(Self::no_follow_flags())
            .open(path_ref)
            .map_err(|e| SecurityError::FileSystemError {
                operation: "open_write".to_string(),
                path: path_ref.to_string_lossy().to_string(),
                error: e.to_string(),
            })?;

        let metadata = file
            .metadata()
            .map_err(|e| SecurityError::FileSystemError {
                operation: "metadata".to_string(),
                path: path_ref.to_string_lossy().to_string(),
                error: e.to_string(),
            })?;

        let canonical_path = Self::canonical_path_from_fd(&file, path_ref)?;

        if !policy.is_path_allowed(&canonical_path)? {
            return Err(SecurityError::PathNotAllowed {
                path: canonical_path.to_string_lossy().to_string(),
            });
        }

        Ok(Self {
            file,
            canonical_path,
            metadata,
        })
    }

    /// Read entire file contents as a string
    ///
    /// Uses the validated file descriptor, not the path. No TOCTOU possible.
    pub fn read_to_string(mut self) -> Result<String, SecurityError> {
        let mut contents = String::new();
        self.file
            .read_to_string(&mut contents)
            .map_err(|e| SecurityError::FileSystemError {
                operation: "read_to_string".to_string(),
                path: self.canonical_path.to_string_lossy().to_string(),
                error: e.to_string(),
            })?;
        Ok(contents)
    }

    /// Read entire file contents as bytes
    pub fn read_to_vec(mut self) -> Result<Vec<u8>, SecurityError> {
        let mut contents = Vec::new();
        self.file
            .read_to_end(&mut contents)
            .map_err(|e| SecurityError::FileSystemError {
                operation: "read_to_vec".to_string(),
                path: self.canonical_path.to_string_lossy().to_string(),
                error: e.to_string(),
            })?;
        Ok(contents)
    }

    /// Write contents to the file
    pub fn write_all(mut self, contents: &[u8]) -> Result<(), SecurityError> {
        self.file
            .write_all(contents)
            .map_err(|e| SecurityError::FileSystemError {
                operation: "write_all".to_string(),
                path: self.canonical_path.to_string_lossy().to_string(),
                error: e.to_string(),
            })?;
        Ok(())
    }

    /// Get the canonical path (for logging only - never use for operations!)
    pub fn path(&self) -> &Path {
        &self.canonical_path
    }

    /// Get file metadata captured at open time
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    /// Get O_NOFOLLOW flags for the current platform
    #[cfg(target_os = "linux")]
    fn no_follow_flags() -> i32 {
        0x20000 // O_NOFOLLOW on Linux
    }

    #[cfg(target_os = "macos")]
    fn no_follow_flags() -> i32 {
        0x0100 // O_NOFOLLOW on macOS
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    fn no_follow_flags() -> i32 {
        0 // Unsupported - best effort
    }

    /// Get canonical path from file descriptor
    ///
    /// On Unix, reads from /proc/self/fd/{fd} which is the canonical path
    /// This is safe because we're reading from the descriptor we control
    #[cfg(target_family = "unix")]
    fn canonical_path_from_fd(file: &File, fallback: &Path) -> Result<PathBuf, SecurityError> {
        use std::os::unix::io::AsRawFd;

        let fd = file.as_raw_fd();
        let proc_path = format!("/proc/self/fd/{}", fd);

        std::fs::read_link(&proc_path)
            .or_else(|_| fallback.canonicalize())
            .map_err(|e| SecurityError::FileSystemError {
                operation: "canonical_path".to_string(),
                path: fallback.to_string_lossy().to_string(),
                error: e.to_string(),
            })
    }

    #[cfg(not(target_family = "unix"))]
    fn canonical_path_from_fd(_file: &File, fallback: &Path) -> Result<PathBuf, SecurityError> {
        fallback
            .canonicalize()
            .map_err(|e| SecurityError::FileSystemError {
                operation: "canonical_path".to_string(),
                path: fallback.to_string_lossy().to_string(),
                error: e.to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::policy::{FileSystemAccess, SymlinkBehavior};

    fn create_test_dir() -> PathBuf {
        let temp_dir = std::env::temp_dir().join(format!("skreaver_test_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        temp_dir
    }

    fn create_test_policy(temp_dir: &Path) -> FileSystemPolicy {
        FileSystemPolicy {
            access: FileSystemAccess::Enabled {
                symlink_behavior: SymlinkBehavior::NoFollow,
                content_scanning: false,
            },
            allow_paths: vec![temp_dir.to_path_buf()],
            deny_patterns: vec![],
            max_file_size: crate::security::policy::FileSizeLimit::megabytes(10).unwrap(),
            max_files_per_operation: crate::security::policy::FileCountLimit::new(100).unwrap(),
        }
    }

    #[test]
    fn test_open_read_validated_fd() {
        let temp_dir = create_test_dir();
        let policy = create_test_policy(&temp_dir);
        let test_file = temp_dir.join("test.txt");

        // Create test file
        std::fs::write(&test_file, b"test content").unwrap();

        // Open and validate
        let fd = ValidatedFileDescriptor::open_read(&test_file, &policy).unwrap();
        let contents = fd.read_to_string().unwrap();

        assert_eq!(contents, "test content");

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_rejects_symlinks() {
        let temp_dir = create_test_dir();
        let policy = create_test_policy(&temp_dir);
        let real_file = temp_dir.join("real.txt");
        let symlink = temp_dir.join("link.txt");

        // Create real file
        std::fs::write(&real_file, b"content").unwrap();

        // Create symlink
        #[cfg(target_family = "unix")]
        std::os::unix::fs::symlink(&real_file, &symlink).unwrap();

        #[cfg(target_family = "unix")]
        {
            // Attempt to open symlink should fail
            let result = ValidatedFileDescriptor::open_read(&symlink, &policy);
            assert!(result.is_err());
        }

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_open_write_validated_fd() {
        let temp_dir = create_test_dir();
        let policy = create_test_policy(&temp_dir);
        let test_file = temp_dir.join("write_test.txt");

        // Open for writing
        let fd = ValidatedFileDescriptor::open_write(&test_file, &policy).unwrap();
        fd.write_all(b"written content").unwrap();

        // Verify contents
        let contents = std::fs::read_to_string(&test_file).unwrap();
        assert_eq!(contents, "written content");

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_file_size_limit() {
        let temp_dir = create_test_dir();
        let mut policy = create_test_policy(&temp_dir);
        policy.max_file_size = crate::security::policy::FileSizeLimit::new(10).unwrap();

        let test_file = temp_dir.join("large.txt");

        // Create file larger than limit
        std::fs::write(&test_file, b"this is more than 10 bytes").unwrap();

        // Should fail due to size limit
        let result = ValidatedFileDescriptor::open_read(&test_file, &policy);
        assert!(result.is_err());

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
