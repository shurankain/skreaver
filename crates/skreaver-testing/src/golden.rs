//! # Golden Test Framework
//!
//! This module provides comprehensive golden test capabilities for testing tool outputs
//! against stored snapshots, ensuring consistent behavior across versions and platforms.

use crate::MockToolRegistry;
use serde::{Deserialize, Serialize};
use skreaver_core::{ExecutionResult, ToolCall, ToolDispatch};
use skreaver_tools::ToolRegistry;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tempfile::TempDir;
use thiserror::Error;

/// Errors that can occur during golden test operations
#[derive(Debug, Error, Clone)]
pub enum GoldenTestError {
    #[error("Snapshot file not found: {0}")]
    SnapshotNotFound(PathBuf),

    #[error("Failed to serialize snapshot: {0}")]
    SerializationError(String),

    #[error("Failed to read/write snapshot file: {0}")]
    IoError(String),

    #[error("Tool execution failed: {0}")]
    ToolExecutionError(String),

    #[error("Snapshot validation failed: {0}")]
    ValidationError(String),

    #[error("Cross-platform normalization error: {0}")]
    NormalizationError(String),
}

impl From<serde_json::Error> for GoldenTestError {
    fn from(err: serde_json::Error) -> Self {
        GoldenTestError::SerializationError(err.to_string())
    }
}

impl From<std::io::Error> for GoldenTestError {
    fn from(err: std::io::Error) -> Self {
        GoldenTestError::IoError(err.to_string())
    }
}

/// Represents a captured tool execution snapshot
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolSnapshot {
    /// Name of the tool
    pub tool_name: String,
    /// Tool dispatch type (Standard or Custom)
    pub tool_type: ToolDispatchType,
    /// Input provided to the tool
    pub input: String,
    /// Execution result from the tool
    pub result: SerializedExecutionResult,
    /// Timestamp when snapshot was created
    pub timestamp: u64,
    /// Platform information for cross-platform testing
    pub platform_info: PlatformInfo,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Version of the tool/framework when snapshot was created
    pub version: String,
}

/// Serializable version of ToolDispatch
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ToolDispatchType {
    Standard(String), // Store as string for stability
    Custom(String),
}

impl From<&ToolDispatch> for ToolDispatchType {
    fn from(dispatch: &ToolDispatch) -> Self {
        match dispatch {
            ToolDispatch::Standard(tool) => ToolDispatchType::Standard(tool.name().to_string()),
            ToolDispatch::Custom(name) => ToolDispatchType::Custom(name.to_string()),
        }
    }
}

/// Serializable version of ExecutionResult
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SerializedExecutionResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub execution_time: Option<u64>,
}

impl From<&ExecutionResult> for SerializedExecutionResult {
    fn from(result: &ExecutionResult) -> Self {
        Self {
            success: result.is_success(),
            output: result.output().to_string(),
            error: if result.is_success() {
                None
            } else {
                Some(result.output().to_string())
            },
            execution_time: None, // Could be added to ExecutionResult in future
        }
    }
}

impl From<SerializedExecutionResult> for ExecutionResult {
    fn from(result: SerializedExecutionResult) -> Self {
        if result.success {
            ExecutionResult::success(result.output)
        } else {
            ExecutionResult::failure(result.error.unwrap_or(result.output))
        }
    }
}

/// Platform-specific information for snapshot normalization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlatformInfo {
    pub os: String,
    pub arch: String,
    pub endianness: String,
}

impl Default for PlatformInfo {
    fn default() -> Self {
        Self {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            endianness: if cfg!(target_endian = "big") {
                "big".to_string()
            } else {
                "little".to_string()
            },
        }
    }
}

/// Collection of tool snapshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotCollection {
    /// Map from test case ID to snapshot
    pub snapshots: HashMap<String, ToolSnapshot>,
    /// Metadata about the collection
    pub metadata: SnapshotMetadata,
}

/// Metadata for snapshot collections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub created_at: u64,
    pub version: String,
    pub description: String,
    pub total_snapshots: usize,
}

