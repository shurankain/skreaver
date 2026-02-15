//! # File System Tools
//!
//! This module provides file system operation tools for reading, writing,
//! and managing files and directories safely.

use crate::core::ToolConfig;
use serde::{Deserialize, Serialize};
use skreaver_core::{ExecutionResult, Tool};
use std::fs;
use std::path::Path;

/// Configuration for file operations
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FileConfig {
    pub path: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub create_dirs: bool,
}

impl ToolConfig for FileConfig {
    fn from_simple(input: String) -> Self {
        Self::new(input)
    }
}

impl FileConfig {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            content: None,
            create_dirs: false,
        }
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    pub fn with_create_dirs(mut self, create: bool) -> Self {
        self.create_dirs = create;
        self
    }
}

/// File reading tool
#[derive(Debug)]
pub struct FileReadTool;

impl FileReadTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FileReadTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "file_read"
    }

    fn description(&self) -> &str {
        "Read the contents of a file at the specified path"
    }

    fn input_schema(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read"
                }
            },
            "required": ["path"]
        }))
    }

    fn output_schema(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file that was read"
                },
                "content": {
                    "type": "string",
                    "description": "Contents of the file"
                },
                "size": {
                    "type": "integer",
                    "description": "Size of the file content in bytes"
                },
                "success": {
                    "type": "boolean",
                    "description": "Whether the operation succeeded"
                }
            },
            "required": ["path", "content", "size", "success"]
        }))
    }

    fn call(&self, input: String) -> ExecutionResult {
        let config = FileConfig::parse(input);

        match fs::read_to_string(&config.path) {
            Ok(content) => {
                let result = serde_json::json!({
                    "path": config.path,
                    "content": content,
                    "size": content.len(),
                    "success": true
                });
                ExecutionResult::success(result.to_string())
            }
            Err(e) => {
                ExecutionResult::failure(format!("Failed to read file '{}': {}", config.path, e))
            }
        }
    }
}

/// File writing tool
#[derive(Debug)]
pub struct FileWriteTool;

impl FileWriteTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FileWriteTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        "file_write"
    }

    fn description(&self) -> &str {
        "Write content to a file at the specified path"
    }

    fn input_schema(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                },
                "create_dirs": {
                    "type": "boolean",
                    "description": "Whether to create parent directories if they don't exist",
                    "default": false
                }
            },
            "required": ["path", "content"]
        }))
    }

    fn output_schema(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file that was written"
                },
                "bytes_written": {
                    "type": "integer",
                    "description": "Number of bytes written"
                },
                "success": {
                    "type": "boolean",
                    "description": "Whether the operation succeeded"
                }
            },
            "required": ["path", "bytes_written", "success"]
        }))
    }

    fn call(&self, input: String) -> ExecutionResult {
        let config: FileConfig = match serde_json::from_str(&input) {
            Ok(config) => config,
            Err(_) => {
                // Fallback to simple "path:content" format
                if let Some((path, content)) = input.split_once(':') {
                    FileConfig::new(path).with_content(content)
                } else {
                    return ExecutionResult::failure(
                        "Invalid input format. Expected JSON config or 'path:content' format"
                            .to_string(),
                    );
                }
            }
        };

        let content = match &config.content {
            Some(content) => content,
            None => {
                return ExecutionResult::failure("No content provided for file write".to_string());
            }
        };

        // Create parent directories if requested
        if let Some(parent) = config
            .create_dirs
            .then(|| Path::new(&config.path).parent())
            .flatten()
        {
            match fs::create_dir_all(parent) {
                Ok(()) => {}
                Err(e) => {
                    return ExecutionResult::failure(format!(
                        "Failed to create parent directories for '{}': {}",
                        config.path, e
                    ));
                }
            }
        }

        match fs::write(&config.path, content) {
            Ok(()) => {
                let result = serde_json::json!({
                    "path": config.path,
                    "bytes_written": content.len(),
                    "success": true
                });
                ExecutionResult::success(result.to_string())
            }
            Err(e) => {
                ExecutionResult::failure(format!("Failed to write file '{}': {}", config.path, e))
            }
        }
    }
}

/// Directory listing tool
#[derive(Debug)]
pub struct DirectoryListTool;

impl DirectoryListTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DirectoryListTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for DirectoryListTool {
    fn name(&self) -> &str {
        "directory_list"
    }

    fn description(&self) -> &str {
        "List the contents of a directory"
    }

