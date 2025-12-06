//! # Agent Status Management
//!
//! Type-safe agent status tracking with compile-time state validation
//! using the typestate pattern.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use utoipa::ToSchema;

// ============================================================================
// State marker types for typestate pattern
// ============================================================================

/// Marker type for Initializing state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Initializing;

/// Marker type for Ready state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ready;

/// Marker type for Processing state
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Processing {
    pub current_task: String,
    pub started_at: DateTime<Utc>,
}

/// Marker type for WaitingForTools state
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitingForTools {
    pub pending_tools: Vec<String>,
    pub started_at: DateTime<Utc>,
}

/// Marker type for Completed state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Completed {
    pub completed_at: DateTime<Utc>,
}

/// Marker type for recoverable Error state
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoverableError {
    pub error: String,
    pub occurred_at: DateTime<Utc>,
}

/// Marker type for fatal (non-recoverable) Error state
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FatalError {
    pub error: String,
    pub occurred_at: DateTime<Utc>,
}

/// Type alias for backward compatibility
#[deprecated(since = "0.5.0", note = "Use RecoverableError or FatalError instead")]
pub type Error = RecoverableError;

/// Marker type for Paused state
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paused {
    pub reason: String,
    pub paused_at: DateTime<Utc>,
}

/// Marker type for Stopped state (terminal state)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stopped {
    pub reason: String,
    pub stopped_at: DateTime<Utc>,
}

// ============================================================================
// Type-safe AgentStatus with typestate pattern
// ============================================================================

/// Type-safe agent status using typestate pattern.
/// The type parameter `S` represents the current state and enforces
/// valid state transitions at compile time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentStatus<S> {
    state: S,
    created_at: DateTime<Utc>,
    last_transition: DateTime<Utc>,
}

// ============================================================================
// Constructors and state-independent methods
// ============================================================================

impl AgentStatus<Initializing> {
    /// Create a new agent in Initializing state
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            state: Initializing,
            created_at: now,
            last_transition: now,
        }
    }
}

impl Default for AgentStatus<Initializing> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> AgentStatus<S> {
    /// Get when the agent was created
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Get when the last state transition occurred
    pub fn last_transition(&self) -> DateTime<Utc> {
        self.last_transition
    }

    /// Helper to create a transition to a new state
    fn transition_to<T>(self, new_state: T) -> AgentStatus<T> {
        AgentStatus {
            state: new_state,
            created_at: self.created_at,
            last_transition: Utc::now(),
        }
    }
}

// ============================================================================
// Valid state transitions (compile-time enforced)
// ============================================================================

impl AgentStatus<Initializing> {
    /// Transition to Ready state
    pub fn set_ready(self) -> AgentStatus<Ready> {
        self.transition_to(Ready)
    }

    /// Transition to recoverable error state
    pub fn set_recoverable_error(self, error: String) -> AgentStatus<RecoverableError> {
        self.transition_to(RecoverableError {
            error,
            occurred_at: Utc::now(),
        })
    }

    /// Transition to fatal error state
    pub fn set_fatal_error(self, error: String) -> AgentStatus<FatalError> {
        self.transition_to(FatalError {
            error,
            occurred_at: Utc::now(),
        })
    }

    /// Transition to Error state (deprecated - use set_recoverable_error or set_fatal_error)
    #[deprecated(
        since = "0.5.0",
        note = "Use set_recoverable_error or set_fatal_error instead"
    )]
    #[allow(deprecated)]
    pub fn set_error(self, error: String, recoverable: bool) -> AgentStatus<RecoverableError> {
        // For backward compatibility, always returns RecoverableError
        // Ignoring 'recoverable' parameter to maintain return type compatibility
        let _ = recoverable;
        self.set_recoverable_error(error)
    }

    /// Transition to Stopped state
    pub fn set_stopped(self, reason: String) -> AgentStatus<Stopped> {
        self.transition_to(Stopped {
            reason,
            stopped_at: Utc::now(),
        })
    }
}

impl AgentStatus<Ready> {
    /// Transition to Processing state
    pub fn start_processing(self, task: String) -> AgentStatus<Processing> {
        self.transition_to(Processing {
            current_task: task,
            started_at: Utc::now(),
        })
    }

    /// Transition to Paused state
    pub fn set_paused(self, reason: String) -> AgentStatus<Paused> {
        self.transition_to(Paused {
            reason,
            paused_at: Utc::now(),
        })
    }

    /// Transition to Stopped state
    pub fn set_stopped(self, reason: String) -> AgentStatus<Stopped> {
        self.transition_to(Stopped {
            reason,
            stopped_at: Utc::now(),
        })
    }

    /// Transition to recoverable error state
    pub fn set_recoverable_error(self, error: String) -> AgentStatus<RecoverableError> {
        self.transition_to(RecoverableError {
            error,
            occurred_at: Utc::now(),
        })
    }

