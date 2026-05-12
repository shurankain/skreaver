//! Anomaly detection for guardrail escalation.
//!
//! Provides a pluggable `AnomalyDetector` trait and a built-in
//! `ThresholdDetector` that escalates threat levels after repeated
//! guardrail denials within a time window.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Threat level for dynamic policy selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThreatLevel {
    Normal,
    Elevated,
    High,
    Critical,
}

impl Default for ThreatLevel {
    fn default() -> Self {
        Self::Normal
    }
}

/// Scored threat assessment from an anomaly detector.
#[derive(Debug, Clone)]
pub struct ThreatScore {
    pub level: ThreatLevel,
    pub confidence: f64,
}

/// Event fed to the anomaly detector on guardrail denials.
#[derive(Debug, Clone)]
pub struct AnomalyEvent {
    pub agent_id: String,
    pub event_type: AnomalyEventType,
    pub timestamp: Instant,
}

/// Types of events that feed anomaly detection.
#[derive(Debug, Clone)]
pub enum AnomalyEventType {
    /// A rule denied a message.
    RuleDenied { rule_name: String },
    /// Rate limit was exceeded.
    RateExceeded,
    /// Suspicious input detected.
    SuspiciousInput,
}

/// Trait for pluggable anomaly detection.
///
/// Implementations analyze events and return a threat score.
/// The built-in `ThresholdDetector` counts denials in a sliding window.
pub trait AnomalyDetector: Send + Sync {
    fn analyze(&self, event: &AnomalyEvent) -> ThreatScore;
}

/// Built-in anomaly detector that escalates based on denial frequency.
///
/// Counts events in a sliding time window and maps the count to a threat level:
/// - < threshold: Normal
/// - >= threshold: Elevated
/// - >= 2x threshold: High
/// - >= 3x threshold: Critical
pub struct ThresholdDetector {
    threshold: u32,
    window: Duration,
    events: Mutex<Vec<Instant>>,
}

impl ThresholdDetector {
    /// Create a new threshold detector.
    ///
    /// `threshold` is the number of events that triggers `Elevated`.
    /// `window` is the sliding time window for counting.
    pub fn new(threshold: u32, window: Duration) -> Self {
        Self {
            threshold,
            window,
            events: Mutex::new(Vec::new()),
        }
    }

    /// Default: 5 events in 5 minutes triggers escalation.
    pub fn default_config() -> Self {
        Self::new(5, Duration::from_secs(300))
    }
}

impl AnomalyDetector for ThresholdDetector {
    fn analyze(&self, event: &AnomalyEvent) -> ThreatScore {
        let mut events = self.events.lock().unwrap_or_else(|e| e.into_inner());
        let now = event.timestamp;

        // Add current event
        events.push(now);

        // Prune events outside the window
        events.retain(|&t| now.duration_since(t) < self.window);

        let count = events.len() as u32;

        let level = if count >= self.threshold * 3 {
            ThreatLevel::Critical
        } else if count >= self.threshold * 2 {
            ThreatLevel::High
        } else if count >= self.threshold {
            ThreatLevel::Elevated
        } else {
            ThreatLevel::Normal
        };

        ThreatScore {
            level,
            confidence: (count as f64 / (self.threshold * 3) as f64).min(1.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_below_threshold_is_normal() {
        let detector = ThresholdDetector::new(5, Duration::from_secs(60));
        let now = Instant::now();
        let event = AnomalyEvent {
            agent_id: "agent-1".to_string(),
            event_type: AnomalyEventType::RuleDenied {
                rule_name: "test".to_string(),
            },
            timestamp: now,
        };
        let score = detector.analyze(&event);
        assert_eq!(score.level, ThreatLevel::Normal);
    }

    #[test]
    fn test_threshold_escalation() {
        let detector = ThresholdDetector::new(3, Duration::from_secs(60));
        let now = Instant::now();

        for _ in 0..3 {
            let event = AnomalyEvent {
                agent_id: "agent-1".to_string(),
                event_type: AnomalyEventType::RuleDenied {
                    rule_name: "test".to_string(),
                },
                timestamp: now,
            };
            detector.analyze(&event);
        }

        // 4th event should be Elevated (count=4, threshold=3)
        let score = detector.analyze(&AnomalyEvent {
            agent_id: "agent-1".to_string(),
            event_type: AnomalyEventType::SuspiciousInput,
            timestamp: now,
        });
        assert_eq!(score.level, ThreatLevel::Elevated);
    }

    #[test]
    fn test_high_and_critical() {
        let detector = ThresholdDetector::new(2, Duration::from_secs(60));
        let now = Instant::now();
        let event = || AnomalyEvent {
            agent_id: "a".to_string(),
            event_type: AnomalyEventType::RateExceeded,
            timestamp: now,
        };

        // Fill to 2x threshold (4 events) → High
        for _ in 0..4 {
            detector.analyze(&event());
        }
        let score = detector.analyze(&event());
        assert_eq!(score.level, ThreatLevel::High); // 5 events, threshold=2, 2x=4

        // Fill to 3x threshold (6 events) → Critical
        let score = detector.analyze(&event());
        assert_eq!(score.level, ThreatLevel::Critical); // 6 events, 3x=6
    }

    #[test]
    fn test_window_decay() {
        let detector = ThresholdDetector::new(3, Duration::from_millis(10));
        let old = Instant::now();

        // Add events at "old" time
        for _ in 0..5 {
            detector.analyze(&AnomalyEvent {
                agent_id: "a".to_string(),
                event_type: AnomalyEventType::RuleDenied {
                    rule_name: "x".to_string(),
                },
                timestamp: old,
            });
        }

        // Wait for window to expire
        std::thread::sleep(Duration::from_millis(15));

        // New event after window — old events pruned, back to Normal
        let score = detector.analyze(&AnomalyEvent {
            agent_id: "a".to_string(),
            event_type: AnomalyEventType::RuleDenied {
                rule_name: "x".to_string(),
            },
            timestamp: Instant::now(),
        });
        assert_eq!(score.level, ThreatLevel::Normal);
    }
}