/// Manages storage and retrieval of golden test snapshots
pub struct SnapshotManager {
    /// Base directory for storing snapshots
    base_dir: PathBuf,
    /// Current snapshot collection
    collection: SnapshotCollection,
    /// Whether to automatically create missing directories
    #[allow(dead_code)]
    auto_create_dirs: bool,
}

impl SnapshotManager {
    /// Create a new snapshot manager with the specified base directory
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Result<Self, GoldenTestError> {
        let base_dir = base_dir.as_ref().to_path_buf();

        // Create base directory if it doesn't exist
        if !base_dir.exists() {
            fs::create_dir_all(&base_dir)?;
        }

        let collection = Self::load_or_create_collection(&base_dir)?;

        Ok(Self {
            base_dir,
            collection,
            auto_create_dirs: true,
        })
    }

    /// Create a new snapshot manager for testing (uses temp directory)
    pub fn new_for_testing() -> Result<(Self, TempDir), GoldenTestError> {
        let temp_dir = tempfile::tempdir()?;
        let manager = Self::new(temp_dir.path())?;
        Ok((manager, temp_dir))
    }

    /// Load existing collection or create a new one
    fn load_or_create_collection(base_dir: &Path) -> Result<SnapshotCollection, GoldenTestError> {
        let snapshots_file = base_dir.join("snapshots.json");

        if snapshots_file.exists() {
            let content = fs::read_to_string(&snapshots_file)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            Ok(SnapshotCollection {
                snapshots: HashMap::new(),
                metadata: SnapshotMetadata {
                    created_at: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    description: "Golden test snapshots".to_string(),
                    total_snapshots: 0,
                },
            })
        }
    }

    /// Store a tool snapshot
    pub fn store_snapshot(
        &mut self,
        test_id: &str,
        snapshot: ToolSnapshot,
    ) -> Result<(), GoldenTestError> {
        self.collection
            .snapshots
            .insert(test_id.to_string(), snapshot);
        self.collection.metadata.total_snapshots = self.collection.snapshots.len();
        self.save_collection()
    }

    /// Retrieve a tool snapshot
    pub fn get_snapshot(&self, test_id: &str) -> Option<&ToolSnapshot> {
        self.collection.snapshots.get(test_id)
    }

    /// Update an existing snapshot
    pub fn update_snapshot(
        &mut self,
        test_id: &str,
        snapshot: ToolSnapshot,
    ) -> Result<(), GoldenTestError> {
        if self.collection.snapshots.contains_key(test_id) {
            self.collection
                .snapshots
                .insert(test_id.to_string(), snapshot);
            self.save_collection()
        } else {
            Err(GoldenTestError::SnapshotNotFound(
                self.base_dir.join(format!("{}.json", test_id)),
            ))
        }
    }

    /// Delete a snapshot
    pub fn delete_snapshot(&mut self, test_id: &str) -> Result<bool, GoldenTestError> {
        let removed = self.collection.snapshots.remove(test_id).is_some();
        if removed {
            self.collection.metadata.total_snapshots = self.collection.snapshots.len();
            self.save_collection()?;
        }
        Ok(removed)
    }

    /// List all snapshot test IDs
    pub fn list_snapshots(&self) -> Vec<String> {
        self.collection.snapshots.keys().cloned().collect()
    }

    /// Save the current collection to disk
    fn save_collection(&self) -> Result<(), GoldenTestError> {
        let snapshots_file = self.base_dir.join("snapshots.json");
        let content = serde_json::to_string_pretty(&self.collection)?;
        fs::write(snapshots_file, content)?;
        Ok(())
    }

    /// Get collection metadata
    pub fn metadata(&self) -> &SnapshotMetadata {
        &self.collection.metadata
    }

    /// Clear all snapshots (useful for testing)
    pub fn clear(&mut self) -> Result<(), GoldenTestError> {
        self.collection.snapshots.clear();
        self.collection.metadata.total_snapshots = 0;
        self.save_collection()
    }

    /// Create directories for organizing snapshots by tool category
    pub fn setup_standard_directories(&self) -> Result<(), GoldenTestError> {
        let categories = vec!["standard_tools", "custom_tools", "integration_tests"];

        for category in categories {
            let dir_path = self.base_dir.join(category);
            if !dir_path.exists() {
                fs::create_dir_all(dir_path)?;
            }
        }

        Ok(())
    }
}

