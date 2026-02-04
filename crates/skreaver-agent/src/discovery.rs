//! Agent Discovery Service
//!
//! This module provides a comprehensive agent discovery system for the Skreaver platform.
//! It enables agents to register themselves, be discovered by other agents, and
//! query for agents based on capabilities, protocols, or custom filters.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    DiscoveryService                         │
//! │  ┌─────────────────┐     ┌─────────────────────────────┐   │
//! │  │   Registration  │     │         Query API           │   │
//! │  │     API         │     │  - by ID                    │   │
//! │  │  - register     │     │  - by protocol              │   │
//! │  │  - deregister   │     │  - by capability            │   │
//! │  │  - heartbeat    │     │  - by tags                  │   │
//! │  └────────┬────────┘     └─────────────┬───────────────┘   │
//! │           │                            │                    │
//! │           ▼                            ▼                    │
//! │  ┌─────────────────────────────────────────────────────┐   │
//! │  │              DiscoveryProvider                       │   │
//! │  │  ┌─────────┐  ┌─────────┐  ┌─────────────────────┐  │   │
//! │  │  │InMemory │  │  HTTP   │  │   Custom Provider   │  │   │
//! │  │  │Provider │  │ Client  │  │                     │  │   │
//! │  │  └─────────┘  └─────────┘  └─────────────────────┘  │   │
//! │  └─────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Features
//!
//! - **Agent Registration**: Agents can register with metadata, capabilities, and health endpoints
//! - **Health Checking**: Automatic health monitoring with configurable intervals
//! - **Filtering**: Query agents by protocol, capability, tags, or custom predicates
//! - **Event Notifications**: Subscribe to agent registration/deregistration events
//! - **Multiple Providers**: Support for in-memory, HTTP-based, and custom discovery providers
//!
//! # Example
//!
//! ```rust,ignore
//! use skreaver_agent::{DiscoveryService, AgentRegistration, DiscoveryQuery};
//!
//! // Create a discovery service
//! let discovery = DiscoveryService::new();
//!
//! // Register an agent
//! let registration = AgentRegistration::new("my-agent", "My Agent")
//!     .with_protocol(Protocol::A2a)
//!     .with_capability("search")
//!     .with_endpoint("https://agent.example.com")
//!     .with_tag("production");
//!
//! discovery.register(registration).await?;
//!
//! // Query for agents
//! let agents = discovery.query(
//!     DiscoveryQuery::new()
//!         .with_capability("search")
//!         .with_protocol(Protocol::A2a)
//! ).await?;
//!
//! for agent in agents {
//!     println!("Found agent: {} at {}", agent.name, agent.endpoint.unwrap_or_default());
//! }
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, broadcast};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::error::{AgentError, AgentResult};
use crate::traits::UnifiedAgent;
use crate::types::{AgentInfo, Capability, Protocol};

// ============================================================================
// Core Types
// ============================================================================

/// Registration information for an agent in the discovery service.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentRegistration {
    /// Unique identifier for this registration
    pub id: String,
    /// Agent ID (may differ from registration ID for multiple instances)
    pub agent_id: String,
    /// Human-readable name
    pub name: String,
    /// Description of the agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Version string
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Protocols supported by this agent
    pub protocols: Vec<Protocol>,
    /// Capabilities/skills offered by this agent
    pub capabilities: Vec<Capability>,
    /// Primary endpoint URL for the agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// Health check endpoint (defaults to endpoint + /health)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_endpoint: Option<String>,
    /// Tags for categorization and filtering
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
    /// When the agent was registered
    pub registered_at: DateTime<Utc>,
    /// Last heartbeat time
    pub last_heartbeat: DateTime<Utc>,
    /// Registration TTL - agent is considered stale after this duration without heartbeat
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u64>,
    /// Current health status
    pub health_status: HealthStatus,
}

