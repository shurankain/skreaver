use crate::memory::MemoryReader;
use crate::tool::ToolCall;

/// Stateful agent trait using the typestate pattern for compile-time state safety.
///
/// This trait enables agents to have compile-time guarantees about their state transitions.
/// Unlike the traditional `Agent` trait which uses mutable memory access, `StatefulAgent`
/// uses immutable memory access through `MemoryReader` and returns new agent instances
/// with different state types for each transition.
///
/// # Type Safety Benefits
///
/// - Invalid state transitions are impossible (compile-time error)
/// - Methods only available in appropriate states
/// - Zero-cost abstractions - no runtime overhead
/// - Clear API that shows what operations are valid when
///
/// # State Lifecycle
///
/// The typical agent lifecycle follows this pattern:
/// 1. **Initial** → **Processing** (via `observe()`)
/// 2. **Processing** → **ToolExecution** (via `get_tool_calls()` + external execution)
/// 3. **ToolExecution** → **Processing** or **Complete** (via `handle_result()`)
/// 4. **Processing** → **Complete** (via `act()`)
///
/// # Example
///
/// ```rust
/// use skreaver_core::{SimpleStatefulAgent, SimpleInitial};
/// use skreaver_core::InMemoryMemory;
///
/// // Agent starts in Initial state
/// let memory = Box::new(InMemoryMemory::new());
/// let agent = SimpleStatefulAgent::new(memory);
///
/// // State transitions return new types - invalid transitions won't compile
/// let processing_agent = agent.observe("Hello world".to_string());
///
/// if processing_agent.needs_tools() {
///     let tool_agent = processing_agent.request_tools();
///     // ... execute tools externally ...
/// } else {
///     let complete_agent = processing_agent.complete_without_tools();
///     let response = complete_agent.act();
/// }
/// ```
pub trait StatefulAgent<State> {
    /// The type of observations this agent can process.
    type Observation;

    /// The type of actions this agent can produce.
    type Action;

    /// Get read-only access to the agent's memory system.
    ///
    /// This enables context retrieval and state queries without requiring
    /// exclusive mutable access, allowing for better concurrency and
    /// immutable state transitions.
    fn memory_reader(&self) -> &dyn MemoryReader;

    /// Get the current tool calls that need to be executed.
    ///
    /// Returns a vector of tool calls that the runtime should execute.
    /// The agent cannot progress to the next state until these tools
    /// have been executed and their results provided via `handle_result()`.
    ///
    /// # Returns
    ///
    /// Vector of tool calls to be executed, or empty vector if no tools needed
    fn get_tool_calls(&self) -> Vec<ToolCall>;

    /// Check if the agent has completed its processing.
    ///
    /// This is used by the runtime to determine if the agent can provide
    /// a final action or needs more processing/tool execution cycles.
    ///
    /// # Returns
    ///
    /// `true` if the agent is in a complete state and ready to act
    fn is_complete(&self) -> bool;
}

/// Marker trait for initial states that can accept observations.
///
/// This trait constraint ensures that only appropriate states can
/// transition via the `observe()` method.
pub trait InitialState {}

/// Marker trait for processing states that can handle tool results.
///
/// This trait constraint ensures that only appropriate states can
/// transition via the `handle_result()` method.
pub trait ProcessingState {}

/// Marker trait for states that can execute tools.
///
/// This trait constraint ensures that only appropriate states can
/// provide tool calls for execution.
pub trait ToolExecutionState {}

/// Marker trait for complete states that can produce actions.
///
/// This trait constraint ensures that only appropriate states can
/// transition via the `act()` method.
pub trait CompleteState {}

/// Deprecated: Use direct implementation pattern instead.
///
/// This trait was removed because its complex associated types made it
/// very difficult for users to implement. See module documentation for
/// the recommended pattern.
#[deprecated(
    since = "0.5.0",
    note = "Implement state transition methods directly on your agent type instead. See module docs for examples."
)]
pub trait StatefulAgentTransitions<State>: StatefulAgent<State> {}

// Note: StatefulAgentTransitions trait was removed in v0.5.0
//
// The previous trait had overly complex associated types that made it very
// difficult for users to implement correctly. Instead of a trait, we now
// recommend implementing state transitions directly on your agent type.
//
// # Recommended Pattern
//
// Implement concrete methods on your agent for each state transition:
//
// ```rust,ignore
// struct MyAgent<S> {
//     state: S,
//     memory: Box<dyn MemoryReader>,
// }
//
// // Implement StatefulAgent<S> for your agent
// impl<S> StatefulAgent<S> for MyAgent<S> {
//     type Observation = String;
//     type Action = String;
//
//     fn memory_reader(&self) -> &dyn MemoryReader {
//         self.memory.as_ref()
//     }
//
//     fn get_tool_calls(&self) -> Vec<ToolCall> {
//         vec![] // Return actual tool calls
//     }
//
//     fn is_complete(&self) -> bool {
//         false // Check if in complete state
//     }
// }
//
// // Transition from Initial to Processing
// impl MyAgent<InitialState> {
//     pub fn observe(self, input: String) -> MyAgent<ProcessingState> {
//         MyAgent {
//             state: ProcessingState { data: input },
//             memory: self.memory,
//         }
//     }
// }
//
// // Transition from Processing to Complete
// impl MyAgent<ProcessingState> {
//     pub fn handle_result(self, result: ExecutionResult) -> MyAgent<CompleteState> {
//         MyAgent {
//             state: CompleteState { result: self.state.data },
//             memory: self.memory,
//         }
//     }
// }
//
// // Extract final result
// impl MyAgent<CompleteState> {
//     pub fn act(self) -> String {
//         self.state.result
//     }
// }
// ```
//
// This pattern is much simpler and gives you full control over state transitions
// without fighting with complex trait bounds.

