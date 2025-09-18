//! SQLite backup functionality
//!
//! This module provides backup and restore capabilities for SQLite databases.

/// Handle for backup operations
pub struct BackupHandle {
    pub id: String,
    pub created_at: std::time::SystemTime,
    pub size_bytes: u64,
    pub format: BackupFormat,
    pub data: Vec<u8>,
}

/// Backup format types
#[derive(Debug, Clone, PartialEq)]
pub enum BackupFormat {
    Json,
    SqliteDump,
}
