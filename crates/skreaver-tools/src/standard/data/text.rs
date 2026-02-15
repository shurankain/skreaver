//! # Text Processing Tools
//!
//! This module provides tools for text manipulation, analysis, and transformation.

use crate::core::ToolConfig;
use serde::{Deserialize, Serialize};
use skreaver_core::{ExecutionResult, Tool};

/// Configuration for text processing operations
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TextConfig {
    pub text: String,
    #[serde(default)]
    pub delimiter: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub case_sensitive: bool,
}

impl TextConfig {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            delimiter: None,
            limit: None,
            case_sensitive: true,
        }
    }

    pub fn with_delimiter(mut self, delimiter: impl Into<String>) -> Self {
        self.delimiter = Some(delimiter.into());
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn case_insensitive(mut self) -> Self {
        self.case_sensitive = false;
        self
    }
}

impl ToolConfig for TextConfig {
    fn from_simple(input: String) -> Self {
        Self::new(input)
    }
}

/// Text uppercase conversion tool
#[derive(Debug)]
pub struct TextUppercaseTool;

impl TextUppercaseTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TextUppercaseTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for TextUppercaseTool {
    fn name(&self) -> &str {
        "text_uppercase"
    }

    fn call(&self, input: String) -> ExecutionResult {
        let config = TextConfig::parse(input);

        let result_text = config.text.to_uppercase();

        let result = serde_json::json!({
            "original": config.text,
            "result": result_text,
            "operation": "uppercase",
            "success": true
        });

        ExecutionResult::success(result.to_string())
    }
}

/// Text reverse tool
#[derive(Debug)]
pub struct TextReverseTool;

impl TextReverseTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TextReverseTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for TextReverseTool {
    fn name(&self) -> &str {
        "text_reverse"
    }

    fn call(&self, input: String) -> ExecutionResult {
        let config = TextConfig::parse(input);

        let result_text: String = config.text.chars().rev().collect();

        let result = serde_json::json!({
            "original": config.text,
            "result": result_text,
            "operation": "reverse",
            "success": true
        });

        ExecutionResult::success(result.to_string())
    }
}

/// Text splitting tool
#[derive(Debug)]
pub struct TextSplitTool;

impl TextSplitTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TextSplitTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for TextSplitTool {
    fn name(&self) -> &str {
        "text_split"
    }

    fn call(&self, input: String) -> ExecutionResult {
        let config: TextConfig = match serde_json::from_str(&input) {
            Ok(config) => config,
            Err(e) => return ExecutionResult::failure(format!("Invalid JSON config: {}", e)),
        };

        let delimiter = config.delimiter.as_deref().unwrap_or(" ");
        let parts: Vec<&str> = if config.limit.is_some() {
            config
                .text
                .splitn(config.limit.unwrap(), delimiter)
                .collect()
        } else {
            config.text.split(delimiter).collect()
        };

        let parts_strings: Vec<String> = parts.into_iter().map(|s| s.to_string()).collect();

        let result = serde_json::json!({
            "original": config.text,
            "delimiter": delimiter,
            "parts": parts_strings,
            "count": parts_strings.len(),
            "operation": "split",
            "success": true
        });

        ExecutionResult::success(result.to_string())
    }
}

/// Text analysis tool
#[derive(Debug)]
pub struct TextAnalyzeTool;

impl TextAnalyzeTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TextAnalyzeTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for TextAnalyzeTool {
    fn name(&self) -> &str {
        "text_analyze"
    }

    fn call(&self, input: String) -> ExecutionResult {
        let config = TextConfig::parse(input);

        let text = &config.text;
        let char_count = text.chars().count();
        let byte_count = text.len();
        let word_count = text.split_whitespace().count();
        let line_count = text.lines().count();

        // Character frequency analysis
        let mut char_freq = std::collections::HashMap::new();
        for ch in text.chars() {
            *char_freq.entry(ch).or_insert(0) += 1;
        }

        let result = serde_json::json!({
            "text": text,
            "analysis": {
                "character_count": char_count,
                "byte_count": byte_count,
                "word_count": word_count,
                "line_count": line_count,
                "is_empty": text.is_empty(),
                "is_ascii": text.is_ascii(),
                "char_frequency": char_freq
            },
            "operation": "analyze",
            "success": true
        });

        ExecutionResult::success(result.to_string())
    }
}

/// Text search tool
#[derive(Debug)]
pub struct TextSearchTool;

impl TextSearchTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TextSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for TextSearchTool {
    fn name(&self) -> &str {
        "text_search"
    }

