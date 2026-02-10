//! # Skreaver
//!
//! Skreaver is a Rust-native coordination runtime for building modular AI agents.
//! It provides a flexible architecture for creating autonomous agents that can
//! reason, use tools, and maintain memory across interactions.
//!
//! ## Core Components
//!
//! - **[Agent]**: Core trait defining agent behavior with observation, action, and tool usage
//! - **[MemoryReader], [MemoryWriter]**: Persistent storage for agent state and context
//! - **[Tool]**: External capabilities that agents can invoke
//! - **[Coordinator]**: Runtime that orchestrates agent execution and tool dispatch
//!
//! ## Quick Start
//!
//! ```rust
//! use skreaver::{Agent, MemoryReader, MemoryWriter, MemoryUpdate, InMemoryMemory};
//! use skreaver::{InMemoryToolRegistry, ToolCall, ExecutionResult};
//! # use skreaver::runtime::Coordinator;
//!
//! // Define your agent
//! struct MyAgent {
//!     memory: InMemoryMemory,
//! }
//!
//! impl Agent for MyAgent {
//!     type Observation = String;
//!     type Action = String;
//!     type Error = std::convert::Infallible;
//!
//!     fn memory_reader(&self) -> &dyn MemoryReader {
//!         &self.memory
//!     }
//!
//!     fn memory_writer(&mut self) -> &mut dyn MemoryWriter {
//!         &mut self.memory
//!     }
//!
//!     fn observe(&mut self, input: String) {
//!         // Process observation
//!     }
//!
//!     fn act(&mut self) -> String {
//!         "Hello from agent".to_string()
//!     }
//!
//!     fn call_tools(&self) -> Vec<ToolCall> {
//!         Vec::new()
//!     }
//!
//!     fn handle_result(&mut self, result: ExecutionResult) {
//!         // Handle tool execution results
//!     }
//!
//!     fn update_context(&mut self, update: MemoryUpdate) {
//!         let _ = self.memory_writer().store(update);
//!     }
//! }
//! ```
//!
//! ## Architecture
//!
//! Skreaver follows a modular architecture where agents coordinate through a runtime
//! that manages tool execution and memory persistence. This enables building complex
//! AI systems with clear separation of concerns and robust error handling.

// ============================================================================
// Module aliases for namespaced access
// ============================================================================

pub use skreaver_core as core;
pub use skreaver_core::agent; // Re-export agent module for qualified access
pub use skreaver_http as http;
pub use skreaver_memory as memory;
pub use skreaver_tools as tools;

#[cfg(feature = "testing")]
pub use skreaver_testing as testing;

// ============================================================================
// Core types - Agent, Memory, Errors
// ============================================================================

// Agent trait and extensions
pub use skreaver_core::{
    Agent, CompleteState, InitialState, ProcessingState, SimpleComplete, SimpleInitial,
    SimpleProcessing, SimpleStatefulAgent, SimpleToolExecution, StatefulAgent,
    StatefulAgentAdapter, ToolExecutionState,
};

// Memory traits
pub use skreaver_core::{
    MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter, SnapshotableMemory, TransactionalMemory,
};

// In-memory implementation
pub use skreaver_core::InMemoryMemory;

// Error types
pub use skreaver_core::{SkreverError, SkreverResult};

// Metadata
pub use skreaver_core::{Metadata, MetadataBuilder, MetadataError, MetadataKey, MetadataValue};

// ============================================================================
// Identifiers - Type-safe validated identifiers
// ============================================================================

pub use skreaver_core::{AgentId, PrincipalId, RequestId, SessionId, ToolId, ValidationError};

// Deprecated - will be removed in v0.6.0
#[allow(deprecated)]
#[deprecated(
    since = "0.5.0",
    note = "Use `validation::ValidationError` instead. This type will be REMOVED in 0.6.0."
)]
pub use skreaver_core::IdValidationError;

// ============================================================================
// Tools - Execution and dispatch
// ============================================================================

pub use skreaver_core::{
    ExecutionResult, FailureReason, StandardTool, StructuredTool, StructuredToolAdapter, Tool,
    ToolCall, ToolDispatch, ToolInput,
};

// Structured tool results
pub use skreaver_core::{StructuredToolResult, ToolExecutionMetadata, ToolResultBuilder};

// Tool registry
pub use skreaver_tools::{
    InMemoryToolRegistry, InvalidToolName, SecureToolRegistry, ToolCallBuildError, ToolCallBuilder,
    ToolConfig, ToolName, ToolRegistry,
};

// Standard tools - I/O
pub use skreaver_tools::{DirectoryCreateTool, DirectoryListTool, FileReadTool, FileWriteTool};

// Standard tools - Network
pub use skreaver_tools::{HttpDeleteTool, HttpGetTool, HttpPostTool, HttpPutTool};