    fn input_schema(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the directory to list"
                }
            },
            "required": ["path"]
        }))
    }

    fn output_schema(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the directory that was listed"
                },
                "files": {
                    "type": "array",
                    "description": "List of files in the directory",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" },
                            "size": { "type": "integer" },
                            "path": { "type": "string" }
                        }
                    }
                },
                "directories": {
                    "type": "array",
                    "description": "List of subdirectories",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" },
                            "path": { "type": "string" }
                        }
                    }
                },
                "errors": {
                    "type": "array",
                    "description": "Any errors encountered while listing",
                    "items": { "type": "string" }
                },
                "success": {
                    "type": "boolean",
                    "description": "Whether the operation succeeded"
                }
            },
            "required": ["path", "files", "directories", "errors", "success"]
        }))
    }

    fn call(&self, input: String) -> ExecutionResult {
        let config = FileConfig::parse(input);

        match fs::read_dir(&config.path) {
            Ok(entries) => {
                let mut files = Vec::new();
                let mut dirs = Vec::new();
                let mut errors = Vec::new();

                for entry_result in entries {
                    match entry_result {
                        Ok(entry) => {
                            let path = entry.path();
                            let name = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("???")
                                .to_string();

                            if path.is_file() {
                                let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                                files.push(serde_json::json!({
                                    "name": name,
                                    "size": size,
                                    "path": path.to_string_lossy()
                                }));
                            } else if path.is_dir() {
                                dirs.push(serde_json::json!({
                                    "name": name,
                                    "path": path.to_string_lossy()
                                }));
                            }
                        }
                        Err(e) => errors.push(e.to_string()),
                    }
                }

                let result = serde_json::json!({
                    "path": config.path,
                    "files": files,
                    "directories": dirs,
                    "errors": errors,
                    "success": true
                });
                ExecutionResult::success(result.to_string())
            }
            Err(e) => ExecutionResult::failure(format!(
                "Failed to list directory '{}': {}",
                config.path, e
            )),
        }
    }
}

/// Directory creation tool
#[derive(Debug)]
pub struct DirectoryCreateTool;

impl DirectoryCreateTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DirectoryCreateTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for DirectoryCreateTool {
    fn name(&self) -> &str {
        "directory_create"
    }

    fn description(&self) -> &str {
        "Create a new directory at the specified path"
    }

