//! # Agent Status Management
//!
//! Type-safe agent status tracking with compile-time state validation
//! and proper state transition management.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Agent operational status with type safety
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    /// Agent is initializing and not yet ready
    #[default]
    Initializing,
    /// Agent is ready to receive observations
    Ready,
    /// Agent is currently processing an observation
    Processing {
        /// What the agent is currently working on
        current_task: String,
        /// When processing started
        started_at: chrono::DateTime<chrono::Utc>,
    },
    /// Agent is waiting for tool execution results
    WaitingForTools {
        /// Tools currently being executed
        pending_tools: Vec<String>,
        /// When tool execution started
        started_at: chrono::DateTime<chrono::Utc>,
    },
    /// Agent has completed processing and has a response ready
    Completed {
        /// When processing completed
        completed_at: chrono::DateTime<chrono::Utc>,
    },
    /// Agent encountered an error and cannot proceed
    Error {
        /// Error message
        error: String,
        /// When the error occurred
        occurred_at: chrono::DateTime<chrono::Utc>,
        /// Whether the error is recoverable
        recoverable: bool,
    },
    /// Agent is temporarily paused
    Paused {
        /// Reason for pausing
        reason: String,
        /// When paused
        paused_at: chrono::DateTime<chrono::Utc>,
    },
    /// Agent has been stopped and is no longer available
    Stopped {
        /// Reason for stopping
        reason: String,
        /// When stopped
        stopped_at: chrono::DateTime<chrono::Utc>,
    },
}

impl AgentStatus {
    /// Check if the agent can accept new observations
    pub fn can_accept_observations(&self) -> bool {
        matches!(self, AgentStatus::Ready | AgentStatus::Completed { .. })
    }

    /// Check if the agent is currently busy
    pub fn is_busy(&self) -> bool {
        matches!(
            self,
            AgentStatus::Processing { .. } | AgentStatus::WaitingForTools { .. }
        )
    }

    /// Check if the agent is in an error state
    pub fn is_error(&self) -> bool {
        matches!(self, AgentStatus::Error { .. })
    }

    /// Check if the agent is operational (not stopped or in unrecoverable error)
    pub fn is_operational(&self) -> bool {
        match self {
            AgentStatus::Stopped { .. } => false,
            AgentStatus::Error { recoverable, .. } => *recoverable,
            _ => true,
        }
    }

    /// Get a human-readable description of the status
    pub fn description(&self) -> String {
        match self {
            AgentStatus::Initializing => "Agent is starting up".to_string(),
            AgentStatus::Ready => "Agent is ready to process requests".to_string(),
            AgentStatus::Processing {
                current_task,
                started_at,
            } => {
                format!(
                    "Processing '{}' (started {})",
                    current_task,
                    humantime::format_duration(
                        chrono::Utc::now()
                            .signed_duration_since(*started_at)
                            .to_std()
                            .unwrap_or_default()
                    )
                )
            }
            AgentStatus::WaitingForTools {
                pending_tools,
                started_at,
            } => {
                format!(
                    "Waiting for {} tools: {} ({})",
                    pending_tools.len(),
                    pending_tools.join(", "),
                    humantime::format_duration(
                        chrono::Utc::now()
                            .signed_duration_since(*started_at)
                            .to_std()
                            .unwrap_or_default()
                    )
                )
            }
            AgentStatus::Completed { completed_at } => {
                format!(
                    "Completed processing ({})",
                    humantime::format_duration(
                        chrono::Utc::now()
                            .signed_duration_since(*completed_at)
                            .to_std()
                            .unwrap_or_default()
                    )
                )
            }
            AgentStatus::Error {
                error,
                occurred_at,
                recoverable,
            } => {
                format!(
                    "{} error: {} ({})",
                    if *recoverable { "Recoverable" } else { "Fatal" },
                    error,
                    humantime::format_duration(
                        chrono::Utc::now()
                            .signed_duration_since(*occurred_at)
                            .to_std()
                            .unwrap_or_default()
                    )
                )
            }
            AgentStatus::Paused { reason, paused_at } => {
                format!(
                    "Paused: {} ({})",
                    reason,
                    humantime::format_duration(
                        chrono::Utc::now()
                            .signed_duration_since(*paused_at)
                            .to_std()
                            .unwrap_or_default()
                    )
                )
            }
            AgentStatus::Stopped { reason, stopped_at } => {
                format!(
                    "Stopped: {} ({})",
                    reason,
                    humantime::format_duration(
                        chrono::Utc::now()
                            .signed_duration_since(*stopped_at)
                            .to_std()
                            .unwrap_or_default()
                    )
                )
            }
        }
    }

