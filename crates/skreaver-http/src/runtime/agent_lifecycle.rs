//! # Typestate Agent Lifecycle Management
//!
//! This module provides compile-time enforcement of agent lifecycle transitions
//! using the typestate pattern. This prevents invalid lifecycle operations like
//! starting an already-running agent or processing with an uninitialized agent.
//!
//! # Problem: Runtime Lifecycle Errors
//!
//! Without typestate enforcement, lifecycle errors can only be caught at runtime:
//!
//! ```ignore
//! let agent = create_agent();
//! agent.start(); // Runtime check: is initialized?
//! agent.start(); // Runtime error: already started!
//! agent.process(); // Runtime check: is started?
//! ```
//!
//! # Solution: Compile-Time Safety
//!
//! With typestates, invalid operations are compile errors:
//!
//! ```ignore
//! let agent = AgentLifecycle::new();
//! let agent = agent.initialize(config)?;  // AgentLifecycle<Initialized>
//! let agent = agent.start()?;              // AgentLifecycle<Running>
//! let result = agent.process(input);       // Only available on Running
//! // agent.start(); // <- COMPILE ERROR! Already in Running state
//! ```

use crate::runtime::agent_instance::{AgentId, AgentInstance, CoordinatorTrait};
use crate::runtime::agent_status::AgentStatusEnum;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::RwLock;

// ============================================================================
// Lifecycle State Markers
// ============================================================================

/// Agent has been created but not yet initialized
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Created;

/// Agent has been initialized with configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Initialized {
    pub initialized_at: DateTime<Utc>,
}

/// Agent is running and can process observations
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Running {
    pub started_at: DateTime<Utc>,
}

/// Agent has been paused and can be resumed
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paused {
    pub paused_at: DateTime<Utc>,
    pub reason: String,
}

/// Agent has been stopped and cannot be restarted
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Terminated {
    pub terminated_at: DateTime<Utc>,
    pub reason: String,
}

// ============================================================================
// Typestate Lifecycle Wrapper
// ============================================================================

/// Typestate wrapper for agent lifecycle management.
///
/// The type parameter `S` represents the current lifecycle state and
/// enforces valid transitions at compile time.
///
/// # Example
///
/// ```ignore
/// // Create agent in Created state
/// let lifecycle = AgentLifecycle::create(id, agent_type, coordinator);
///
/// // Initialize (Created -> Initialized)
/// let lifecycle = lifecycle.initialize()?;
///
/// // Start (Initialized -> Running)
/// let lifecycle = lifecycle.start()?;
///
/// // Process observations (only available in Running state)
/// let response = lifecycle.process_observation(input).await?;
///
/// // Pause (Running -> Paused)
/// let lifecycle = lifecycle.pause("Manual pause".into())?;
///
/// // Resume (Paused -> Running)
/// let lifecycle = lifecycle.resume()?;
///
/// // Terminate (any state -> Terminated)
/// let _lifecycle = lifecycle.terminate("Shutdown".into())?;
/// ```
pub struct AgentLifecycle<S> {
    /// The underlying agent instance
    instance: Arc<RwLock<AgentInstance>>,
    /// Current lifecycle state
    state: S,
}

// ============================================================================
// Lifecycle Errors
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleError {
    /// Agent failed to initialize
    InitializationFailed(String),
    /// Agent failed to start
    StartFailed(String),
    /// Operation failed while processing
    ProcessingFailed(String),
    /// Agent is already in the target state
    AlreadyInState(String),
}

impl std::fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LifecycleError::InitializationFailed(msg) => {
                write!(f, "Initialization failed: {}", msg)
            }
            LifecycleError::StartFailed(msg) => write!(f, "Start failed: {}", msg),
            LifecycleError::ProcessingFailed(msg) => write!(f, "Processing failed: {}", msg),
            LifecycleError::AlreadyInState(state) => {
                write!(f, "Already in state: {}", state)
            }
        }
    }
}

