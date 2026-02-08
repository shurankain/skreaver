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