    fn call(&self, input: String) -> ExecutionResult {
        #[derive(Deserialize)]
        struct SearchConfig {
            text: String,
            pattern: String,
            #[serde(default)]
            case_sensitive: bool,
        }

        let config: SearchConfig = match serde_json::from_str(&input) {
            Ok(config) => config,
            Err(e) => return ExecutionResult::failure(format!("Invalid JSON config: {}", e)),
        };

        let text = if config.case_sensitive {
            config.text.clone()
        } else {
            config.text.to_lowercase()
        };

        let pattern = if config.case_sensitive {
            config.pattern.clone()
        } else {
            config.pattern.to_lowercase()
        };

        let matches: Vec<usize> = text
            .match_indices(&pattern)
            .map(|(index, _)| index)
            .collect();

        let result = serde_json::json!({
            "text": config.text,
            "pattern": config.pattern,
            "case_sensitive": config.case_sensitive,
            "matches": matches,
            "match_count": matches.len(),
            "found": !matches.is_empty(),
            "operation": "search",
            "success": true
        });

        ExecutionResult::success(result.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skreaver_core::Tool;

    #[test]
    fn test_text_uppercase_success() {
        let tool = TextUppercaseTool::new();
        let result = tool.call("hello world".to_string());

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert_eq!(output["result"], "HELLO WORLD");
        assert_eq!(output["original"], "hello world");
        assert!(output["success"].as_bool().unwrap());
    }

    #[test]
    fn test_text_uppercase_json_input() {
        let tool = TextUppercaseTool::new();
        let input = serde_json::json!({"text": "mixed Case TEXT"}).to_string();
        let result = tool.call(input);

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert_eq!(output["result"], "MIXED CASE TEXT");
    }

    #[test]
    fn test_text_reverse_success() {
        let tool = TextReverseTool::new();
        let result = tool.call("hello".to_string());

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert_eq!(output["result"], "olleh");
        assert_eq!(output["operation"], "reverse");
    }

    #[test]
    fn test_text_reverse_unicode() {
        let tool = TextReverseTool::new();
        let result = tool.call("🎉🎊".to_string());

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert_eq!(output["result"], "🎊🎉");
    }

    #[test]
    fn test_text_split_default_delimiter() {
        let tool = TextSplitTool::new();
        let input = serde_json::json!({"text": "one two three"}).to_string();
        let result = tool.call(input);

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert_eq!(output["count"], 3);
        assert_eq!(output["parts"][0], "one");
        assert_eq!(output["parts"][2], "three");
    }

    #[test]
    fn test_text_split_custom_delimiter() {
        let tool = TextSplitTool::new();
        let input = serde_json::json!({
            "text": "a,b,c,d",
            "delimiter": ","
        })
        .to_string();
        let result = tool.call(input);

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert_eq!(output["count"], 4);
        assert_eq!(output["delimiter"], ",");
    }

    #[test]
    fn test_text_split_with_limit() {
        let tool = TextSplitTool::new();
        let input = serde_json::json!({
            "text": "a b c d e",
            "limit": 3
        })
        .to_string();
        let result = tool.call(input);

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert_eq!(output["count"], 3);
        assert_eq!(output["parts"][2], "c d e");
    }

    #[test]
    fn test_text_analyze_basic() {
        let tool = TextAnalyzeTool::new();
        let result = tool.call("Hello World".to_string());

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert_eq!(output["analysis"]["word_count"], 2);
        assert_eq!(output["analysis"]["character_count"], 11);
        assert!(!output["analysis"]["is_empty"].as_bool().unwrap());
    }

    #[test]
    fn test_text_analyze_multiline() {
        let tool = TextAnalyzeTool::new();
        let input = serde_json::json!({"text": "line1\nline2\nline3"}).to_string();
        let result = tool.call(input);

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert_eq!(output["analysis"]["line_count"], 3);
        assert_eq!(output["analysis"]["word_count"], 3);
    }

    #[test]
    fn test_text_analyze_empty() {
        let tool = TextAnalyzeTool::new();
        let result = tool.call("".to_string());

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert!(output["analysis"]["is_empty"].as_bool().unwrap());
        assert_eq!(output["analysis"]["word_count"], 0);
    }

    #[test]
    fn test_text_search_case_sensitive() {
        let tool = TextSearchTool::new();
        let input = serde_json::json!({
            "text": "Hello World Hello",
            "pattern": "Hello",
            "case_sensitive": true
        })
        .to_string();
        let result = tool.call(input);

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert_eq!(output["match_count"], 2);
        assert!(output["found"].as_bool().unwrap());
        assert_eq!(output["matches"][0], 0);
        assert_eq!(output["matches"][1], 12);
    }

    #[test]
    fn test_text_search_case_insensitive() {
        let tool = TextSearchTool::new();
        let input = serde_json::json!({
            "text": "Hello world HELLO",
            "pattern": "hello",
            "case_sensitive": false
        })
        .to_string();
        let result = tool.call(input);

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert_eq!(output["match_count"], 2);
        assert!(output["found"].as_bool().unwrap());
    }

    #[test]
    fn test_text_search_not_found() {
        let tool = TextSearchTool::new();
        let input = serde_json::json!({
            "text": "Hello World",
            "pattern": "xyz",
            "case_sensitive": true
        })
        .to_string();
        let result = tool.call(input);

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert_eq!(output["match_count"], 0);
        assert!(!output["found"].as_bool().unwrap());
    }

    #[test]
    fn test_text_config_builder() {
        let config = TextConfig::new("test")
            .with_delimiter(",")
            .with_limit(5)
            .case_insensitive();

        assert_eq!(config.text, "test");
        assert_eq!(config.delimiter, Some(",".to_string()));
        assert_eq!(config.limit, Some(5));
        assert!(!config.case_sensitive);
    }
}