impl std::error::Error for LifecycleError {}

pub type LifecycleResult<T> = Result<T, LifecycleError>;

// ============================================================================
// Created State
// ============================================================================

impl AgentLifecycle<Created> {
    /// Create a new agent in the Created state
    pub fn create(
        id: AgentId,
        agent_type: String,
        coordinator: Box<dyn CoordinatorTrait + Send + Sync>,
    ) -> Self {
        let instance = AgentInstance::new(id, agent_type, coordinator);
        Self {
            instance: Arc::new(RwLock::new(instance)),
            state: Created,
        }
    }

    /// Initialize the agent (Created -> Initialized)
    ///
    /// This transition sets up the agent with necessary configuration
    /// and prepares it for processing.
    ///
    /// SECURITY: The instance lock is held for the entire initialization process
    /// to prevent race conditions where the agent could be modified during
    /// initialization (TOCTOU vulnerability). The typestate pattern guarantees
    /// this method can only be called on `AgentLifecycle<Created>`, preventing
    /// double initialization at compile time.
    pub async fn initialize(self) -> LifecycleResult<AgentLifecycle<Initialized>> {
        // SECURITY: Hold the instance lock for the entire initialization
        // to prevent race conditions and ensure atomic state transitions.
        // The typestate pattern (Created -> Initialized) already guarantees at
        // compile time that this can only be called once per agent.
        let instance = self.instance.write().await;

        // Set initializing status (atomic transition under lock)
        {
            let mut status = instance.status.write().await;
            *status = AgentStatusEnum::Initializing;
        }

        // Perform initialization logic here
        // Note: If initialization fails, we should set status back to a failed state
        // For now, just transition to initialized state

        // Set ready status (still holding instance lock - atomic with previous status change)
        {
            let mut status = instance.status.write().await;
            *status = AgentStatusEnum::Ready;
        }

        // Release the lock before returning
        drop(instance);

        Ok(AgentLifecycle {
            instance: self.instance,
            state: Initialized {
                initialized_at: Utc::now(),
            },
        })
    }
}

// ============================================================================
// Initialized State
// ============================================================================

impl AgentLifecycle<Initialized> {
    /// Start the agent (Initialized -> Running)
    ///
    /// This transition makes the agent ready to process observations.
    pub fn start(self) -> LifecycleResult<AgentLifecycle<Running>> {
        Ok(AgentLifecycle {
            instance: self.instance,
            state: Running {
                started_at: Utc::now(),
            },
        })
    }

    /// Terminate the agent (Initialized -> Terminated)
    pub fn terminate(self, reason: String) -> LifecycleResult<AgentLifecycle<Terminated>> {
        Ok(AgentLifecycle {
            instance: self.instance,
            state: Terminated {
                terminated_at: Utc::now(),
                reason,
            },
        })
    }
}

// ============================================================================
// Running State
// ============================================================================

impl AgentLifecycle<Running> {
    /// Process an observation (only available in Running state)
    ///
    /// This is the core operation that demonstrates the value of typestates:
    /// you CANNOT call this method unless the agent is in the Running state.
    pub async fn process_observation(&self, input: String) -> LifecycleResult<String> {
        let mut instance = self.instance.write().await;

        // Update activity timestamp
        {
            let mut last_activity = instance.last_activity.write().await;
            *last_activity = Utc::now();
        }

        // Increment observation count
        instance
            .observation_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        // Process the observation
        let response = instance.coordinator.step(input);

        Ok(response)
    }

    /// Get the underlying agent instance (read-only access)
    pub fn instance(&self) -> Arc<RwLock<AgentInstance>> {
        Arc::clone(&self.instance)
    }

    /// Pause the agent (Running -> Paused)
    pub fn pause(self, reason: String) -> LifecycleResult<AgentLifecycle<Paused>> {
        Ok(AgentLifecycle {
            instance: self.instance,
            state: Paused {
                paused_at: Utc::now(),
                reason,
            },
        })
    }