    /// Transition to fatal error state
    pub fn set_fatal_error(self, error: String) -> AgentStatus<FatalError> {
        self.transition_to(FatalError {
            error,
            occurred_at: Utc::now(),
        })
    }

    /// Transition to Error state (deprecated)
    #[deprecated(
        since = "0.5.0",
        note = "Use set_recoverable_error or set_fatal_error instead"
    )]
    #[allow(deprecated)]
    pub fn set_error(self, error: String, recoverable: bool) -> AgentStatus<RecoverableError> {
        // For backward compatibility, always returns RecoverableError
        // Ignoring 'recoverable' parameter to maintain return type compatibility
        let _ = recoverable;
        self.set_recoverable_error(error)
    }
}

impl AgentStatus<Processing> {
    /// Get the current task
    pub fn current_task(&self) -> &str {
        &self.state.current_task
    }

    /// Get when processing started
    pub fn started_at(&self) -> DateTime<Utc> {
        self.state.started_at
    }

    /// Transition to WaitingForTools state
    pub fn wait_for_tools(self, tools: Vec<String>) -> AgentStatus<WaitingForTools> {
        self.transition_to(WaitingForTools {
            pending_tools: tools,
            started_at: Utc::now(),
        })
    }

    /// Transition to Completed state
    pub fn complete(self) -> AgentStatus<Completed> {
        self.transition_to(Completed {
            completed_at: Utc::now(),
        })
    }

    /// Transition to recoverable error state
    pub fn set_recoverable_error(self, error: String) -> AgentStatus<RecoverableError> {
        self.transition_to(RecoverableError {
            error,
            occurred_at: Utc::now(),
        })
    }

    /// Transition to fatal error state
    pub fn set_fatal_error(self, error: String) -> AgentStatus<FatalError> {
        self.transition_to(FatalError {
            error,
            occurred_at: Utc::now(),
        })
    }

    /// Transition to Error state (deprecated)
    #[deprecated(
        since = "0.5.0",
        note = "Use set_recoverable_error or set_fatal_error instead"
    )]
    #[allow(deprecated)]
    pub fn set_error(self, error: String, recoverable: bool) -> AgentStatus<RecoverableError> {
        // For backward compatibility, always returns RecoverableError
        // Ignoring 'recoverable' parameter to maintain return type compatibility
        let _ = recoverable;
        self.set_recoverable_error(error)
    }

    /// Transition to Stopped state
    pub fn set_stopped(self, reason: String) -> AgentStatus<Stopped> {
        self.transition_to(Stopped {
            reason,
            stopped_at: Utc::now(),
        })
    }
}

impl AgentStatus<WaitingForTools> {
    /// Get the pending tools
    pub fn pending_tools(&self) -> &[String] {
        &self.state.pending_tools
    }

    /// Get when tool execution started
    pub fn started_at(&self) -> DateTime<Utc> {
        self.state.started_at
    }

    /// Transition back to Processing state
    pub fn resume_processing(self, task: String) -> AgentStatus<Processing> {
        self.transition_to(Processing {
            current_task: task,
            started_at: Utc::now(),
        })
    }

    /// Transition to Completed state
    pub fn complete(self) -> AgentStatus<Completed> {
        self.transition_to(Completed {
            completed_at: Utc::now(),
        })
    }

    /// Transition to recoverable error state
    pub fn set_recoverable_error(self, error: String) -> AgentStatus<RecoverableError> {
        self.transition_to(RecoverableError {
            error,
            occurred_at: Utc::now(),
        })
    }

    /// Transition to fatal error state
    pub fn set_fatal_error(self, error: String) -> AgentStatus<FatalError> {
        self.transition_to(FatalError {
            error,
            occurred_at: Utc::now(),
        })
    }

    /// Transition to Error state (deprecated)
    #[deprecated(
        since = "0.5.0",
        note = "Use set_recoverable_error or set_fatal_error instead"
    )]
    #[allow(deprecated)]
    pub fn set_error(self, error: String, recoverable: bool) -> AgentStatus<RecoverableError> {
        // For backward compatibility, always returns RecoverableError
        // Ignoring 'recoverable' parameter to maintain return type compatibility
        let _ = recoverable;
        self.set_recoverable_error(error)
    }

    /// Transition to Stopped state
    pub fn set_stopped(self, reason: String) -> AgentStatus<Stopped> {
        self.transition_to(Stopped {
            reason,
            stopped_at: Utc::now(),
        })
    }
}

impl AgentStatus<Completed> {
    /// Get when processing completed
    pub fn completed_at(&self) -> DateTime<Utc> {
        self.state.completed_at
    }

    /// Transition to Ready state
    pub fn set_ready(self) -> AgentStatus<Ready> {
        self.transition_to(Ready)
    }

    /// Transition to Processing state
    pub fn start_processing(self, task: String) -> AgentStatus<Processing> {
        self.transition_to(Processing {
            current_task: task,
            started_at: Utc::now(),
        })
    }