impl AgentRegistration {
    /// Create a new agent registration.
    pub fn new(agent_id: impl Into<String>, name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            agent_id: agent_id.into(),
            name: name.into(),
            description: None,
            version: None,
            protocols: Vec::new(),
            capabilities: Vec::new(),
            endpoint: None,
            health_endpoint: None,
            tags: Vec::new(),
            metadata: HashMap::new(),
            registered_at: now,
            last_heartbeat: now,
            ttl_seconds: Some(300), // Default 5 minute TTL
            health_status: HealthStatus::Unknown,
        }
    }

    /// Create registration from an existing UnifiedAgent.
    pub fn from_agent(agent: &dyn UnifiedAgent) -> Self {
        let info = agent.info();
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            agent_id: info.id.clone(),
            name: info.name.clone(),
            description: info.description.clone(),
            version: info.version.clone(),
            protocols: info.protocols.clone(),
            capabilities: info.capabilities.clone(),
            endpoint: info.url.clone(),
            health_endpoint: None,
            tags: Vec::new(),
            metadata: info.metadata.clone(),
            registered_at: now,
            last_heartbeat: now,
            ttl_seconds: Some(300),
            health_status: HealthStatus::Unknown,
        }
    }

    /// Set description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set version.
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Add a protocol.
    pub fn with_protocol(mut self, protocol: Protocol) -> Self {
        if !self.protocols.contains(&protocol) {
            self.protocols.push(protocol);
        }
        self
    }

    /// Add a capability.
    pub fn with_capability(mut self, capability: impl Into<String>) -> Self {
        let cap_id = capability.into();
        if !self.capabilities.iter().any(|c| c.id == cap_id) {
            self.capabilities.push(Capability::new(&cap_id, &cap_id));
        }
        self
    }

    /// Add a full capability object.
    pub fn with_capability_full(mut self, capability: Capability) -> Self {
        if !self.capabilities.iter().any(|c| c.id == capability.id) {
            self.capabilities.push(capability);
        }
        self
    }

    /// Set the endpoint URL.
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Set the health endpoint URL.
    pub fn with_health_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.health_endpoint = Some(endpoint.into());
        self
    }

    /// Add a tag.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        let tag_str = tag.into();
        if !self.tags.contains(&tag_str) {
            self.tags.push(tag_str);
        }
        self
    }

    /// Add metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Set TTL in seconds.
    pub fn with_ttl(mut self, seconds: u64) -> Self {
        self.ttl_seconds = Some(seconds);
        self
    }

    /// Check if the registration is stale (past TTL without heartbeat).
    pub fn is_stale(&self) -> bool {
        if let Some(ttl) = self.ttl_seconds {
            let elapsed = Utc::now()
                .signed_duration_since(self.last_heartbeat)
                .num_seconds();
            elapsed > ttl as i64
        } else {
            false // No TTL means never stale
        }
    }

    /// Update the heartbeat timestamp.
    pub fn heartbeat(&mut self) {
        self.last_heartbeat = Utc::now();
    }

    /// Convert to AgentInfo for use with UnifiedAgent interface.
    pub fn to_agent_info(&self) -> AgentInfo {
        let mut info = AgentInfo::new(&self.agent_id, &self.name);
        if let Some(desc) = &self.description {
            info = info.with_description(desc);
        }
        if let Some(ver) = &self.version {
            info = info.with_version(ver);
        }
        for protocol in &self.protocols {
            info = info.with_protocol(*protocol);
        }
        for cap in &self.capabilities {
            info = info.with_capability(cap.clone());
        }
        if let Some(url) = &self.endpoint {
            info = info.with_url(url);
        }
        info
    }
}

/// Health status of an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Health status is unknown
    Unknown,
    /// Agent is healthy and responsive
    Healthy,
    /// Agent is degraded but still functional
    Degraded,
    /// Agent is unhealthy/unresponsive
    Unhealthy,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Unknown => write!(f, "unknown"),
            HealthStatus::Healthy => write!(f, "healthy"),
            HealthStatus::Degraded => write!(f, "degraded"),
            HealthStatus::Unhealthy => write!(f, "unhealthy"),
        }
    }
}

// ============================================================================
// Query Types
// ============================================================================

