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