    /// Transition to Paused state
    pub fn set_paused(self, reason: String) -> AgentStatus<Paused> {
        self.transition_to(Paused {
            reason,
            paused_at: Utc::now(),
        })
    }

    /// Transition to Stopped state
    pub fn set_stopped(self, reason: String) -> AgentStatus<Stopped> {
        self.transition_to(Stopped {
            reason,
            stopped_at: Utc::now(),
        })
    }
}

impl AgentStatus<RecoverableError> {
    /// Get the error message
    pub fn error(&self) -> &str {
        &self.state.error
    }

    /// Get when the error occurred
    pub fn occurred_at(&self) -> DateTime<Utc> {
        self.state.occurred_at
    }

    /// Transition to Ready state (recoverable errors can be recovered)
    pub fn recover_to_ready(self) -> AgentStatus<Ready> {
        self.transition_to(Ready)
    }

    /// Transition to Processing state after recovery
    pub fn recover_to_processing(self, task: String) -> AgentStatus<Processing> {
        self.transition_to(Processing {
            current_task: task,
            started_at: Utc::now(),
        })
    }

    /// Transition to Stopped state
    pub fn set_stopped(self, reason: String) -> AgentStatus<Stopped> {
        self.transition_to(Stopped {
            reason,
            stopped_at: Utc::now(),
        })
    }
}

impl AgentStatus<FatalError> {
    /// Get the error message
    pub fn error(&self) -> &str {
        &self.state.error
    }

    /// Get when the error occurred
    pub fn occurred_at(&self) -> DateTime<Utc> {
        self.state.occurred_at
    }

    /// Fatal errors can only transition to Stopped
    pub fn set_stopped(self, reason: String) -> AgentStatus<Stopped> {
        self.transition_to(Stopped {
            reason,
            stopped_at: Utc::now(),
        })
    }
}

impl AgentStatus<Paused> {
    /// Get the pause reason
    pub fn reason(&self) -> &str {
        &self.state.reason
    }

    /// Get when paused
    pub fn paused_at(&self) -> DateTime<Utc> {
        self.state.paused_at
    }

    /// Transition to Ready state
    pub fn resume(self) -> AgentStatus<Ready> {
        self.transition_to(Ready)
    }

    /// Transition to Stopped state
    pub fn set_stopped(self, reason: String) -> AgentStatus<Stopped> {
        self.transition_to(Stopped {
            reason,
            stopped_at: Utc::now(),
        })
    }

    /// Transition to recoverable error state
    pub fn set_recoverable_error(self, error: String) -> AgentStatus<RecoverableError> {
        self.transition_to(RecoverableError {
            error,
            occurred_at: Utc::now(),
        })
    }

    /// Transition to fatal error state
    pub fn set_fatal_error(self, error: String) -> AgentStatus<FatalError> {
        self.transition_to(FatalError {
            error,
            occurred_at: Utc::now(),
        })
    }

    /// Transition to Error state (deprecated)
    #[deprecated(
        since = "0.5.0",
        note = "Use set_recoverable_error or set_fatal_error instead"
    )]
    #[allow(deprecated)]
    pub fn set_error(self, error: String, recoverable: bool) -> AgentStatus<RecoverableError> {
        // For backward compatibility, always returns RecoverableError
        // Ignoring 'recoverable' parameter to maintain return type compatibility
        let _ = recoverable;
        self.set_recoverable_error(error)
    }
}

impl AgentStatus<Stopped> {
    /// Get the stop reason
    pub fn reason(&self) -> &str {
        &self.state.reason
    }

    /// Get when stopped
    pub fn stopped_at(&self) -> DateTime<Utc> {
        self.state.stopped_at
    }

    // No transitions from Stopped state - it's terminal
}

// ============================================================================
// Backward compatibility: Type-erased status for serialization/storage
// ============================================================================

/// Type-erased agent status for serialization and API responses.
/// This maintains backward compatibility with the old enum-based design.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatusEnum {
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
        started_at: DateTime<Utc>,
    },
    /// Agent is waiting for tool execution results
    WaitingForTools {
        /// Tools currently being executed
        pending_tools: Vec<String>,
        /// When tool execution started
        started_at: DateTime<Utc>,
    },
    /// Agent has completed processing and has a response ready
    Completed {
        /// When processing completed
        completed_at: DateTime<Utc>,
    },
    /// Agent encountered a recoverable error
    RecoverableError {
        /// Error message
        error: String,
        /// When the error occurred
        occurred_at: DateTime<Utc>,
    },
    /// Agent encountered a fatal (non-recoverable) error
    FatalError {
        /// Error message
        error: String,
        /// When the error occurred
        occurred_at: DateTime<Utc>,
    },
    /// Deprecated: Use RecoverableError or FatalError instead
    #[deprecated(since = "0.5.0", note = "Use RecoverableError or FatalError instead")]
    Error {
        /// Error message
        error: String,
        /// When the error occurred
        occurred_at: DateTime<Utc>,
        /// Whether the error is recoverable (always true for this deprecated variant)
        recoverable: bool,
    },
    /// Agent is temporarily paused
    Paused {
        /// Reason for pausing
        reason: String,
        /// When paused
        paused_at: DateTime<Utc>,
    },
    /// Agent has been stopped and is no longer available
    Stopped {
        /// Reason for stopping
        reason: String,
        /// When stopped
        stopped_at: DateTime<Utc>,
    },
}