/// Tool output capture for golden testing
pub struct ToolCapture {
    registry: Box<dyn ToolRegistry + Send + Sync>,
    normalization_enabled: bool,
}

impl ToolCapture {
    /// Create a new tool capture with the given registry
    pub fn new(registry: Box<dyn ToolRegistry + Send + Sync>) -> Self {
        Self {
            registry,
            normalization_enabled: true,
        }
    }

    /// Create with mock tools for testing
    pub fn new_with_mocks() -> Self {
        let registry = MockToolRegistry::new()
            .with_mock_tools()
            .with_standard_tool_mocks();
        Self::new(Box::new(registry))
    }

    /// Capture tool execution and create snapshot
    pub fn capture_tool_execution(
        &self,
        tool_call: ToolCall,
    ) -> Result<ToolSnapshot, GoldenTestError> {
        let start_time = Instant::now();

        // Execute the tool
        let result = self.registry.dispatch(tool_call.clone()).ok_or_else(|| {
            GoldenTestError::ToolExecutionError(format!(
                "Tool '{}' not found in registry",
                tool_call.name()
            ))
        })?;

        let duration = start_time.elapsed();

        // Create snapshot
        let snapshot = ToolSnapshot {
            tool_name: tool_call.name().to_string(),
            tool_type: ToolDispatchType::from(&tool_call.dispatch),
            input: tool_call.input.clone(),
            result: SerializedExecutionResult::from(&result),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            platform_info: PlatformInfo::default(),
            duration_ms: duration.as_millis() as u64,
            version: env!("CARGO_PKG_VERSION").to_string(),
        };

        // Apply normalization if enabled
        if self.normalization_enabled {
            Ok(self.normalize_snapshot(snapshot)?)
        } else {
            Ok(snapshot)
        }
    }

    /// Normalize snapshot for cross-platform consistency
    fn normalize_snapshot(
        &self,
        mut snapshot: ToolSnapshot,
    ) -> Result<ToolSnapshot, GoldenTestError> {
        // Normalize file paths to use forward slashes
        if snapshot.tool_name.contains("file") || snapshot.tool_name.contains("directory") {
            snapshot.result.output = snapshot.result.output.replace('\\', "/");

            if let Some(ref mut error) = snapshot.result.error {
                *error = error.replace('\\', "/");
            }
        }

        // Remove or normalize timestamps in output
        // This would need more sophisticated regex matching for real scenarios

        // Sort JSON objects if output contains JSON
        if snapshot.tool_name.contains("json")
            && let Ok(mut json_val) =
                serde_json::from_str::<serde_json::Value>(&snapshot.result.output)
        {
            Self::sort_json_recursively(&mut json_val);
            snapshot.result.output = serde_json::to_string(&json_val)
                .map_err(|e| GoldenTestError::NormalizationError(e.to_string()))?;
        }

        Ok(snapshot)
    }

    /// Recursively sort JSON objects for consistent ordering
    fn sort_json_recursively(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                // Collect all key-value pairs and sort them
                let mut sorted_pairs: Vec<(String, serde_json::Value)> =
                    map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                sorted_pairs.sort_by(|a, b| a.0.cmp(&b.0));

                // Recursively sort nested values
                for (_, val) in sorted_pairs.iter_mut() {
                    Self::sort_json_recursively(val);
                }

                // Clear and rebuild the map with sorted entries
                map.clear();
                for (key, value) in sorted_pairs {
                    map.insert(key, value);
                }
            }
            serde_json::Value::Array(arr) => {
                for item in arr.iter_mut() {
                    Self::sort_json_recursively(item);
                }
            }
            _ => {}
        }
    }

    /// Enable or disable cross-platform normalization
    pub fn set_normalization(&mut self, enabled: bool) {
        self.normalization_enabled = enabled;
    }
}