    /// Terminate the agent (Running -> Terminated)
    pub fn terminate(self, reason: String) -> LifecycleResult<AgentLifecycle<Terminated>> {
        Ok(AgentLifecycle {
            instance: self.instance,
            state: Terminated {
                terminated_at: Utc::now(),
                reason,
            },
        })
    }
}

// ============================================================================
// Paused State
// ============================================================================

impl AgentLifecycle<Paused> {
    /// Resume the agent (Paused -> Running)
    pub fn resume(self) -> LifecycleResult<AgentLifecycle<Running>> {
        Ok(AgentLifecycle {
            instance: self.instance,
            state: Running {
                started_at: Utc::now(),
            },
        })
    }

    /// Terminate the agent (Paused -> Terminated)
    pub fn terminate(self, reason: String) -> LifecycleResult<AgentLifecycle<Terminated>> {
        Ok(AgentLifecycle {
            instance: self.instance,
            state: Terminated {
                terminated_at: Utc::now(),
                reason,
            },
        })
    }

    /// Get the pause reason
    pub fn reason(&self) -> &str {
        &self.state.reason
    }
}

// ============================================================================
// Terminated State
// ============================================================================

impl AgentLifecycle<Terminated> {
    /// Get the termination reason
    pub fn reason(&self) -> &str {
        &self.state.reason
    }

    /// Get when the agent was terminated
    pub fn terminated_at(&self) -> DateTime<Utc> {
        self.state.terminated_at
    }

    // No transitions from Terminated state - it's a terminal state
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::coordinator::Coordinator;
    use skreaver_tools::InMemoryToolRegistry;

    fn create_test_coordinator() -> Box<dyn CoordinatorTrait + Send + Sync> {
        use std::collections::HashMap;

        let config = HashMap::new();
        let agent = crate::runtime::agent_builders::EchoAgent::new(config).unwrap();

        Box::new(Coordinator::new(agent, InMemoryToolRegistry::new()))
    }

    #[tokio::test]
    async fn test_lifecycle_transitions() {
        let id = AgentId::new_unchecked("test-agent");

        // Created state
        let lifecycle = AgentLifecycle::create(id, "test".to_string(), create_test_coordinator());

        // Initialize
        let lifecycle = lifecycle.initialize().await.unwrap();

        // Start
        let lifecycle = lifecycle.start().unwrap();

        // Process (only available in Running state)
        let response = lifecycle
            .process_observation("test input".to_string())
            .await
            .unwrap();
        assert!(!response.is_empty());

        // Terminate
        let lifecycle = lifecycle.terminate("Test complete".to_string()).unwrap();
        assert_eq!(lifecycle.reason(), "Test complete");
    }

    #[tokio::test]
    async fn test_pause_and_resume() {
        let id = AgentId::new_unchecked("pause-test");

        let lifecycle = AgentLifecycle::create(id, "test".to_string(), create_test_coordinator());
        let lifecycle = lifecycle.initialize().await.unwrap();
        let lifecycle = lifecycle.start().unwrap();

        // Pause
        let lifecycle = lifecycle.pause("Testing pause".to_string()).unwrap();
        assert_eq!(lifecycle.reason(), "Testing pause");

        // Resume
        let lifecycle = lifecycle.resume().unwrap();

        // Should be able to process again
        let _response = lifecycle
            .process_observation("after resume".to_string())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_terminate_from_paused() {
        let id = AgentId::new_unchecked("terminate-paused");

        let lifecycle = AgentLifecycle::create(id, "test".to_string(), create_test_coordinator());
        let lifecycle = lifecycle.initialize().await.unwrap();
        let lifecycle = lifecycle.start().unwrap();
        let lifecycle = lifecycle.pause("Pausing".to_string()).unwrap();

        // Can terminate from paused state
        let lifecycle = lifecycle.terminate("Done".to_string()).unwrap();
        assert_eq!(lifecycle.reason(), "Done");
    }
}
