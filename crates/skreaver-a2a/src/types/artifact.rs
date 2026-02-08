//! Artifact types for the A2A protocol.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::Part;

/// An artifact produced as output from a task
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    /// Unique identifier for the artifact
    pub id: String,

    /// Content parts of the artifact
    pub parts: Vec<Part>,

    /// MIME type of the artifact
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,

    /// Human-readable label
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,

    /// Description of the artifact
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Artifact {
    /// Create a new artifact with the given ID
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            parts: Vec::new(),
            media_type: None,
            label: None,
            description: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a new artifact with a generated UUID
    pub fn new_with_uuid() -> Self {
        Self::new(Uuid::new_v4().to_string())
    }

    /// Create a text artifact
    pub fn text(id: impl Into<String>, content: impl Into<String>) -> Self {
        let mut artifact = Self::new(id);
        artifact.parts.push(Part::text(content));
        artifact.media_type = Some("text/plain".to_string());
        artifact
    }

    /// Create a JSON artifact
    pub fn json(id: impl Into<String>, data: serde_json::Value) -> Self {
        let mut artifact = Self::new(id);
        artifact.parts.push(Part::data(data, "application/json"));
        artifact.media_type = Some("application/json".to_string());
        artifact
    }

    /// Add a part to the artifact
    pub fn with_part(mut self, part: Part) -> Self {
        self.parts.push(part);
        self
    }

    /// Set the label
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}
