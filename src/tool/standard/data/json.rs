//! # Data Processing Tools
//!
//! This module provides tools for processing and transforming structured data
//! including JSON and XML parsing, validation, and transformation.

use crate::tool::{ExecutionResult, Tool};
use quick_xml::de::from_str as xml_from_str;
use serde::{Deserialize, Serialize};
use serde_json::{self, Value as JsonValue};

/// Configuration for data processing operations
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DataConfig {
    pub input: String,
    #[serde(default)]
    pub path: Option<String>, // JSON path for extraction
    #[serde(default)]
    pub format: Option<String>, // Output format: "pretty", "compact"
}

impl DataConfig {
    pub fn new(input: impl Into<String>) -> Self {
        Self {
            input: input.into(),
            path: None,
            format: None,
        }
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn with_format(mut self, format: impl Into<String>) -> Self {
        self.format = Some(format.into());
        self
    }
}

/// JSON parsing and validation tool
pub struct JsonParseTool;

impl JsonParseTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for JsonParseTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for JsonParseTool {
    fn name(&self) -> &str {
        "json_parse"
    }

    fn call(&self, input: String) -> ExecutionResult {
        let config: DataConfig = match serde_json::from_str(&input) {
            Ok(config) => config,
            Err(_) => DataConfig::new(input), // Fallback to direct JSON input
        };

        match serde_json::from_str::<JsonValue>(&config.input) {
            Ok(parsed) => {
                let output = if let Some(format) = &config.format {
                    match format.as_str() {
                        "pretty" => serde_json::to_string_pretty(&parsed)
                            .unwrap_or_else(|_| parsed.to_string()),
                        _ => parsed.to_string(),
                    }
                } else {
                    parsed.to_string()
                };

                let result = serde_json::json!({
                    "parsed": parsed,
                    "formatted": output,
                    "valid": true,
                    "success": true
                });
                ExecutionResult::success(result.to_string())
            }
            Err(e) => {
                let result = serde_json::json!({
                    "valid": false,
                    "error": e.to_string(),
                    "success": false
                });
                ExecutionResult::success(result.to_string())
            }
        }
    }
}

/// JSON transformation tool for extracting and modifying JSON data
pub struct JsonTransformTool;

impl JsonTransformTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for JsonTransformTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for JsonTransformTool {
    fn name(&self) -> &str {
        "json_transform"
    }

    fn call(&self, input: String) -> ExecutionResult {
        let config: DataConfig = match serde_json::from_str(&input) {
            Ok(config) => config,
            Err(e) => return ExecutionResult::failure(format!("Invalid config JSON: {}", e)),
        };

        let json_value: JsonValue = match serde_json::from_str(&config.input) {
            Ok(value) => value,
            Err(e) => return ExecutionResult::failure(format!("Invalid input JSON: {}", e)),
        };

        let extracted = if let Some(path) = &config.path {
            // Simple path extraction (dot notation)
            extract_json_path(&json_value, path)
        } else {
            Some(json_value.clone())
        };

        match extracted {
            Some(value) => {
                let output = if let Some(format) = &config.format {
                    match format.as_str() {
                        "pretty" => serde_json::to_string_pretty(&value)
                            .unwrap_or_else(|_| value.to_string()),
                        _ => value.to_string(),
                    }
                } else {
                    value.to_string()
                };

                let result = serde_json::json!({
                    "original": json_value,
                    "extracted": value,
                    "formatted": output,
                    "path": config.path,
                    "success": true
                });
                ExecutionResult::success(result.to_string())
            }
            None => ExecutionResult::failure(format!(
                "Path '{}' not found in JSON",
                config.path.unwrap_or_default()
            )),
        }
    }
}

/// XML parsing tool
pub struct XmlParseTool;

impl XmlParseTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for XmlParseTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for XmlParseTool {
    fn name(&self) -> &str {
        "xml_parse"
    }

    fn call(&self, input: String) -> ExecutionResult {
        let config: DataConfig = match serde_json::from_str(&input) {
            Ok(config) => config,
            Err(_) => DataConfig::new(input), // Fallback to direct XML input
        };

        // First try to parse as generic XML structure
        match xml_from_str::<serde_json::Value>(&config.input) {
            Ok(parsed) => {
                let json_output = serde_json::to_string_pretty(&parsed)
                    .unwrap_or_else(|_| "Failed to serialize to JSON".to_string());

                let result = serde_json::json!({
                    "parsed": parsed,
                    "json_representation": json_output,
                    "valid": true,
                    "success": true
                });
                ExecutionResult::success(result.to_string())
            }
            Err(e) => {
                let result = serde_json::json!({
                    "valid": false,
                    "error": e.to_string(),
                    "success": false
                });
                ExecutionResult::success(result.to_string())
            }
        }
    }
}

/// Helper function to extract values from JSON using simple dot notation
fn extract_json_path(value: &JsonValue, path: &str) -> Option<JsonValue> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = value;

    for part in parts {
        if part.is_empty() {
            continue;
        }

        // Handle array indices
        if let Ok(index) = part.parse::<usize>() {
            if let Some(array_value) = current.get(index) {
                current = array_value;
            } else {
                return None;
            }
        } else if let Some(obj_value) = current.get(part) {
            current = obj_value;
        } else {
            return None;
        }
    }

    Some(current.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_path_extraction() {
        let json = serde_json::json!({
            "user": {
                "name": "Alice",
                "age": 30,
                "hobbies": ["reading", "coding"]
            }
        });

        assert_eq!(
            extract_json_path(&json, "user.name"),
            Some(JsonValue::String("Alice".to_string()))
        );

        assert_eq!(
            extract_json_path(&json, "user.hobbies.0"),
            Some(JsonValue::String("reading".to_string()))
        );

        assert_eq!(extract_json_path(&json, "user.invalid"), None);
    }
}