/// Query builder for discovering agents.
#[derive(Debug, Clone, Default)]
pub struct DiscoveryQuery {
    /// Filter by agent ID
    pub agent_id: Option<String>,
    /// Filter by protocols
    pub protocols: Vec<Protocol>,
    /// Filter by capabilities (agent must have ALL specified)
    pub capabilities: Vec<String>,
    /// Filter by capabilities (agent must have ANY specified)
    pub capabilities_any: Vec<String>,
    /// Filter by tags (agent must have ALL specified)
    pub tags: Vec<String>,
    /// Filter by tags (agent must have ANY specified)
    pub tags_any: Vec<String>,
    /// Filter by health status
    pub health_status: Option<HealthStatus>,
    /// Include stale registrations
    pub include_stale: bool,
    /// Maximum number of results
    pub limit: Option<usize>,
    /// Custom filter predicate name (for provider-specific filters)
    pub custom_filter: Option<String>,
}

impl DiscoveryQuery {
    /// Create a new empty query (matches all agents).
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by agent ID.
    pub fn with_agent_id(mut self, id: impl Into<String>) -> Self {
        self.agent_id = Some(id.into());
        self
    }

    /// Filter by protocol.
    pub fn with_protocol(mut self, protocol: Protocol) -> Self {
        if !self.protocols.contains(&protocol) {
            self.protocols.push(protocol);
        }
        self
    }

    /// Filter by capability (must have this capability).
    pub fn with_capability(mut self, capability: impl Into<String>) -> Self {
        self.capabilities.push(capability.into());
        self
    }

    /// Filter by any of these capabilities.
    pub fn with_capability_any(mut self, capability: impl Into<String>) -> Self {
        self.capabilities_any.push(capability.into());
        self
    }

    /// Filter by tag (must have this tag).
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Filter by any of these tags.
    pub fn with_tag_any(mut self, tag: impl Into<String>) -> Self {
        self.tags_any.push(tag.into());
        self
    }

    /// Filter by health status.
    pub fn with_health_status(mut self, status: HealthStatus) -> Self {
        self.health_status = Some(status);
        self
    }

    /// Include stale registrations in results.
    pub fn include_stale(mut self) -> Self {
        self.include_stale = true;
        self
    }

    /// Limit the number of results.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Check if a registration matches this query.
    pub fn matches(&self, registration: &AgentRegistration) -> bool {
        // Check stale
        if !self.include_stale && registration.is_stale() {
            return false;
        }

        // Check agent ID
        if self
            .agent_id
            .as_ref()
            .is_some_and(|id| registration.agent_id != *id)
        {
            return false;
        }

        // Check protocols (must support at least one)
        if !self.protocols.is_empty()
            && !self
                .protocols
                .iter()
                .any(|p| registration.protocols.contains(p))
        {
            return false;
        }

        // Check capabilities (must have ALL)
        if !self.capabilities.is_empty() {
            let reg_caps: HashSet<_> = registration.capabilities.iter().map(|c| &c.id).collect();
            if !self.capabilities.iter().all(|c| reg_caps.contains(c)) {
                return false;
            }
        }

        // Check capabilities_any (must have ANY)
        if !self.capabilities_any.is_empty() {
            let reg_caps: HashSet<_> = registration.capabilities.iter().map(|c| &c.id).collect();
            if !self.capabilities_any.iter().any(|c| reg_caps.contains(c)) {
                return false;
            }
        }

        // Check tags (must have ALL)
        if !self.tags.is_empty() {
            let reg_tags: HashSet<_> = registration.tags.iter().collect();
            if !self.tags.iter().all(|t| reg_tags.contains(t)) {
                return false;
            }
        }

        // Check tags_any (must have ANY)
        if !self.tags_any.is_empty() {
            let reg_tags: HashSet<_> = registration.tags.iter().collect();
            if !self.tags_any.iter().any(|t| reg_tags.contains(t)) {
                return false;
            }
        }

        // Check health status
        if self
            .health_status
            .is_some_and(|status| registration.health_status != status)
        {
            return false;
        }

        true
    }
}

// ============================================================================
// Events
// ============================================================================

/// Events emitted by the discovery service.
#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    /// An agent was registered.
    AgentRegistered {
        registration_id: String,
        agent_id: String,
        name: String,
    },
    /// An agent was deregistered.
    AgentDeregistered {
        registration_id: String,
        agent_id: String,
        reason: DeregistrationReason,
    },
    /// Agent health status changed.
    HealthStatusChanged {
        registration_id: String,
        agent_id: String,
        old_status: HealthStatus,
        new_status: HealthStatus,
    },
    /// Agent heartbeat received.
    Heartbeat {
        registration_id: String,
        agent_id: String,
    },
}

