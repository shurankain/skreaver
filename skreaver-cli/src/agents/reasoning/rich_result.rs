use serde::{Deserialize, Serialize};

/// Rich result with confidence and evidence tracking.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RichResult {
    pub summary: String,
    pub confidence: f32,
    pub evidence: Vec<String>,
}

impl RichResult {
    /// Create a high-confidence result preset.
    #[cfg(test)]
    pub fn high_confidence(summary: String, evidence: Vec<String>) -> Self {
        Self {
            summary,
            confidence: 0.9,
            evidence,
        }
    }

    /// Create a medium-confidence result preset.
    #[cfg(test)]
    pub fn medium_confidence(summary: String, evidence: Vec<String>) -> Self {
        Self {
            summary,
            confidence: 0.7,
            evidence,
        }
    }

    /// Create a low-confidence result preset.
    #[cfg(test)]
    pub fn low_confidence(summary: String, evidence: Vec<String>) -> Self {
        Self {
            summary,
            confidence: 0.4,
            evidence,
        }
    }
}