    /// Get the simple status name for API responses
    pub fn simple_name(&self) -> &'static str {
        match self {
            AgentStatus::Initializing => "initializing",
            AgentStatus::Ready => "ready",
            AgentStatus::Processing { .. } => "processing",
            AgentStatus::WaitingForTools { .. } => "waiting_for_tools",
            AgentStatus::Completed { .. } => "completed",
            AgentStatus::Error { .. } => "error",
            AgentStatus::Paused { .. } => "paused",
            AgentStatus::Stopped { .. } => "stopped",
        }
    }
}


impl fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.simple_name())
    }
}

/// Valid state transitions for agent status
#[derive(Debug, Clone)]
pub struct StatusTransition {
    pub from: AgentStatus,
    pub to: AgentStatus,
    pub reason: Option<String>,
}

impl StatusTransition {
    /// Check if a state transition is valid
    pub fn is_valid(from: &AgentStatus, to: &AgentStatus) -> bool {
        use AgentStatus::*;

        match (from, to) {
            // From Initializing
            (Initializing, Ready) => true,
            (Initializing, Error { .. }) => true,
            (Initializing, Stopped { .. }) => true,

            // From Ready
            (Ready, Processing { .. }) => true,
            (Ready, Paused { .. }) => true,
            (Ready, Stopped { .. }) => true,
            (Ready, Error { .. }) => true,

            // From Processing
            (Processing { .. }, WaitingForTools { .. }) => true,
            (Processing { .. }, Completed { .. }) => true,
            (Processing { .. }, Error { .. }) => true,
            (Processing { .. }, Stopped { .. }) => true,

            // From WaitingForTools
            (WaitingForTools { .. }, Processing { .. }) => true,
            (WaitingForTools { .. }, Completed { .. }) => true,
            (WaitingForTools { .. }, Error { .. }) => true,
            (WaitingForTools { .. }, Stopped { .. }) => true,

            // From Completed
            (Completed { .. }, Ready) => true,
            (Completed { .. }, Processing { .. }) => true,
            (Completed { .. }, Paused { .. }) => true,
            (Completed { .. }, Stopped { .. }) => true,

            // From Error (recoverable only)
            (
                Error {
                    recoverable: true, ..
                },
                Ready,
            ) => true,
            (
                Error {
                    recoverable: true, ..
                },
                Stopped { .. },
            ) => true,
            (Error { .. }, Stopped { .. }) => true, // Can always stop

            // From Paused
            (Paused { .. }, Ready) => true,
            (Paused { .. }, Stopped { .. }) => true,
            (Paused { .. }, Error { .. }) => true,

            // From Stopped (terminal state - no transitions allowed)
            (Stopped { .. }, _) => false,

            // Same state (no-op)
            (a, b) if std::mem::discriminant(a) == std::mem::discriminant(b) => true,

            // All other transitions are invalid
            _ => false,
        }
    }

    /// Create a new status transition with validation
    pub fn new(from: AgentStatus, to: AgentStatus, reason: Option<String>) -> Result<Self, String> {
        if Self::is_valid(&from, &to) {
            Ok(Self { from, to, reason })
        } else {
            Err(format!(
                "Invalid status transition from {} to {}",
                from.simple_name(),
                to.simple_name()
            ))
        }
    }
}