/// Reason for agent deregistration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeregistrationReason {
    /// Agent explicitly deregistered
    Explicit,
    /// Registration expired (no heartbeat)
    Expired,
    /// Health check failed repeatedly
    Unhealthy,
    /// Administrative removal
    Administrative,
}

// ============================================================================
// Discovery Provider Trait
// ============================================================================

/// Trait for discovery service providers.
///
/// Implement this trait to create custom discovery backends (e.g., etcd, consul, redis).
#[async_trait]
pub trait DiscoveryProvider: Send + Sync {
    /// Register an agent.
    async fn register(&self, registration: AgentRegistration) -> AgentResult<String>;

    /// Deregister an agent by registration ID.
    async fn deregister(&self, registration_id: &str) -> AgentResult<()>;

    /// Update heartbeat for a registration.
    async fn heartbeat(&self, registration_id: &str) -> AgentResult<()>;

    /// Update health status for a registration.
    async fn update_health(&self, registration_id: &str, status: HealthStatus) -> AgentResult<()>;

    /// Get a registration by ID.
    async fn get(&self, registration_id: &str) -> AgentResult<Option<AgentRegistration>>;

    /// Query registrations.
    async fn query(&self, query: &DiscoveryQuery) -> AgentResult<Vec<AgentRegistration>>;

    /// List all registrations.
    async fn list(&self) -> AgentResult<Vec<AgentRegistration>>;

    /// Get count of registrations.
    async fn count(&self) -> AgentResult<usize>;

    /// Clean up stale registrations.
    async fn cleanup_stale(&self) -> AgentResult<Vec<String>>;
}

// ============================================================================
// In-Memory Provider
// ============================================================================

/// In-memory discovery provider for single-process deployments.
pub struct InMemoryDiscoveryProvider {
    registrations: RwLock<HashMap<String, AgentRegistration>>,
    event_tx: broadcast::Sender<DiscoveryEvent>,
}

impl Default for InMemoryDiscoveryProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryDiscoveryProvider {
    /// Create a new in-memory provider.
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(100);
        Self {
            registrations: RwLock::new(HashMap::new()),
            event_tx,
        }
    }

    /// Subscribe to discovery events.
    pub fn subscribe(&self) -> broadcast::Receiver<DiscoveryEvent> {
        self.event_tx.subscribe()
    }

    fn emit_event(&self, event: DiscoveryEvent) {
        // Ignore send errors (no subscribers)
        let _ = self.event_tx.send(event);
    }
}

#[async_trait]
impl DiscoveryProvider for InMemoryDiscoveryProvider {
    async fn register(&self, registration: AgentRegistration) -> AgentResult<String> {
        let id = registration.id.clone();
        let agent_id = registration.agent_id.clone();
        let name = registration.name.clone();

        info!(
            registration_id = %id,
            agent_id = %agent_id,
            name = %name,
            "Registering agent in discovery service"
        );

        self.registrations
            .write()
            .await
            .insert(id.clone(), registration);

        self.emit_event(DiscoveryEvent::AgentRegistered {
            registration_id: id.clone(),
            agent_id,
            name,
        });

        Ok(id)
    }

    async fn deregister(&self, registration_id: &str) -> AgentResult<()> {
        let mut regs = self.registrations.write().await;
        if let Some(reg) = regs.remove(registration_id) {
            info!(
                registration_id = %registration_id,
                agent_id = %reg.agent_id,
                "Deregistering agent from discovery service"
            );

            self.emit_event(DiscoveryEvent::AgentDeregistered {
                registration_id: registration_id.to_string(),
                agent_id: reg.agent_id,
                reason: DeregistrationReason::Explicit,
            });

            Ok(())
        } else {
            Err(AgentError::AgentNotFound(registration_id.to_string()))
        }
    }

    async fn heartbeat(&self, registration_id: &str) -> AgentResult<()> {
        let mut regs = self.registrations.write().await;
        if let Some(reg) = regs.get_mut(registration_id) {
            reg.heartbeat();
            debug!(
                registration_id = %registration_id,
                agent_id = %reg.agent_id,
                "Heartbeat received"
            );

            self.emit_event(DiscoveryEvent::Heartbeat {
                registration_id: registration_id.to_string(),
                agent_id: reg.agent_id.clone(),
            });

            Ok(())
        } else {
            Err(AgentError::AgentNotFound(registration_id.to_string()))
        }
    }