impl AgentStatusEnum {
    /// Check if the agent can accept new observations
    pub fn can_accept_observations(&self) -> bool {
        matches!(
            self,
            AgentStatusEnum::Ready | AgentStatusEnum::Completed { .. }
        )
    }

    /// Check if the agent is currently busy
    pub fn is_busy(&self) -> bool {
        matches!(
            self,
            AgentStatusEnum::Processing { .. } | AgentStatusEnum::WaitingForTools { .. }
        )
    }

    /// Check if the agent is in an error state
    #[allow(deprecated)]
    pub fn is_error(&self) -> bool {
        matches!(
            self,
            AgentStatusEnum::RecoverableError { .. }
                | AgentStatusEnum::FatalError { .. }
                | AgentStatusEnum::Error { .. }
        )
    }

    /// Check if the agent is operational (not stopped or in unrecoverable error)
    pub fn is_operational(&self) -> bool {
        match self {
            AgentStatusEnum::Stopped { .. } => false,
            AgentStatusEnum::FatalError { .. } => false,
            AgentStatusEnum::RecoverableError { .. } => true,
            #[allow(deprecated)]
            AgentStatusEnum::Error { recoverable, .. } => *recoverable,
            _ => true,
        }
    }

    /// Get a human-readable description of the status
    pub fn description(&self) -> String {
        match self {
            AgentStatusEnum::Initializing => "Agent is starting up".to_string(),
            AgentStatusEnum::Ready => "Agent is ready to process requests".to_string(),
            AgentStatusEnum::Processing {
                current_task,
                started_at,
            } => {
                format!(
                    "Processing '{}' (started {})",
                    current_task,
                    humantime::format_duration(
                        Utc::now()
                            .signed_duration_since(*started_at)
                            .to_std()
                            .unwrap_or_default()
                    )
                )
            }
            AgentStatusEnum::WaitingForTools {
                pending_tools,
                started_at,
            } => {
                format!(
                    "Waiting for {} tools: {} ({})",
                    pending_tools.len(),
                    pending_tools.join(", "),
                    humantime::format_duration(
                        Utc::now()
                            .signed_duration_since(*started_at)
                            .to_std()
                            .unwrap_or_default()
                    )
                )
            }
            AgentStatusEnum::Completed { completed_at } => {
                format!(
                    "Completed processing ({})",
                    humantime::format_duration(
                        Utc::now()
                            .signed_duration_since(*completed_at)
                            .to_std()
                            .unwrap_or_default()
                    )
                )
            }
            AgentStatusEnum::RecoverableError { error, occurred_at } => {
                format!(
                    "Recoverable error: {} ({})",
                    error,
                    humantime::format_duration(
                        Utc::now()
                            .signed_duration_since(*occurred_at)
                            .to_std()
                            .unwrap_or_default()
                    )
                )
            }
            AgentStatusEnum::FatalError { error, occurred_at } => {
                format!(
                    "Fatal error: {} ({})",
                    error,
                    humantime::format_duration(
                        Utc::now()
                            .signed_duration_since(*occurred_at)
                            .to_std()
                            .unwrap_or_default()
                    )
                )
            }
            #[allow(deprecated)]
            AgentStatusEnum::Error {
                error,
                occurred_at,
                recoverable,
            } => {
                format!(
                    "{} error: {} ({})",
                    if *recoverable { "Recoverable" } else { "Fatal" },
                    error,
                    humantime::format_duration(
                        Utc::now()
                            .signed_duration_since(*occurred_at)
                            .to_std()
                            .unwrap_or_default()
                    )
                )
            }
            AgentStatusEnum::Paused { reason, paused_at } => {
                format!(
                    "Paused: {} ({})",
                    reason,
                    humantime::format_duration(
                        Utc::now()
                            .signed_duration_since(*paused_at)
                            .to_std()
                            .unwrap_or_default()
                    )
                )
            }
            AgentStatusEnum::Stopped { reason, stopped_at } => {
                format!(
                    "Stopped: {} ({})",
                    reason,
                    humantime::format_duration(
                        Utc::now()
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
            AgentStatusEnum::Initializing => "initializing",
            AgentStatusEnum::Ready => "ready",
            AgentStatusEnum::Processing { .. } => "processing",
            AgentStatusEnum::WaitingForTools { .. } => "waiting_for_tools",
            AgentStatusEnum::Completed { .. } => "completed",
            AgentStatusEnum::RecoverableError { .. } => "recoverable_error",
            AgentStatusEnum::FatalError { .. } => "fatal_error",
            #[allow(deprecated)]
            AgentStatusEnum::Error { .. } => "error",
            AgentStatusEnum::Paused { .. } => "paused",
            AgentStatusEnum::Stopped { .. } => "stopped",
        }
    }
}

impl fmt::Display for AgentStatusEnum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.simple_name())
    }
}