/// Adapter to bridge stateful agents with the legacy Agent trait.
///
/// This provides backward compatibility while allowing gradual migration
/// to the stateful agent pattern. The adapter handles the state management
/// internally while presenting the traditional Agent interface.
pub struct StatefulAgentAdapter<T> {
    inner: T,
    /// We store the state as a type-erased trait object since the legacy
    /// Agent trait doesn't support type-level state information
    _phantom: std::marker::PhantomData<()>,
}

impl<T> StatefulAgentAdapter<T> {
    /// Create a new adapter wrapping a stateful agent.
    ///
    /// # Parameters
    ///
    /// * `inner` - The stateful agent to wrap
    ///
    /// # Returns
    ///
    /// New adapter instance that implements the legacy Agent trait
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get a reference to the inner stateful agent.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Get a mutable reference to the inner stateful agent.
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

/// Helper trait for implementing memory access in stateful agents.
///
/// This trait provides a standard way to implement the memory_reader
/// method for agents that contain memory systems.
pub trait HasMemoryReader {
    /// Get the memory reader for this agent.
    fn memory_reader(&self) -> &dyn MemoryReader;
}

/// Default implementation of memory_reader for types with memory fields.
impl<T> HasMemoryReader for T
where
    T: AsRef<dyn MemoryReader>,
{
    fn memory_reader(&self) -> &dyn MemoryReader {
        self.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::ToolCall;
    use crate::{MemoryKey, MemoryReader};

    // Example state types for testing
    #[derive(Debug)]
    struct TestInitial;

    #[derive(Debug)]
    struct TestProcessing {
        data: String,
    }

    #[derive(Debug)]
    struct TestComplete {
        result: String,
    }

    // Marker trait implementations
    impl InitialState for TestInitial {}
    impl ProcessingState for TestProcessing {}
    impl CompleteState for TestComplete {}

    // Mock memory reader for testing
    struct MockMemoryReader;

    impl MemoryReader for MockMemoryReader {
        fn load(&self, _key: &MemoryKey) -> Result<Option<String>, crate::error::MemoryError> {
            Ok(None)
        }
    }

    // Example agent implementation that actually uses the state
    struct TestAgent<State> {
        memory: MockMemoryReader,
        state: State,
    }

    impl<State> StatefulAgent<State> for TestAgent<State> {
        type Observation = String;
        type Action = String;

        fn memory_reader(&self) -> &dyn MemoryReader {
            &self.memory
        }

        fn get_tool_calls(&self) -> Vec<ToolCall> {
            vec![]
        }

        fn is_complete(&self) -> bool {
            false // Default implementation
        }
    }

    // Specialized implementation that uses the state field
    impl TestAgent<TestProcessing> {
        fn get_data(&self) -> &str {
            &self.state.data
        }
    }

    impl TestAgent<TestComplete> {
        fn get_result(&self) -> &str {
            &self.state.result
        }
    }

    #[test]
    fn test_stateful_agent_creation() {
        let agent = TestAgent {
            memory: MockMemoryReader,
            state: TestInitial,
        };

        // Verify we can access memory reader
        let _memory = agent.memory_reader();

        // Verify we can get tool calls
        let _tools = agent.get_tool_calls();
        assert!(!agent.is_complete());
    }

    #[test]
    fn test_processing_state_usage() {
        let agent = TestAgent {
            memory: MockMemoryReader,
            state: TestProcessing {
                data: "test data".to_string(),
            },
        };

        // Use the state field through a method
        assert_eq!(agent.get_data(), "test data");
    }

    #[test]
    fn test_complete_state_usage() {
        let agent = TestAgent {
            memory: MockMemoryReader,
            state: TestComplete {
                result: "final result".to_string(),
            },
        };

        // Use the state field through a method
        assert_eq!(agent.get_result(), "final result");
    }

    #[test]
    fn test_adapter_creation() {
        let agent = TestAgent {
            memory: MockMemoryReader,
            state: TestInitial,
        };

        let adapter = StatefulAgentAdapter::new(agent);
        let _inner = adapter.inner();
    }
}