    async fn update_health(&self, registration_id: &str, status: HealthStatus) -> AgentResult<()> {
        let mut regs = self.registrations.write().await;
        if let Some(reg) = regs.get_mut(registration_id) {
            let old_status = reg.health_status;
            if old_status != status {
                reg.health_status = status;
                info!(
                    registration_id = %registration_id,
                    agent_id = %reg.agent_id,
                    old_status = %old_status,
                    new_status = %status,
                    "Health status changed"
                );

                self.emit_event(DiscoveryEvent::HealthStatusChanged {
                    registration_id: registration_id.to_string(),
                    agent_id: reg.agent_id.clone(),
                    old_status,
                    new_status: status,
                });
            }
            Ok(())
        } else {
            Err(AgentError::AgentNotFound(registration_id.to_string()))
        }
    }

    async fn get(&self, registration_id: &str) -> AgentResult<Option<AgentRegistration>> {
        Ok(self
            .registrations
            .read()
            .await
            .get(registration_id)
            .cloned())
    }

    async fn query(&self, query: &DiscoveryQuery) -> AgentResult<Vec<AgentRegistration>> {
        let regs = self.registrations.read().await;
        let mut results: Vec<_> = regs
            .values()
            .filter(|r| query.matches(r))
            .cloned()
            .collect();

        // Apply limit if specified
        if let Some(limit) = query.limit {
            results.truncate(limit);
        }

        Ok(results)
    }

    async fn list(&self) -> AgentResult<Vec<AgentRegistration>> {
        Ok(self.registrations.read().await.values().cloned().collect())
    }

    async fn count(&self) -> AgentResult<usize> {
        Ok(self.registrations.read().await.len())
    }

    async fn cleanup_stale(&self) -> AgentResult<Vec<String>> {
        let mut regs = self.registrations.write().await;
        let stale_ids: Vec<_> = regs
            .iter()
            .filter(|(_, r)| r.is_stale())
            .map(|(id, r)| (id.clone(), r.agent_id.clone()))
            .collect();

        let mut removed = Vec::new();
        for (id, agent_id) in stale_ids {
            regs.remove(&id);
            warn!(
                registration_id = %id,
                agent_id = %agent_id,
                "Removed stale agent registration"
            );

            self.emit_event(DiscoveryEvent::AgentDeregistered {
                registration_id: id.clone(),
                agent_id,
                reason: DeregistrationReason::Expired,
            });

            removed.push(id);
        }

        Ok(removed)
    }
}

// ============================================================================
// Discovery Service
// ============================================================================

/// Configuration for the discovery service.
#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    /// Enable automatic health checking
    pub enable_health_check: bool,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Enable automatic stale registration cleanup
    pub enable_cleanup: bool,
    /// Cleanup interval
    pub cleanup_interval: Duration,
    /// HTTP client timeout for health checks
    pub health_check_timeout: Duration,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            enable_health_check: true,
            health_check_interval: Duration::from_secs(30),
            enable_cleanup: true,
            cleanup_interval: Duration::from_secs(60),
            health_check_timeout: Duration::from_secs(5),
        }
    }
}

/// The main discovery service.
pub struct DiscoveryService {
    provider: Arc<dyn DiscoveryProvider>,
    config: DiscoveryConfig,
    /// Registered agents (Arc<dyn UnifiedAgent> for direct access)
    agents: RwLock<HashMap<String, Arc<dyn UnifiedAgent>>>,
}

impl DiscoveryService {
    /// Create a new discovery service with in-memory provider.
    pub fn new() -> Self {
        Self::with_provider(Arc::new(InMemoryDiscoveryProvider::new()))
    }