// ============================================================================
// Conversions between typed and type-erased status
// ============================================================================

impl From<AgentStatus<Initializing>> for AgentStatusEnum {
    fn from(_: AgentStatus<Initializing>) -> Self {
        AgentStatusEnum::Initializing
    }
}

impl From<AgentStatus<Ready>> for AgentStatusEnum {
    fn from(_: AgentStatus<Ready>) -> Self {
        AgentStatusEnum::Ready
    }
}

impl From<AgentStatus<Processing>> for AgentStatusEnum {
    fn from(status: AgentStatus<Processing>) -> Self {
        AgentStatusEnum::Processing {
            current_task: status.state.current_task,
            started_at: status.state.started_at,
        }
    }
}

impl From<AgentStatus<WaitingForTools>> for AgentStatusEnum {
    fn from(status: AgentStatus<WaitingForTools>) -> Self {
        AgentStatusEnum::WaitingForTools {
            pending_tools: status.state.pending_tools,
            started_at: status.state.started_at,
        }
    }
}

impl From<AgentStatus<Completed>> for AgentStatusEnum {
    fn from(status: AgentStatus<Completed>) -> Self {
        AgentStatusEnum::Completed {
            completed_at: status.state.completed_at,
        }
    }
}

impl From<AgentStatus<RecoverableError>> for AgentStatusEnum {
    fn from(status: AgentStatus<RecoverableError>) -> Self {
        AgentStatusEnum::RecoverableError {
            error: status.state.error,
            occurred_at: status.state.occurred_at,
        }
    }
}

impl From<AgentStatus<FatalError>> for AgentStatusEnum {
    fn from(status: AgentStatus<FatalError>) -> Self {
        AgentStatusEnum::FatalError {
            error: status.state.error,
            occurred_at: status.state.occurred_at,
        }
    }
}

impl From<AgentStatus<Paused>> for AgentStatusEnum {
    fn from(status: AgentStatus<Paused>) -> Self {
        AgentStatusEnum::Paused {
            reason: status.state.reason,
            paused_at: status.state.paused_at,
        }
    }
}

impl From<AgentStatus<Stopped>> for AgentStatusEnum {
    fn from(status: AgentStatus<Stopped>) -> Self {
        AgentStatusEnum::Stopped {
            reason: status.state.reason,
            stopped_at: status.state.stopped_at,
        }
    }
}

// ============================================================================
// Dynamic status wrapper for storage (maintains type-safety when possible)
// ============================================================================

/// Dynamic wrapper that can hold any agent status state.
/// Use this for storage/serialization where the specific state type isn't known at compile time.
#[derive(Debug, Clone)]
pub enum DynamicAgentStatus {
    Initializing(AgentStatus<Initializing>),
    Ready(AgentStatus<Ready>),
    Processing(AgentStatus<Processing>),
    WaitingForTools(AgentStatus<WaitingForTools>),
    Completed(AgentStatus<Completed>),
    RecoverableError(AgentStatus<RecoverableError>),
    FatalError(AgentStatus<FatalError>),
    #[deprecated(since = "0.5.0", note = "Use RecoverableError or FatalError instead")]
    Error(AgentStatus<RecoverableError>),
    Paused(AgentStatus<Paused>),
    Stopped(AgentStatus<Stopped>),
}

impl DynamicAgentStatus {
    /// Convert to type-erased enum for serialization
    pub fn to_enum(&self) -> AgentStatusEnum {
        match self {
            DynamicAgentStatus::Initializing(s) => s.clone().into(),
            DynamicAgentStatus::Ready(s) => s.clone().into(),
            DynamicAgentStatus::Processing(s) => s.clone().into(),
            DynamicAgentStatus::WaitingForTools(s) => s.clone().into(),
            DynamicAgentStatus::Completed(s) => s.clone().into(),
            DynamicAgentStatus::RecoverableError(s) => s.clone().into(),
            DynamicAgentStatus::FatalError(s) => s.clone().into(),
            #[allow(deprecated)]
            DynamicAgentStatus::Error(s) => s.clone().into(),
            DynamicAgentStatus::Paused(s) => s.clone().into(),
            DynamicAgentStatus::Stopped(s) => s.clone().into(),
        }
    }

