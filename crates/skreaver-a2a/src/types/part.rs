//! Content part types for the A2A protocol.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A content part within a message or artifact
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Part {
    /// Text content
    #[serde(rename = "text")]
    Text(TextPart),

    /// File reference
    #[serde(rename = "file")]
    File(FilePart),

    /// Structured data
    #[serde(rename = "data")]
    Data(DataPart),
}

impl Part {
    /// Create a text part
    pub fn text(content: impl Into<String>) -> Self {
        Part::Text(TextPart {
            text: content.into(),
            metadata: HashMap::new(),
        })
    }

    /// Create a file part
    pub fn file(uri: impl Into<String>, media_type: impl Into<String>) -> Self {
        Part::File(FilePart {
            uri: uri.into(),
            media_type: media_type.into(),
            name: None,
            metadata: HashMap::new(),
        })
    }

    /// Create a data part
    pub fn data(data: serde_json::Value, media_type: impl Into<String>) -> Self {
        Part::Data(DataPart {
            data,
            media_type: media_type.into(),
            metadata: HashMap::new(),
        })
    }

    /// Get the text content if this is a text part
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Part::Text(t) => Some(&t.text),
            _ => None,
        }
    }
}

/// Text content part
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextPart {
    /// The text content
    pub text: String,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// File reference part
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePart {
    /// URI to the file
    pub uri: String,

    /// MIME type of the file
    pub media_type: String,

    /// Optional file name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Structured data part
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataPart {
    /// The structured data
    pub data: serde_json::Value,

    /// MIME type of the data (e.g., "application/json")
    pub media_type: String,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}