    fn input_schema(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path where the directory should be created"
                },
                "create_dirs": {
                    "type": "boolean",
                    "description": "Whether to create parent directories recursively",
                    "default": false
                }
            },
            "required": ["path"]
        }))
    }

    fn output_schema(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the directory that was created"
                },
                "created": {
                    "type": "boolean",
                    "description": "Whether the directory was created"
                },
                "recursive": {
                    "type": "boolean",
                    "description": "Whether parent directories were created"
                },
                "success": {
                    "type": "boolean",
                    "description": "Whether the operation succeeded"
                }
            },
            "required": ["path", "created", "recursive", "success"]
        }))
    }

    fn call(&self, input: String) -> ExecutionResult {
        let config = FileConfig::parse(input);

        let create_all = config.create_dirs;

        let result = if create_all {
            fs::create_dir_all(&config.path)
        } else {
            fs::create_dir(&config.path)
        };

        match result {
            Ok(()) => {
                let result = serde_json::json!({
                    "path": config.path,
                    "created": true,
                    "recursive": create_all,
                    "success": true
                });
                ExecutionResult::success(result.to_string())
            }
            Err(e) => ExecutionResult::failure(format!(
                "Failed to create directory '{}': {}",
                config.path, e
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skreaver_core::Tool;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_file_read_success() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "Hello, World!").unwrap();

        let tool = FileReadTool::new();
        let input = serde_json::json!({
            "path": file_path.to_str().unwrap()
        })
        .to_string();

        let result = tool.call(input);
        assert!(result.is_success());

        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert_eq!(output["content"], "Hello, World!");
        assert_eq!(output["size"], 13);
        assert!(output["success"].as_bool().unwrap());
    }

    #[test]
    fn test_file_read_simple_input() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("simple.txt");
        fs::write(&file_path, "Simple").unwrap();

        let tool = FileReadTool::new();
        let result = tool.call(file_path.to_str().unwrap().to_string());

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert_eq!(output["content"], "Simple");
    }

    #[test]
    fn test_file_read_not_found() {
        let tool = FileReadTool::new();
        let result = tool.call("/nonexistent/file.txt".to_string());

        assert!(result.is_failure());
        assert!(result.output().contains("Failed to read file"));
    }

    #[test]
    fn test_file_read_has_schemas() {
        let tool = FileReadTool::new();
        assert!(tool.input_schema().is_some());
        assert!(tool.output_schema().is_some());
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_file_write_success() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("output.txt");

        let tool = FileWriteTool::new();
        let input = serde_json::json!({
            "path": file_path.to_str().unwrap(),
            "content": "Test content"
        })
        .to_string();

        let result = tool.call(input);
        assert!(result.is_success());

        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert_eq!(output["bytes_written"], 12);
        assert!(output["success"].as_bool().unwrap());

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Test content");
    }

    #[test]
    fn test_file_write_with_create_dirs() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("subdir/nested/file.txt");

        let tool = FileWriteTool::new();
        let input = serde_json::json!({
            "path": file_path.to_str().unwrap(),
            "content": "Nested content",
            "create_dirs": true
        })
        .to_string();

        let result = tool.call(input);
        assert!(result.is_success());

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Nested content");
    }

    #[test]
    fn test_file_write_simple_format() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("simple.txt");

        let tool = FileWriteTool::new();
        let input = format!("{}:Simple content", file_path.to_str().unwrap());

        let result = tool.call(input);
        assert!(result.is_success());

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Simple content");
    }

    #[test]
    fn test_file_write_no_content() {
        let tool = FileWriteTool::new();
        let input = serde_json::json!({
            "path": "/tmp/test.txt"
        })
        .to_string();

        let result = tool.call(input);
        assert!(result.is_failure());
        assert!(result.output().contains("No content provided"));
    }

    #[test]
    fn test_file_write_has_schemas() {
        let tool = FileWriteTool::new();
        assert!(tool.input_schema().is_some());
        assert!(tool.output_schema().is_some());
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_directory_list_success() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path();

        fs::write(dir_path.join("file1.txt"), "content1").unwrap();
        fs::write(dir_path.join("file2.txt"), "content2").unwrap();
        fs::create_dir(dir_path.join("subdir")).unwrap();

        let tool = DirectoryListTool::new();
        let input = serde_json::json!({
            "path": dir_path.to_str().unwrap()
        })
        .to_string();

        let result = tool.call(input);
        assert!(result.is_success());

        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert!(output["success"].as_bool().unwrap());
        assert_eq!(output["files"].as_array().unwrap().len(), 2);
        assert_eq!(output["directories"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_directory_list_simple_input() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("test.txt"), "test").unwrap();

        let tool = DirectoryListTool::new();
        let result = tool.call(temp_dir.path().to_str().unwrap().to_string());

        assert!(result.is_success());
    }

    #[test]
    fn test_directory_list_not_found() {
        let tool = DirectoryListTool::new();
        let result = tool.call("/nonexistent/directory".to_string());

        assert!(result.is_failure());
        assert!(result.output().contains("Failed to list directory"));
    }

    #[test]
    fn test_directory_list_has_schemas() {
        let tool = DirectoryListTool::new();
        assert!(tool.input_schema().is_some());
        assert!(tool.output_schema().is_some());
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_directory_create_success() {
        let temp_dir = TempDir::new().unwrap();
        let new_dir = temp_dir.path().join("newdir");

        let tool = DirectoryCreateTool::new();
        let input = serde_json::json!({
            "path": new_dir.to_str().unwrap()
        })
        .to_string();

        let result = tool.call(input);
        assert!(result.is_success());

        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert!(output["created"].as_bool().unwrap());
        assert!(!output["recursive"].as_bool().unwrap());
        assert!(new_dir.exists());
    }

    #[test]
    fn test_directory_create_recursive() {
        let temp_dir = TempDir::new().unwrap();
        let nested_dir = temp_dir.path().join("a/b/c");

        let tool = DirectoryCreateTool::new();
        let input = serde_json::json!({
            "path": nested_dir.to_str().unwrap(),
            "create_dirs": true
        })
        .to_string();

        let result = tool.call(input);
        assert!(result.is_success());

        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert!(output["recursive"].as_bool().unwrap());
        assert!(nested_dir.exists());
    }

    #[test]
    fn test_directory_create_simple_input() {
        let temp_dir = TempDir::new().unwrap();
        let new_dir = temp_dir.path().join("simpledir");

        let tool = DirectoryCreateTool::new();
        let result = tool.call(new_dir.to_str().unwrap().to_string());

        assert!(result.is_success());
        assert!(new_dir.exists());
    }

    #[test]
    fn test_directory_create_has_schemas() {
        let tool = DirectoryCreateTool::new();
        assert!(tool.input_schema().is_some());
        assert!(tool.output_schema().is_some());
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_file_config_parse() {
        let config: FileConfig = serde_json::from_str(r#"{"path":"/tmp/test.txt"}"#).unwrap();
        assert_eq!(config.path, "/tmp/test.txt");

        let config = FileConfig::parse("/simple/path".to_string());
        assert_eq!(config.path, "/simple/path");
    }

    #[test]
    fn test_file_config_builder() {
        let config = FileConfig::new("/test/path")
            .with_content("test content")
            .with_create_dirs(true);

        assert_eq!(config.path, "/test/path");
        assert_eq!(config.content, Some("test content".to_string()));
        assert!(config.create_dirs);
    }
}
