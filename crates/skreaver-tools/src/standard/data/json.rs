//! # Data Processing Tools
//!
//! This module provides tools for processing and transforming structured data
//! including JSON and XML parsing, validation, and transformation.

use crate::core::ToolConfig;
use quick_xml::de::from_str as xml_from_str;
use serde::{Deserialize, Serialize};
use serde_json::{self, Value as JsonValue};
use skreaver_core::{ExecutionResult, Tool};

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

impl ToolConfig for DataConfig {
    fn from_simple(input: String) -> Self {
        Self::new(input)
    }
}

/// JSON parsing and validation tool
#[derive(Debug)]
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
        let config = DataConfig::parse(input);

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
#[derive(Debug)]
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
#[derive(Debug)]
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
        let config = DataConfig::parse(input);

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
    use skreaver_core::Tool;

    // ==================== DataConfig Tests ====================

    #[test]
    fn test_data_config_new() {
        let config = DataConfig::new(r#"{"key": "value"}"#);
        assert_eq!(config.input, r#"{"key": "value"}"#);
        assert!(config.path.is_none());
        assert!(config.format.is_none());
    }

    #[test]
    fn test_data_config_builder() {
        let config = DataConfig::new(r#"{"user": "alice"}"#)
            .with_path("user")
            .with_format("pretty");

        assert_eq!(config.input, r#"{"user": "alice"}"#);
        assert_eq!(config.path, Some("user".to_string()));
        assert_eq!(config.format, Some("pretty".to_string()));
    }

    #[test]
    fn test_data_config_from_simple() {
        let config = DataConfig::from_simple("test input".to_string());
        assert_eq!(config.input, "test input");
    }

    // ==================== JSON Path Extraction Tests ====================

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

    #[test]
    fn test_json_path_extraction_nested() {
        let json = serde_json::json!({
            "data": {
                "items": [
                    {"id": 1, "name": "first"},
                    {"id": 2, "name": "second"}
                ]
            }
        });

        assert_eq!(
            extract_json_path(&json, "data.items.1.name"),
            Some(JsonValue::String("second".to_string()))
        );

        assert_eq!(
            extract_json_path(&json, "data.items.0.id"),
            Some(JsonValue::Number(1.into()))
        );
    }

    #[test]
    fn test_json_path_extraction_empty_path() {
        let json = serde_json::json!({"key": "value"});
        // Empty path returns the original value
        assert_eq!(extract_json_path(&json, ""), Some(json.clone()));
    }

    // ==================== JsonParseTool Tests ====================

    #[test]
    fn test_json_parse_tool_name() {
        let tool = JsonParseTool::new();
        assert_eq!(tool.name(), "json_parse");
    }

    #[test]
    fn test_json_parse_tool_default() {
        let tool = JsonParseTool::default();
        assert_eq!(tool.name(), "json_parse");
    }

    #[test]
    fn test_json_parse_valid_json() {
        let tool = JsonParseTool::new();
        let input = r#"{"name": "Alice", "age": 30}"#;
        let result = tool.call(input.to_string());

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert!(output["valid"].as_bool().unwrap());
        assert!(output["success"].as_bool().unwrap());
        assert_eq!(output["parsed"]["name"], "Alice");
        assert_eq!(output["parsed"]["age"], 30);
    }

    #[test]
    fn test_json_parse_invalid_json() {
        let tool = JsonParseTool::new();
        let input = "not valid json {";
        let result = tool.call(input.to_string());

        assert!(result.is_success()); // Tool returns success with valid=false
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert!(!output["valid"].as_bool().unwrap());
        assert!(!output["success"].as_bool().unwrap());
        assert!(output["error"].as_str().is_some());
    }

    #[test]
    fn test_json_parse_with_pretty_format() {
        let tool = JsonParseTool::new();
        let config = serde_json::json!({
            "input": r#"{"key":"value"}"#,
            "format": "pretty"
        });
        let result = tool.call(config.to_string());

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert!(output["valid"].as_bool().unwrap());
        // Pretty format should have newlines/indentation
        let formatted = output["formatted"].as_str().unwrap();
        assert!(formatted.contains('\n') || formatted.contains("  "));
    }

    #[test]
    fn test_json_parse_array() {
        let tool = JsonParseTool::new();
        let input = r#"[1, 2, 3, "four", true]"#;
        let result = tool.call(input.to_string());

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert!(output["valid"].as_bool().unwrap());
        assert!(output["parsed"].is_array());
        assert_eq!(output["parsed"][0], 1);
        assert_eq!(output["parsed"][3], "four");
    }

    // ==================== JsonTransformTool Tests ====================

    #[test]
    fn test_json_transform_tool_name() {
        let tool = JsonTransformTool::new();
        assert_eq!(tool.name(), "json_transform");
    }

    #[test]
    fn test_json_transform_tool_default() {
        let tool = JsonTransformTool::default();
        assert_eq!(tool.name(), "json_transform");
    }

    #[test]
    fn test_json_transform_extract_path() {
        let tool = JsonTransformTool::new();
        let config = serde_json::json!({
            "input": r#"{"user": {"name": "Bob", "email": "bob@example.com"}}"#,
            "path": "user.name"
        });
        let result = tool.call(config.to_string());

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert!(output["success"].as_bool().unwrap());
        assert_eq!(output["extracted"], "Bob");
        assert_eq!(output["path"], "user.name");
    }

    #[test]
    fn test_json_transform_no_path() {
        let tool = JsonTransformTool::new();
        let config = serde_json::json!({
            "input": r#"{"status": "ok"}"#
        });
        let result = tool.call(config.to_string());

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert!(output["success"].as_bool().unwrap());
        // Without path, extracted should equal original
        assert_eq!(output["extracted"]["status"], "ok");
    }

    #[test]
    fn test_json_transform_path_not_found() {
        let tool = JsonTransformTool::new();
        let config = serde_json::json!({
            "input": r#"{"user": {"name": "Alice"}}"#,
            "path": "user.email"
        });
        let result = tool.call(config.to_string());

        assert!(result.is_failure());
        assert!(result.output().contains("not found"));
    }

    #[test]
    fn test_json_transform_invalid_config() {
        let tool = JsonTransformTool::new();
        let result = tool.call("not valid json config".to_string());

        assert!(result.is_failure());
        assert!(result.output().contains("Invalid config JSON"));
    }

    #[test]
    fn test_json_transform_invalid_input_json() {
        let tool = JsonTransformTool::new();
        let config = serde_json::json!({
            "input": "not valid json"
        });
        let result = tool.call(config.to_string());

        assert!(result.is_failure());
        assert!(result.output().contains("Invalid input JSON"));
    }

    #[test]
    fn test_json_transform_with_pretty_format() {
        let tool = JsonTransformTool::new();
        let config = serde_json::json!({
            "input": r#"{"data": {"nested": {"value": 42}}}"#,
            "path": "data",
            "format": "pretty"
        });
        let result = tool.call(config.to_string());

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert!(output["success"].as_bool().unwrap());
        let formatted = output["formatted"].as_str().unwrap();
        assert!(formatted.contains('\n') || formatted.contains("  "));
    }

    // ==================== XmlParseTool Tests ====================

    #[test]
    fn test_xml_parse_tool_name() {
        let tool = XmlParseTool::new();
        assert_eq!(tool.name(), "xml_parse");
    }

    #[test]
    fn test_xml_parse_tool_default() {
        let tool = XmlParseTool::default();
        assert_eq!(tool.name(), "xml_parse");
    }

    #[test]
    fn test_xml_parse_valid_xml() {
        let tool = XmlParseTool::new();
        let input = r#"<root><name>Alice</name><age>30</age></root>"#;
        let result = tool.call(input.to_string());

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert!(output["valid"].as_bool().unwrap());
        assert!(output["success"].as_bool().unwrap());
        // Parsed XML should be available
        assert!(output["parsed"].is_object() || output["parsed"].is_string());
    }

    #[test]
    fn test_xml_parse_invalid_xml() {
        let tool = XmlParseTool::new();
        let input = "<root><unclosed>";
        let result = tool.call(input.to_string());

        assert!(result.is_success()); // Tool returns success with valid=false
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert!(!output["valid"].as_bool().unwrap());
        assert!(!output["success"].as_bool().unwrap());
        assert!(output["error"].as_str().is_some());
    }

    #[test]
    fn test_xml_parse_with_attributes() {
        let tool = XmlParseTool::new();
        let input = r#"<person id="123" status="active">John</person>"#;
        let result = tool.call(input.to_string());

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert!(output["valid"].as_bool().unwrap());
        // JSON representation should be present
        assert!(output["json_representation"].as_str().is_some());
    }

    #[test]
    fn test_xml_parse_nested_elements() {
        let tool = XmlParseTool::new();
        let input = r#"<order><item><name>Widget</name><qty>5</qty></item></order>"#;
        let result = tool.call(input.to_string());

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert!(output["valid"].as_bool().unwrap());
        assert!(output["success"].as_bool().unwrap());
    }

    #[test]
    fn test_xml_parse_with_json_config() {
        let tool = XmlParseTool::new();
        let config = serde_json::json!({
            "input": "<data><value>42</value></data>"
        });
        let result = tool.call(config.to_string());

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert!(output["valid"].as_bool().unwrap());
    }

    // ==================== Debug Implementation Tests ====================

    #[test]
    fn test_tools_debug() {
        let json_parse = JsonParseTool::new();
        let debug_str = format!("{:?}", json_parse);
        assert!(debug_str.contains("JsonParseTool"));

        let json_transform = JsonTransformTool::new();
        let debug_str = format!("{:?}", json_transform);
        assert!(debug_str.contains("JsonTransformTool"));

        let xml_parse = XmlParseTool::new();
        let debug_str = format!("{:?}", xml_parse);
        assert!(debug_str.contains("XmlParseTool"));
    }
}