/// Agent status manager for tracking and validating state transitions
#[derive(Debug)]
pub struct AgentStatusManager {
    current_status: AgentStatus,
    status_history: Vec<StatusTransition>,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl AgentStatusManager {
    /// Create a new status manager with initial status
    pub fn new() -> Self {
        Self {
            current_status: AgentStatus::default(),
            status_history: Vec::new(),
            created_at: chrono::Utc::now(),
        }
    }

    /// Get the current status
    pub fn current_status(&self) -> &AgentStatus {
        &self.current_status
    }

    /// Get the status history
    pub fn status_history(&self) -> &[StatusTransition] {
        &self.status_history
    }

    /// Get when the agent was created
    pub fn created_at(&self) -> chrono::DateTime<chrono::Utc> {
        self.created_at
    }

    /// Attempt to transition to a new status
    pub fn transition_to(
        &mut self,
        new_status: AgentStatus,
        reason: Option<String>,
    ) -> Result<(), String> {
        let transition =
            StatusTransition::new(self.current_status.clone(), new_status.clone(), reason)?;

        self.status_history.push(transition);
        self.current_status = new_status;
        Ok(())
    }

    /// Transition to Ready status
    pub fn set_ready(&mut self) -> Result<(), String> {
        self.transition_to(
            AgentStatus::Ready,
            Some("Agent ready for processing".to_string()),
        )
    }

    /// Transition to Processing status
    pub fn set_processing(&mut self, task: String) -> Result<(), String> {
        self.transition_to(
            AgentStatus::Processing {
                current_task: task,
                started_at: chrono::Utc::now(),
            },
            Some("Started processing observation".to_string()),
        )
    }

    /// Transition to WaitingForTools status
    pub fn set_waiting_for_tools(&mut self, tools: Vec<String>) -> Result<(), String> {
        self.transition_to(
            AgentStatus::WaitingForTools {
                pending_tools: tools,
                started_at: chrono::Utc::now(),
            },
            Some("Waiting for tool execution".to_string()),
        )
    }

    /// Transition to Completed status
    pub fn set_completed(&mut self) -> Result<(), String> {
        self.transition_to(
            AgentStatus::Completed {
                completed_at: chrono::Utc::now(),
            },
            Some("Processing completed successfully".to_string()),
        )
    }

    /// Transition to Error status
    pub fn set_error(&mut self, error: String, recoverable: bool) -> Result<(), String> {
        self.transition_to(
            AgentStatus::Error {
                error: error.clone(),
                occurred_at: chrono::Utc::now(),
                recoverable,
            },
            Some(format!("Error occurred: {}", error)),
        )
    }

    /// Transition to Paused status
    pub fn set_paused(&mut self, reason: String) -> Result<(), String> {
        self.transition_to(
            AgentStatus::Paused {
                reason: reason.clone(),
                paused_at: chrono::Utc::now(),
            },
            Some(reason),
        )
    }

    /// Transition to Stopped status
    pub fn set_stopped(&mut self, reason: String) -> Result<(), String> {
        self.transition_to(
            AgentStatus::Stopped {
                reason: reason.clone(),
                stopped_at: chrono::Utc::now(),
            },
            Some(reason),
        )
    }
}

impl Default for AgentStatusManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_capabilities() {
        let ready = AgentStatus::Ready;
        assert!(ready.can_accept_observations());
        assert!(!ready.is_busy());
        assert!(!ready.is_error());
        assert!(ready.is_operational());

        let processing = AgentStatus::Processing {
            current_task: "test".to_string(),
            started_at: chrono::Utc::now(),
        };
        assert!(!processing.can_accept_observations());
        assert!(processing.is_busy());
        assert!(!processing.is_error());
        assert!(processing.is_operational());

        let error = AgentStatus::Error {
            error: "test error".to_string(),
            occurred_at: chrono::Utc::now(),
            recoverable: false,
        };
        assert!(!error.can_accept_observations());
        assert!(!error.is_busy());
        assert!(error.is_error());
        assert!(!error.is_operational());
    }

    #[test]
    fn test_valid_transitions() {
        assert!(StatusTransition::is_valid(
            &AgentStatus::Initializing,
            &AgentStatus::Ready
        ));
        assert!(StatusTransition::is_valid(
            &AgentStatus::Ready,
            &AgentStatus::Processing {
                current_task: "test".to_string(),
                started_at: chrono::Utc::now(),
            }
        ));
        assert!(!StatusTransition::is_valid(
            &AgentStatus::Stopped {
                reason: "test".to_string(),
                stopped_at: chrono::Utc::now(),
            },
            &AgentStatus::Ready
        ));
    }

    #[test]
    fn test_status_manager() {
        let mut manager = AgentStatusManager::new();

        assert_eq!(manager.current_status().simple_name(), "initializing");

        manager.set_ready().unwrap();
        assert_eq!(manager.current_status().simple_name(), "ready");

        manager.set_processing("test task".to_string()).unwrap();
        assert_eq!(manager.current_status().simple_name(), "processing");

        manager.set_completed().unwrap();
        assert_eq!(manager.current_status().simple_name(), "completed");

        assert_eq!(manager.status_history().len(), 3);
    }

    #[test]
    fn test_invalid_transition() {
        let mut manager = AgentStatusManager::new();

        // Try to go directly from Initializing to Completed (should fail)
        let result = manager.set_completed();
        assert!(result.is_err());

        // Status should remain unchanged
        assert_eq!(manager.current_status().simple_name(), "initializing");
    }
}