/// Compare two snapshots and return differences
pub fn compare_snapshots(expected: &ToolSnapshot, actual: &ToolSnapshot) -> SnapshotComparison {
    let mut differences = Vec::new();

    if expected.tool_name != actual.tool_name {
        differences.push(format!(
            "Tool name mismatch: expected '{}', got '{}'",
            expected.tool_name, actual.tool_name
        ));
    }

    if expected.input != actual.input {
        differences.push(format!(
            "Input mismatch: expected '{}', got '{}'",
            expected.input, actual.input
        ));
    }

    if expected.result != actual.result {
        differences.push(format!(
            "Result mismatch: expected '{:?}', got '{:?}'",
            expected.result, actual.result
        ));
    }

    // Compare success status specifically
    if expected.result.success != actual.result.success {
        differences.push(format!(
            "Success status mismatch: expected {}, got {}",
            expected.result.success, actual.result.success
        ));
    }

    // Compare outputs with detailed diff for better debugging
    if expected.result.output != actual.result.output {
        differences.push(format!(
            "Output mismatch:\n  Expected: '{}'\n  Actual:   '{}'",
            expected.result.output, actual.result.output
        ));
    }

    SnapshotComparison {
        matches: differences.is_empty(),
        differences,
        expected: expected.clone(),
        actual: actual.clone(),
    }
}

/// Result of comparing two snapshots
#[derive(Debug, Clone)]
pub struct SnapshotComparison {
    pub matches: bool,
    pub differences: Vec<String>,
    pub expected: ToolSnapshot,
    pub actual: ToolSnapshot,
}

impl SnapshotComparison {
    /// Get a human-readable summary of the comparison
    pub fn summary(&self) -> String {
        if self.matches {
            "✓ Snapshots match".to_string()
        } else {
            format!(
                "✗ Snapshots differ:\n{}",
                self.differences
                    .iter()
                    .map(|d| format!("  - {}", d))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skreaver_core::ToolCall;

    #[test]
    fn test_snapshot_manager_basic_operations() {
        let (mut manager, _temp_dir) = SnapshotManager::new_for_testing().unwrap();

        let snapshot = create_test_snapshot();

        // Store snapshot
        manager.store_snapshot("test1", snapshot.clone()).unwrap();

        // Retrieve snapshot
        let retrieved = manager.get_snapshot("test1").unwrap();
        assert_eq!(*retrieved, snapshot);

        // List snapshots
        let snapshots = manager.list_snapshots();
        assert_eq!(snapshots.len(), 1);
        assert!(snapshots.contains(&"test1".to_string()));
    }

    #[test]
    fn test_snapshot_serialization() {
        let snapshot = create_test_snapshot();

        let serialized = serde_json::to_string(&snapshot).unwrap();
        let deserialized: ToolSnapshot = serde_json::from_str(&serialized).unwrap();

        assert_eq!(snapshot, deserialized);
    }

    #[test]
    fn test_tool_capture_with_mocks() {
        let capture = ToolCapture::new_with_mocks();

        let tool_call = ToolCall::new("echo", "test input").unwrap();
        let snapshot = capture.capture_tool_execution(tool_call).unwrap();

        assert_eq!(snapshot.tool_name, "echo");
        assert_eq!(snapshot.input, "test input");
        assert!(snapshot.result.success);
    }

    #[test]
    fn test_snapshot_comparison() {
        let snapshot1 = create_test_snapshot();
        let mut snapshot2 = snapshot1.clone();
        snapshot2.result.output = "different output".to_string();

        let comparison = compare_snapshots(&snapshot1, &snapshot2);

        assert!(!comparison.matches);
        assert!(!comparison.differences.is_empty());
        assert!(comparison.summary().contains("✗"));
    }

    fn create_test_snapshot() -> ToolSnapshot {
        ToolSnapshot {
            tool_name: "test_tool".to_string(),
            tool_type: ToolDispatchType::Custom("test_tool".to_string()),
            input: "test input".to_string(),
            result: SerializedExecutionResult {
                success: true,
                output: "test output".to_string(),
                error: None,
                execution_time: Some(10),
            },
            timestamp: 1234567890,
            platform_info: PlatformInfo::default(),
            duration_ms: 10,
            version: "0.1.0".to_string(),
        }
    }
}