// Standard tools - Data
pub use skreaver_tools::{
    JsonParseTool, JsonTransformTool, TextAnalyzeTool, TextReverseTool, TextSearchTool,
    TextSplitTool, TextUppercaseTool, XmlParseTool,
};

// ============================================================================
// Collections
// ============================================================================

pub use skreaver_core::{EmptyQueueError, EmptyVecError, NonEmptyQueue, NonEmptyVec};

// ============================================================================
// Security
// ============================================================================

pub use skreaver_core::{
    DomainValidator, InputValidator, PathValidator, ResourceLimits, ResourceTracker, SecretBytes,
    SecretString, SecretValue, SecureFileSystem, SecurityConfig, SecurityContext, SecurityError,
    SecurityManager, SecurityPolicy, ValidatedPath, ValidatedUrl,
};

// Sanitization
pub use skreaver_core::{
    ContentSanitizer, DatabaseErrorSanitizer, SanitizeError, SanitizeIdentifier, SecretRedactor,
};

// ============================================================================
// Authentication
// ============================================================================

pub use skreaver_core::{
    ApiKey, ApiKeyConfig, ApiKeyManager, AuthContext, AuthError, AuthManager, AuthMethod,
    AuthMiddleware, AuthResult, AuthenticatedRequest, AuthenticationPolicy, CredentialStorage,
    InMemoryStorage, JwtClaims, JwtConfig, JwtManager, JwtToken, Permission, Principal, Role,
    RoleManager, SecureStorage, ToolPolicy,
};

// ============================================================================
// Database
// ============================================================================

pub use skreaver_core::{
    DatabaseName, HealthCheck, HealthReport, HealthStatus, HostAddress, PerformanceMetrics,
    PoolSize, PoolStatistics,
};

// ============================================================================
// Memory backends
// ============================================================================

pub use skreaver_memory::{FileMemory, NamespacedMemory};

// Memory admin operations
pub use skreaver_memory::{
    AppliedMigration, BackupFormat, BackupHandle, HealthSeverity, MemoryAdmin, MigrationStatus,
    PoolHealth,
};
// Note: HealthStatus already exported from skreaver_core::database::health

#[cfg(feature = "redis")]
pub use skreaver_memory::{RedisConfigBuilder, RedisMemory, ValidRedisConfig};

#[cfg(feature = "sqlite")]
pub use skreaver_memory::{Migration, MigrationEngine, PooledConnection, SqliteMemory, SqlitePool};

#[cfg(feature = "postgres")]
pub use skreaver_memory::{
    PostgresConfig, PostgresMemory, PostgresMigration, PostgresMigrationEngine, PostgresPool,
    PostgresPoolHealth,
};

// ============================================================================
// HTTP Runtime
// ============================================================================

pub use skreaver_http::runtime::{
    // Agent builders
    AdvancedAgentBuilder,
    AgentBuilder,
    AgentFactory,
    AgentFactoryError,
    AgentInstance,
    AgentObservation,
    AgentResponse,
    AgentSpec,
    AgentStatus,
    AgentStatusEnum,
    AgentStatusManager,
    AgentType,
    AnalyticsAgentBuilder,
    // Backpressure
    BackpressureConfig,
    BackpressureManager,
    // Config
    ConfigError,
    // Connection limits
    ConnectionLimitConfig,
    ConnectionStats,
    ConnectionTracker,
    // Coordinator
    Coordinator,
    CoordinatorTrait,
    // Delivery
    DeliveryError,
    EchoAgentBuilder,
    // Error handling
    ErrorResponse,
    // HTTP runtime
    HttpAgentRuntime,
    HttpRuntimeConfig,
    HttpRuntimeConfigBuilder,
    QueueMetrics,
    RequestIdExtension,
    RequestPriority,
    ResponseDelivery,
    RuntimeError,
    RuntimeResult,
    // Security (HTTP-specific - different from core SecurityConfig)
    SecretKey,
    // Shutdown
    request_id_middleware,
    shutdown_signal,
    shutdown_signal_with_timeout,
    shutdown_with_cleanup,
};
// Note: ApiKeyData uses same name but different type, access via http::runtime::ApiKeyData

// Re-export runtime module for qualified access
pub use skreaver_http::runtime;

// ============================================================================
// OpenAPI (conditional)
// ============================================================================

#[cfg(feature = "openapi")]
pub use skreaver_http::openapi;

// ============================================================================
// WebSocket (conditional)
// ============================================================================

#[cfg(feature = "websocket")]
pub use skreaver_http::websocket::{
    self, ConnectionInfo, WebSocketConfig, WebSocketManager, WsError, WsMessage,
    handlers as ws_handlers, protocol,
};

// ============================================================================
// Testing utilities (conditional)
// ============================================================================

#[cfg(feature = "testing")]
pub use skreaver_testing::*;