    /// Create a new discovery service with custom provider.
    pub fn with_provider(provider: Arc<dyn DiscoveryProvider>) -> Self {
        Self {
            provider,
            config: DiscoveryConfig::default(),
            agents: RwLock::new(HashMap::new()),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(mut self, config: DiscoveryConfig) -> Self {
        self.config = config;
        self
    }

    /// Get the configuration.
    pub fn config(&self) -> &DiscoveryConfig {
        &self.config
    }

    /// Register an agent.
    pub async fn register(&self, registration: AgentRegistration) -> AgentResult<String> {
        self.provider.register(registration).await
    }

    /// Register a UnifiedAgent directly.
    pub async fn register_agent(&self, agent: Arc<dyn UnifiedAgent>) -> AgentResult<String> {
        let registration = AgentRegistration::from_agent(agent.as_ref());
        let id = self.provider.register(registration).await?;
        self.agents.write().await.insert(id.clone(), agent);
        Ok(id)
    }

    /// Deregister an agent.
    pub async fn deregister(&self, registration_id: &str) -> AgentResult<()> {
        self.agents.write().await.remove(registration_id);
        self.provider.deregister(registration_id).await
    }

    /// Send heartbeat for a registration.
    pub async fn heartbeat(&self, registration_id: &str) -> AgentResult<()> {
        self.provider.heartbeat(registration_id).await
    }

    /// Get a registration by ID.
    pub async fn get(&self, registration_id: &str) -> AgentResult<Option<AgentRegistration>> {
        self.provider.get(registration_id).await
    }

    /// Query registrations.
    pub async fn query(&self, query: DiscoveryQuery) -> AgentResult<Vec<AgentRegistration>> {
        self.provider.query(&query).await
    }

    /// Find agents by capability.
    pub async fn find_by_capability(
        &self,
        capability: &str,
    ) -> AgentResult<Vec<AgentRegistration>> {
        self.query(DiscoveryQuery::new().with_capability(capability))
            .await
    }

    /// Find agents by protocol.
    pub async fn find_by_protocol(
        &self,
        protocol: Protocol,
    ) -> AgentResult<Vec<AgentRegistration>> {
        self.query(DiscoveryQuery::new().with_protocol(protocol))
            .await
    }

    /// Find agents by tag.
    pub async fn find_by_tag(&self, tag: &str) -> AgentResult<Vec<AgentRegistration>> {
        self.query(DiscoveryQuery::new().with_tag(tag)).await
    }

    /// List all registrations.
    pub async fn list(&self) -> AgentResult<Vec<AgentRegistration>> {
        self.provider.list().await
    }

    /// Get count of registrations.
    pub async fn count(&self) -> AgentResult<usize> {
        self.provider.count().await
    }

    /// Get a registered UnifiedAgent by registration ID.
    pub async fn get_agent(&self, registration_id: &str) -> Option<Arc<dyn UnifiedAgent>> {
        self.agents.read().await.get(registration_id).cloned()
    }

    /// Clean up stale registrations.
    pub async fn cleanup_stale(&self) -> AgentResult<Vec<String>> {
        let removed = self.provider.cleanup_stale().await?;
        let mut agents = self.agents.write().await;
        for id in &removed {
            agents.remove(id);
        }
        Ok(removed)
    }

    /// Start background tasks (health checking, cleanup).
    pub fn start_background_tasks(self: &Arc<Self>) -> BackgroundTaskHandle {
        let service = Arc::clone(self);
        let config = self.config.clone();

        let handle = tokio::spawn(async move {
            let mut health_interval = tokio::time::interval(config.health_check_interval);
            let mut cleanup_interval = tokio::time::interval(config.cleanup_interval);

            loop {
                tokio::select! {
                    _ = health_interval.tick() => {
                        if config.enable_health_check
                            && let Err(e) = service.run_health_checks().await
                        {
                            warn!(error = %e, "Health check failed");
                        }
                    }
                    _ = cleanup_interval.tick() => {
                        if config.enable_cleanup
                            && let Err(e) = service.cleanup_stale().await
                        {
                            warn!(error = %e, "Cleanup failed");
                        }
                    }
                }
            }
        });

        BackgroundTaskHandle { handle }
    }

    /// Run health checks on all registered agents.
    async fn run_health_checks(&self) -> AgentResult<()> {
        let registrations = self.provider.list().await?;

        for reg in registrations {
            let health_url = reg
                .health_endpoint
                .clone()
                .or_else(|| reg.endpoint.as_ref().map(|e| format!("{}/health", e)));

            if let Some(url) = health_url {
                let status = self.check_health(&url).await;
                let _ = self.provider.update_health(&reg.id, status).await;
            }
        }

        Ok(())
    }

    /// Check health of a single endpoint.
    ///
    /// When the `discovery-health` feature is enabled, this performs an actual
    /// HTTP GET request to the health endpoint. A 2xx response is considered
    /// `Healthy`, a 5xx response is `Unhealthy`, and anything else is `Degraded`.
    ///
    /// Without the feature, this always returns `Unknown`.
    #[cfg(feature = "discovery-health")]
    async fn check_health(&self, url: &str) -> HealthStatus {
        let client = reqwest::Client::new();
        match client
            .get(url)
            .timeout(self.config.health_check_timeout)
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status().as_u16();
                if (200..300).contains(&status) {
                    HealthStatus::Healthy
                } else if status >= 500 {
                    HealthStatus::Unhealthy
                } else {
                    HealthStatus::Degraded
                }
            }
            Err(e) => {
                debug!(url = %url, error = %e, "Health check request failed");
                HealthStatus::Unhealthy
            }
        }
    }