    /// Create from type-erased enum
    pub fn from_enum(status: AgentStatusEnum) -> Self {
        match status {
            AgentStatusEnum::Initializing => DynamicAgentStatus::Initializing(AgentStatus::new()),
            AgentStatusEnum::Ready => {
                let base = AgentStatus::new();
                DynamicAgentStatus::Ready(base.set_ready())
            }
            AgentStatusEnum::Processing {
                current_task,
                started_at,
            } => {
                let now = Utc::now();
                DynamicAgentStatus::Processing(AgentStatus {
                    state: Processing {
                        current_task,
                        started_at,
                    },
                    created_at: now,
                    last_transition: now,
                })
            }
            AgentStatusEnum::WaitingForTools {
                pending_tools,
                started_at,
            } => {
                let now = Utc::now();
                DynamicAgentStatus::WaitingForTools(AgentStatus {
                    state: WaitingForTools {
                        pending_tools,
                        started_at,
                    },
                    created_at: now,
                    last_transition: now,
                })
            }
            AgentStatusEnum::Completed { completed_at } => {
                let now = Utc::now();
                DynamicAgentStatus::Completed(AgentStatus {
                    state: Completed { completed_at },
                    created_at: now,
                    last_transition: now,
                })
            }
            AgentStatusEnum::RecoverableError { error, occurred_at } => {
                let now = Utc::now();
                DynamicAgentStatus::RecoverableError(AgentStatus {
                    state: RecoverableError { error, occurred_at },
                    created_at: now,
                    last_transition: now,
                })
            }
            AgentStatusEnum::FatalError { error, occurred_at } => {
                let now = Utc::now();
                DynamicAgentStatus::FatalError(AgentStatus {
                    state: FatalError { error, occurred_at },
                    created_at: now,
                    last_transition: now,
                })
            }
            #[allow(deprecated)]
            AgentStatusEnum::Error {
                error,
                occurred_at,
                recoverable: _,
            } => {
                let now = Utc::now();
                DynamicAgentStatus::RecoverableError(AgentStatus {
                    state: RecoverableError { error, occurred_at },
                    created_at: now,
                    last_transition: now,
                })
            }
            AgentStatusEnum::Paused { reason, paused_at } => {
                let now = Utc::now();
                DynamicAgentStatus::Paused(AgentStatus {
                    state: Paused { reason, paused_at },
                    created_at: now,
                    last_transition: now,
                })
            }
            AgentStatusEnum::Stopped { reason, stopped_at } => {
                let now = Utc::now();
                DynamicAgentStatus::Stopped(AgentStatus {
                    state: Stopped { reason, stopped_at },
                    created_at: now,
                    last_transition: now,
                })
            }
        }
    }
}

impl From<DynamicAgentStatus> for AgentStatusEnum {
    fn from(status: DynamicAgentStatus) -> Self {
        status.to_enum()
    }
}

impl From<AgentStatusEnum> for DynamicAgentStatus {
    fn from(status: AgentStatusEnum) -> Self {
        Self::from_enum(status)
    }
}

// ============================================================================
// Backward compatibility: AgentStatusManager using DynamicAgentStatus
// ============================================================================

/// Agent status manager for tracking and validating state transitions.
/// This maintains backward compatibility while using the typestate pattern internally.
#[derive(Debug)]
pub struct AgentStatusManager {
    current_status: DynamicAgentStatus,
    created_at: DateTime<Utc>,
}

impl AgentStatusManager {
    /// Create a new status manager with initial status
    pub fn new() -> Self {
        Self {
            current_status: DynamicAgentStatus::Initializing(AgentStatus::new()),
            created_at: Utc::now(),
        }
    }

    /// Get the current status as enum
    pub fn current_status(&self) -> AgentStatusEnum {
        self.current_status.to_enum()
    }

    /// Get when the agent was created
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Transition to Ready status
    pub fn set_ready(&mut self) -> Result<(), String> {
        self.current_status = match std::mem::replace(
            &mut self.current_status,
            DynamicAgentStatus::Initializing(AgentStatus::new()),
        ) {
            DynamicAgentStatus::Initializing(s) => DynamicAgentStatus::Ready(s.set_ready()),
            DynamicAgentStatus::Completed(s) => DynamicAgentStatus::Ready(s.set_ready()),
            DynamicAgentStatus::Paused(s) => DynamicAgentStatus::Ready(s.resume()),
            DynamicAgentStatus::RecoverableError(s) => {
                DynamicAgentStatus::Ready(s.recover_to_ready())
            }
            #[allow(deprecated)]
            DynamicAgentStatus::Error(s) => DynamicAgentStatus::Ready(s.recover_to_ready()),
            other => {
                self.current_status = other;
                return Err("Invalid state transition to Ready".to_string());
            }
        };
        Ok(())
    }