    /// Check health of a single endpoint (stub without `discovery-health` feature).
    #[cfg(not(feature = "discovery-health"))]
    async fn check_health(&self, _url: &str) -> HealthStatus {
        debug!(
            "Health check skipped: enable the 'discovery-health' feature for HTTP health checks"
        );
        HealthStatus::Unknown
    }
}

impl Default for DiscoveryService {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle for background tasks.
pub struct BackgroundTaskHandle {
    handle: tokio::task::JoinHandle<()>,
}

impl BackgroundTaskHandle {
    /// Stop the background tasks.
    pub fn stop(self) {
        self.handle.abort();
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registration_builder() {
        let reg = AgentRegistration::new("agent-1", "Test Agent")
            .with_description("A test agent")
            .with_version("1.0.0")
            .with_protocol(Protocol::A2a)
            .with_capability("search")
            .with_endpoint("https://agent.example.com")
            .with_tag("production")
            .with_ttl(600);

        assert_eq!(reg.agent_id, "agent-1");
        assert_eq!(reg.name, "Test Agent");
        assert_eq!(reg.description, Some("A test agent".to_string()));
        assert!(reg.protocols.contains(&Protocol::A2a));
        assert_eq!(reg.capabilities.len(), 1);
        assert_eq!(reg.tags, vec!["production"]);
        assert_eq!(reg.ttl_seconds, Some(600));
    }

    #[test]
    fn test_registration_is_stale() {
        let mut reg = AgentRegistration::new("agent-1", "Test").with_ttl(1); // 1 second TTL

        // Manually set last_heartbeat to past
        reg.last_heartbeat = Utc::now() - chrono::Duration::seconds(10);

        assert!(reg.is_stale());

        // Update heartbeat
        reg.heartbeat();
        assert!(!reg.is_stale());
    }

    #[test]
    fn test_query_builder() {
        let query = DiscoveryQuery::new()
            .with_protocol(Protocol::A2a)
            .with_capability("search")
            .with_tag("production")
            .with_health_status(HealthStatus::Healthy)
            .with_limit(10);

        assert!(query.protocols.contains(&Protocol::A2a));
        assert!(query.capabilities.contains(&"search".to_string()));
        assert!(query.tags.contains(&"production".to_string()));
        assert_eq!(query.health_status, Some(HealthStatus::Healthy));
        assert_eq!(query.limit, Some(10));
    }

    #[test]
    fn test_query_matches() {
        let reg = AgentRegistration::new("agent-1", "Test Agent")
            .with_protocol(Protocol::A2a)
            .with_capability("search")
            .with_capability("analyze")
            .with_tag("production");

        // Should match
        let query1 = DiscoveryQuery::new()
            .with_protocol(Protocol::A2a)
            .with_capability("search");
        assert!(query1.matches(&reg));

        // Should match with capability_any
        let query2 = DiscoveryQuery::new()
            .with_capability_any("search")
            .with_capability_any("nonexistent");
        assert!(query2.matches(&reg));

        // Should not match - missing capability
        let query3 = DiscoveryQuery::new()
            .with_capability("search")
            .with_capability("nonexistent");
        assert!(!query3.matches(&reg));

        // Should not match - wrong protocol
        let query4 = DiscoveryQuery::new().with_protocol(Protocol::Mcp);
        assert!(!query4.matches(&reg));
    }

    #[tokio::test]
    async fn test_in_memory_provider() {
        let provider = InMemoryDiscoveryProvider::new();

        // Register
        let reg = AgentRegistration::new("agent-1", "Test Agent")
            .with_protocol(Protocol::A2a)
            .with_capability("search");

        let id = provider.register(reg).await.unwrap();

        // Get
        let retrieved = provider.get(&id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().agent_id, "agent-1");

        // Query
        let results = provider
            .query(&DiscoveryQuery::new().with_capability("search"))
            .await
            .unwrap();
        assert_eq!(results.len(), 1);

        // Heartbeat
        provider.heartbeat(&id).await.unwrap();

        // Update health
        provider
            .update_health(&id, HealthStatus::Healthy)
            .await
            .unwrap();
        let updated = provider.get(&id).await.unwrap().unwrap();
        assert_eq!(updated.health_status, HealthStatus::Healthy);

        // Deregister
        provider.deregister(&id).await.unwrap();
        assert_eq!(provider.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_discovery_service() {
        let service = DiscoveryService::new();

        // Register
        let reg = AgentRegistration::new("agent-1", "Test Agent")
            .with_protocol(Protocol::A2a)
            .with_capability("search")
            .with_tag("test");

        let id = service.register(reg).await.unwrap();

        // Query by capability
        let results = service.find_by_capability("search").await.unwrap();
        assert_eq!(results.len(), 1);

        // Query by protocol
        let results = service.find_by_protocol(Protocol::A2a).await.unwrap();
        assert_eq!(results.len(), 1);

        // Query by tag
        let results = service.find_by_tag("test").await.unwrap();
        assert_eq!(results.len(), 1);

        // List all
        let all = service.list().await.unwrap();
        assert_eq!(all.len(), 1);

        // Count
        assert_eq!(service.count().await.unwrap(), 1);

        // Deregister
        service.deregister(&id).await.unwrap();
        assert_eq!(service.count().await.unwrap(), 0);
    }

    #[test]
    fn test_health_status_display() {
        assert_eq!(HealthStatus::Healthy.to_string(), "healthy");
        assert_eq!(HealthStatus::Unhealthy.to_string(), "unhealthy");
        assert_eq!(HealthStatus::Degraded.to_string(), "degraded");
        assert_eq!(HealthStatus::Unknown.to_string(), "unknown");
    }

    #[tokio::test]
    async fn test_cleanup_stale() {
        let provider = InMemoryDiscoveryProvider::new();

        // Register with very short TTL
        let mut reg = AgentRegistration::new("agent-1", "Test Agent").with_ttl(1);
        reg.last_heartbeat = Utc::now() - chrono::Duration::seconds(10);
        let id = provider.register(reg).await.unwrap();

        // Should have 1 registration
        assert_eq!(provider.count().await.unwrap(), 1);

        // Cleanup should remove the stale registration
        let removed = provider.cleanup_stale().await.unwrap();
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0], id);

        // Should have 0 registrations now
        assert_eq!(provider.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_event_subscription() {
        let provider = InMemoryDiscoveryProvider::new();
        let mut rx = provider.subscribe();

        // Register
        let reg = AgentRegistration::new("agent-1", "Test Agent");
        let id = provider.register(reg).await.unwrap();

        // Should receive registration event
        let event = rx.try_recv().unwrap();
        match event {
            DiscoveryEvent::AgentRegistered {
                registration_id,
                agent_id,
                ..
            } => {
                assert_eq!(registration_id, id);
                assert_eq!(agent_id, "agent-1");
            }
            _ => panic!("Expected AgentRegistered event"),
        }

        // Deregister
        provider.deregister(&id).await.unwrap();

        // Should receive deregistration event
        let event = rx.try_recv().unwrap();
        match event {
            DiscoveryEvent::AgentDeregistered {
                registration_id,
                reason,
                ..
            } => {
                assert_eq!(registration_id, id);
                assert_eq!(reason, DeregistrationReason::Explicit);
            }
            _ => panic!("Expected AgentDeregistered event"),
        }
    }
}