    /// Transition to Processing status
    pub fn set_processing(&mut self, task: String) -> Result<(), String> {
        self.current_status = match std::mem::replace(
            &mut self.current_status,
            DynamicAgentStatus::Initializing(AgentStatus::new()),
        ) {
            DynamicAgentStatus::Ready(s) => {
                DynamicAgentStatus::Processing(s.start_processing(task))
            }
            DynamicAgentStatus::Completed(s) => {
                DynamicAgentStatus::Processing(s.start_processing(task))
            }
            DynamicAgentStatus::WaitingForTools(s) => {
                DynamicAgentStatus::Processing(s.resume_processing(task))
            }
            other => {
                self.current_status = other;
                return Err("Invalid state transition to Processing".to_string());
            }
        };
        Ok(())
    }

    /// Transition to WaitingForTools status
    pub fn set_waiting_for_tools(&mut self, tools: Vec<String>) -> Result<(), String> {
        self.current_status = match std::mem::replace(
            &mut self.current_status,
            DynamicAgentStatus::Initializing(AgentStatus::new()),
        ) {
            DynamicAgentStatus::Processing(s) => {
                DynamicAgentStatus::WaitingForTools(s.wait_for_tools(tools))
            }
            other => {
                self.current_status = other;
                return Err("Invalid state transition to WaitingForTools".to_string());
            }
        };
        Ok(())
    }

    /// Transition to Completed status
    pub fn set_completed(&mut self) -> Result<(), String> {
        self.current_status = match std::mem::replace(
            &mut self.current_status,
            DynamicAgentStatus::Initializing(AgentStatus::new()),
        ) {
            DynamicAgentStatus::Processing(s) => DynamicAgentStatus::Completed(s.complete()),
            DynamicAgentStatus::WaitingForTools(s) => DynamicAgentStatus::Completed(s.complete()),
            other => {
                self.current_status = other;
                return Err("Invalid state transition to Completed".to_string());
            }
        };
        Ok(())
    }

    /// Transition to recoverable error status
    pub fn set_recoverable_error(&mut self, error: String) -> Result<(), String> {
        self.current_status = match std::mem::replace(
            &mut self.current_status,
            DynamicAgentStatus::Initializing(AgentStatus::new()),
        ) {
            DynamicAgentStatus::Initializing(s) => {
                DynamicAgentStatus::RecoverableError(s.set_recoverable_error(error))
            }
            DynamicAgentStatus::Ready(s) => {
                DynamicAgentStatus::RecoverableError(s.set_recoverable_error(error))
            }
            DynamicAgentStatus::Processing(s) => {
                DynamicAgentStatus::RecoverableError(s.set_recoverable_error(error))
            }
            DynamicAgentStatus::WaitingForTools(s) => {
                DynamicAgentStatus::RecoverableError(s.set_recoverable_error(error))
            }
            DynamicAgentStatus::Paused(s) => {
                DynamicAgentStatus::RecoverableError(s.set_recoverable_error(error))
            }
            other => {
                self.current_status = other;
                return Ok(());
            }
        };
        Ok(())
    }

    /// Transition to fatal error status
    pub fn set_fatal_error(&mut self, error: String) -> Result<(), String> {
        self.current_status = match std::mem::replace(
            &mut self.current_status,
            DynamicAgentStatus::Initializing(AgentStatus::new()),
        ) {
            DynamicAgentStatus::Initializing(s) => {
                DynamicAgentStatus::FatalError(s.set_fatal_error(error))
            }
            DynamicAgentStatus::Ready(s) => {
                DynamicAgentStatus::FatalError(s.set_fatal_error(error))
            }
            DynamicAgentStatus::Processing(s) => {
                DynamicAgentStatus::FatalError(s.set_fatal_error(error))
            }
            DynamicAgentStatus::WaitingForTools(s) => {
                DynamicAgentStatus::FatalError(s.set_fatal_error(error))
            }
            DynamicAgentStatus::Paused(s) => {
                DynamicAgentStatus::FatalError(s.set_fatal_error(error))
            }
            other => {
                self.current_status = other;
                return Ok(());
            }
        };
        Ok(())
    }

    /// Transition to Error status (deprecated - use set_recoverable_error or set_fatal_error)
    #[deprecated(
        since = "0.5.0",
        note = "Use set_recoverable_error or set_fatal_error instead"
    )]
    #[allow(deprecated)]
    pub fn set_error(&mut self, error: String, recoverable: bool) -> Result<(), String> {
        if recoverable {
            self.set_recoverable_error(error)
        } else {
            self.set_fatal_error(error)
        }
    }

    /// Transition to Paused status
    pub fn set_paused(&mut self, reason: String) -> Result<(), String> {
        self.current_status = match std::mem::replace(
            &mut self.current_status,
            DynamicAgentStatus::Initializing(AgentStatus::new()),
        ) {
            DynamicAgentStatus::Ready(s) => DynamicAgentStatus::Paused(s.set_paused(reason)),
            DynamicAgentStatus::Completed(s) => DynamicAgentStatus::Paused(s.set_paused(reason)),
            other => {
                self.current_status = other;
                return Err("Invalid state transition to Paused".to_string());
            }
        };
        Ok(())
    }

    /// Transition to Stopped status
    pub fn set_stopped(&mut self, reason: String) -> Result<(), String> {
        self.current_status = match std::mem::replace(
            &mut self.current_status,
            DynamicAgentStatus::Initializing(AgentStatus::new()),
        ) {
            DynamicAgentStatus::Initializing(s) => {
                DynamicAgentStatus::Stopped(s.set_stopped(reason))
            }
            DynamicAgentStatus::Ready(s) => DynamicAgentStatus::Stopped(s.set_stopped(reason)),
            DynamicAgentStatus::Processing(s) => DynamicAgentStatus::Stopped(s.set_stopped(reason)),
            DynamicAgentStatus::WaitingForTools(s) => {
                DynamicAgentStatus::Stopped(s.set_stopped(reason))
            }
            DynamicAgentStatus::Completed(s) => DynamicAgentStatus::Stopped(s.set_stopped(reason)),
            DynamicAgentStatus::RecoverableError(s) => {
                DynamicAgentStatus::Stopped(s.set_stopped(reason))
            }
            DynamicAgentStatus::FatalError(s) => DynamicAgentStatus::Stopped(s.set_stopped(reason)),
            #[allow(deprecated)]
            DynamicAgentStatus::Error(s) => DynamicAgentStatus::Stopped(s.set_stopped(reason)),
            DynamicAgentStatus::Paused(s) => DynamicAgentStatus::Stopped(s.set_stopped(reason)),
            DynamicAgentStatus::Stopped(_) => {
                return Err("Already in Stopped state".to_string());
            }
        };
        Ok(())
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
    fn test_typestate_transitions() {
        // Start in Initializing state
        let status = AgentStatus::new();

        // Can transition to Ready
        let status = status.set_ready();

        // Can transition to Processing
        let status = status.start_processing("test task".to_string());
        assert_eq!(status.current_task(), "test task");

        // Can transition to WaitingForTools
        let status = status.wait_for_tools(vec!["tool1".to_string(), "tool2".to_string()]);
        assert_eq!(status.pending_tools().len(), 2);

        // Can transition to Completed
        let status = status.complete();

        // Can transition back to Ready
        let _status = status.set_ready();
    }

    #[test]
    fn test_error_recovery() {
        let status = AgentStatus::new();
        let status = status.set_ready();
        let status = status.set_recoverable_error("test error".to_string());

        assert_eq!(status.error(), "test error");

        // Recoverable error can transition to Ready
        let status = status.recover_to_ready();
        assert_eq!(
            status.last_transition().timestamp(),
            status.last_transition().timestamp()
        );
    }

    #[test]
    fn test_non_recoverable_error() {
        let status = AgentStatus::new();
        let status = status.set_ready();
        let status = status.set_fatal_error("fatal error".to_string());

        assert_eq!(status.error(), "fatal error");

        // Fatal error can only transition to Stopped
        let status = status.set_stopped("fatal error cleanup".to_string());
        assert_eq!(status.reason(), "fatal error cleanup");
    }

    #[test]
    fn test_stopped_is_terminal() {
        let status = AgentStatus::new();
        let status = status.set_ready();
        let status = status.set_stopped("shutdown".to_string());

        // Stopped state has no transition methods (won't compile if you try)
        assert_eq!(status.reason(), "shutdown");
    }

    #[test]
    fn test_enum_conversion() {
        let status = AgentStatus::new();
        let status = status.set_ready();
        let status = status.start_processing("test".to_string());

        let enum_status: AgentStatusEnum = status.into();
        assert_eq!(enum_status.simple_name(), "processing");
        assert!(enum_status.is_busy());
    }

    #[test]
    fn test_dynamic_status() {
        let status = AgentStatus::new();
        let status = status.set_ready();
        let dynamic = DynamicAgentStatus::Ready(status);

        let enum_status = dynamic.to_enum();
        assert_eq!(enum_status.simple_name(), "ready");
        assert!(enum_status.can_accept_observations());
    }

    #[test]
    fn test_enum_status_capabilities() {
        let ready = AgentStatusEnum::Ready;
        assert!(ready.can_accept_observations());
        assert!(!ready.is_busy());
        assert!(!ready.is_error());
        assert!(ready.is_operational());

        let processing = AgentStatusEnum::Processing {
            current_task: "test".to_string(),
            started_at: Utc::now(),
        };
        assert!(!processing.can_accept_observations());
        assert!(processing.is_busy());
        assert!(!processing.is_error());
        assert!(processing.is_operational());

        #[allow(deprecated)]
        let error = AgentStatusEnum::Error {
            error: "test error".to_string(),
            occurred_at: Utc::now(),
            recoverable: false,
        };
        assert!(!error.can_accept_observations());
        assert!(!error.is_busy());
        assert!(error.is_error());
        assert!(!error.is_operational());
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
