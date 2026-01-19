//! Agent orchestration patterns for running multiple agents together.
//!
//! This module provides various orchestration patterns:
//! - **SequentialPipeline**: Chain agents where output of one becomes input of next
//! - **ParallelAgent**: Run multiple agents concurrently and aggregate results
//! - **RouterAgent**: Route messages to agents based on rules or capabilities
//! - **SupervisorAgent**: Coordinate complex workflows with decision-making

use async_trait::async_trait;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::error::{AgentError, AgentResult};
use crate::traits::UnifiedAgent;
use crate::types::{AgentInfo, MessageRole, StreamEvent, TaskStatus, UnifiedMessage, UnifiedTask};

// ============================================================================
// SequentialPipeline - Chain agents in sequence
// ============================================================================

/// A pipeline that chains agents sequentially.
///
/// Each agent's output becomes the input for the next agent in the chain.
/// This is useful for multi-step processing workflows.
///
/// # Example
/// ```rust,ignore
/// let pipeline = SequentialPipeline::new("analysis-pipeline", "Analysis Pipeline")
///     .add_stage(preprocessor_agent)
///     .add_stage(analyzer_agent)
///     .add_stage(summarizer_agent);
///
/// let result = pipeline.send_message(UnifiedMessage::user("Analyze this data")).await?;
/// ```
pub struct SequentialPipeline {
    info: AgentInfo,
    stages: Vec<Arc<dyn UnifiedAgent>>,
    /// How to transform output from one stage to input for the next
    transform: TransformMode,
    tasks: tokio::sync::RwLock<HashMap<String, PipelineTask>>,
}

/// How to transform output between pipeline stages.
#[derive(Debug, Clone, Copy, Default)]
pub enum TransformMode {
    /// Use the last agent message as input to the next stage
    #[default]
    LastMessage,
    /// Concatenate all agent messages
    ConcatenateMessages,
    /// Use artifacts as input (first text artifact)
    FirstArtifact,
}

/// Internal state for pipeline task tracking.
#[derive(Debug, Clone)]
struct PipelineTask {
    task: UnifiedTask,
    /// Current stage index (for resumable pipelines - future use)
    #[allow(dead_code)]
    current_stage: usize,
    /// Task IDs from each stage (for debugging/tracing - future use)
    #[allow(dead_code)]
    stage_tasks: Vec<String>,
}

impl SequentialPipeline {
    /// Create a new sequential pipeline.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            info: AgentInfo::new(id, name).with_description("Sequential agent pipeline"),
            stages: Vec::new(),
            transform: TransformMode::default(),
            tasks: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Add a stage to the pipeline.
    pub fn add_stage(mut self, agent: Arc<dyn UnifiedAgent>) -> Self {
        // Merge capabilities from the agent
        for cap in agent.capabilities() {
            if !self.info.capabilities.iter().any(|c| c.id == cap.id) {
                self.info.capabilities.push(cap.clone());
            }
        }
        // Merge protocols
        for proto in &agent.info().protocols {
            if !self.info.protocols.contains(proto) {
                self.info.protocols.push(*proto);
            }
        }
        self.stages.push(agent);
        self
    }

    /// Set the transform mode between stages.
    pub fn with_transform(mut self, mode: TransformMode) -> Self {
        self.transform = mode;
        self
    }

    /// Get the number of stages.
    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    /// Extract the next input from a completed task.
    fn extract_next_input(&self, task: &UnifiedTask) -> String {
        match self.transform {
            TransformMode::LastMessage => task
                .messages
                .iter()
                .filter(|m| m.role == MessageRole::Agent)
                .next_back()
                .map(|m| m.text_content())
                .unwrap_or_default(),
            TransformMode::ConcatenateMessages => task
                .messages
                .iter()
                .filter(|m| m.role == MessageRole::Agent)
                .map(|m| m.text_content())
                .collect::<Vec<_>>()
                .join("\n\n"),
            TransformMode::FirstArtifact => task
                .artifacts
                .first()
                .and_then(|a| a.content.first())
                .and_then(|c| c.as_text())
                .map(|s| s.to_string())
                .unwrap_or_default(),
        }
    }
}

#[async_trait]
impl UnifiedAgent for SequentialPipeline {
    fn info(&self) -> &AgentInfo {
        &self.info
    }

    async fn send_message(&self, message: UnifiedMessage) -> AgentResult<UnifiedTask> {
        if self.stages.is_empty() {
            return Err(AgentError::Internal("Pipeline has no stages".to_string()));
        }

        let mut pipeline_task = UnifiedTask::new_with_uuid();
        pipeline_task.add_message(message.clone());

        let mut current_input = message;
        let mut stage_task_ids = Vec::new();

        for (idx, stage) in self.stages.iter().enumerate() {
            debug!(
                pipeline = %self.info.id,
                stage = idx,
                agent = %stage.info().id,
                "Executing pipeline stage"
            );

            let stage_result = stage.send_message(current_input.clone()).await?;
            stage_task_ids.push(stage_result.id.clone());

            // Check if stage failed
            if stage_result.status == TaskStatus::Failed {
                pipeline_task.set_status(TaskStatus::Failed);
                // Add error message
                pipeline_task.add_message(UnifiedMessage::agent(format!(
                    "Pipeline failed at stage {}: {}",
                    idx,
                    stage.info().name
                )));
                return Ok(pipeline_task);
            }

            // Add stage messages to pipeline task
            for msg in &stage_result.messages {
                if msg.role == MessageRole::Agent {
                    pipeline_task.add_message(msg.clone());
                }
            }

            // Add stage artifacts to pipeline task
            for artifact in &stage_result.artifacts {
                pipeline_task.add_artifact(artifact.clone());
            }

            // Prepare input for next stage
            if idx < self.stages.len() - 1 {
                let next_text = self.extract_next_input(&stage_result);
                current_input = UnifiedMessage::user(next_text);
            }
        }

        // Store pipeline task state
        let task_id = pipeline_task.id.clone();
        self.tasks.write().await.insert(
            task_id.clone(),
            PipelineTask {
                task: pipeline_task.clone(),
                current_stage: self.stages.len(),
                stage_tasks: stage_task_ids,
            },
        );

        pipeline_task.set_status(TaskStatus::Completed);
        info!(
            pipeline = %self.info.id,
            task_id = %task_id,
            stages = self.stages.len(),
            "Pipeline completed"
        );

        Ok(pipeline_task)
    }

    async fn send_message_to_task(
        &self,
        task_id: &str,
        message: UnifiedMessage,
    ) -> AgentResult<UnifiedTask> {
        // For pipelines, continuing a task starts a new pipeline run
        let _ = task_id;
        self.send_message(message).await
    }

    async fn send_message_streaming(
        &self,
        message: UnifiedMessage,
    ) -> AgentResult<Pin<Box<dyn Stream<Item = AgentResult<StreamEvent>> + Send>>> {
        // Execute pipeline and simulate streaming
        let task = self.send_message(message).await?;
        let task_id = task.id.clone();

        let stream = async_stream::stream! {
            yield Ok(StreamEvent::StatusUpdate {
                task_id: task_id.clone(),
                status: TaskStatus::Working,
                message: Some("Pipeline started".to_string()),
            });

            for msg in &task.messages {
                yield Ok(StreamEvent::MessageAdded {
                    task_id: task_id.clone(),
                    message: msg.clone(),
                });
            }

            for artifact in &task.artifacts {
                yield Ok(StreamEvent::ArtifactAdded {
                    task_id: task_id.clone(),
                    artifact: artifact.clone(),
                });
            }

            yield Ok(StreamEvent::StatusUpdate {
                task_id: task_id.clone(),
                status: task.status,
                message: None,
            });
        };

        Ok(Box::pin(stream))
    }

    async fn get_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
        self.tasks
            .read()
            .await
            .get(task_id)
            .map(|pt| pt.task.clone())
            .ok_or_else(|| AgentError::TaskNotFound(task_id.to_string()))
    }

    async fn cancel_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
        let mut tasks = self.tasks.write().await;
        let pipeline_task = tasks
            .get_mut(task_id)
            .ok_or_else(|| AgentError::TaskNotFound(task_id.to_string()))?;

        pipeline_task.task.set_status(TaskStatus::Cancelled);
        Ok(pipeline_task.task.clone())
    }
}

// ============================================================================
// ParallelAgent - Run agents concurrently and aggregate results
// ============================================================================

/// How to aggregate results from parallel execution.
#[derive(Debug, Clone, Copy, Default)]
pub enum AggregationMode {
    /// Collect all results (messages and artifacts from all agents)
    #[default]
    CollectAll,
    /// Return first successful result
    FirstSuccess,
    /// Return first completed (success or failure)
    FirstComplete,
    /// Require all to succeed
    RequireAll,
}

/// An agent that runs multiple agents in parallel and aggregates results.
///
/// This is useful for:
/// - Querying multiple data sources simultaneously
/// - Getting diverse perspectives from different agents
/// - Redundancy and failover patterns
///
/// # Example
/// ```rust,ignore
/// let parallel = ParallelAgent::new("multi-search", "Multi Search")
///     .add_agent(google_agent)
///     .add_agent(bing_agent)
///     .add_agent(duckduckgo_agent)
///     .with_aggregation(AggregationMode::CollectAll);
///
/// let result = parallel.send_message(UnifiedMessage::user("Search for rust")).await?;
/// ```
pub struct ParallelAgent {
    info: AgentInfo,
    agents: Vec<Arc<dyn UnifiedAgent>>,
    aggregation: AggregationMode,
    /// Maximum time to wait for all agents (in milliseconds)
    timeout_ms: Option<u64>,
    tasks: tokio::sync::RwLock<HashMap<String, UnifiedTask>>,
}

impl ParallelAgent {
    /// Create a new parallel agent.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            info: AgentInfo::new(id, name).with_description("Parallel agent execution"),
            agents: Vec::new(),
            aggregation: AggregationMode::default(),
            timeout_ms: None,
            tasks: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Add an agent to run in parallel.
    pub fn add_agent(mut self, agent: Arc<dyn UnifiedAgent>) -> Self {
        // Merge capabilities
        for cap in agent.capabilities() {
            if !self.info.capabilities.iter().any(|c| c.id == cap.id) {
                self.info.capabilities.push(cap.clone());
            }
        }
        self.agents.push(agent);
        self
    }

    /// Set the aggregation mode.
    pub fn with_aggregation(mut self, mode: AggregationMode) -> Self {
        self.aggregation = mode;
        self
    }

    /// Set timeout for parallel execution.
    pub fn with_timeout_ms(mut self, timeout: u64) -> Self {
        self.timeout_ms = Some(timeout);
        self
    }

    /// Get the number of parallel agents.
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }
}

#[async_trait]
impl UnifiedAgent for ParallelAgent {
    fn info(&self) -> &AgentInfo {
        &self.info
    }

    async fn send_message(&self, message: UnifiedMessage) -> AgentResult<UnifiedTask> {
        if self.agents.is_empty() {
            return Err(AgentError::Internal(
                "ParallelAgent has no agents".to_string(),
            ));
        }

        let mut combined = UnifiedTask::new_with_uuid();
        combined.add_message(message.clone());

        // Execute all agents in parallel
        let futures: Vec<_> = self
            .agents
            .iter()
            .map(|a| a.send_message(message.clone()))
            .collect();

        let results = if let Some(timeout) = self.timeout_ms {
            match tokio::time::timeout(
                std::time::Duration::from_millis(timeout),
                futures::future::join_all(futures),
            )
            .await
            {
                Ok(results) => results,
                Err(_) => {
                    warn!(
                        agent = %self.info.id,
                        timeout_ms = timeout,
                        "Parallel execution timed out"
                    );
                    combined.add_message(UnifiedMessage::agent("Some agents timed out"));
                    combined.set_status(TaskStatus::Completed);
                    return Ok(combined);
                }
            }
        } else {
            futures::future::join_all(futures).await
        };

        // Process results based on aggregation mode
        match self.aggregation {
            AggregationMode::CollectAll => {
                let mut any_success = false;
                for (idx, result) in results.into_iter().enumerate() {
                    match result {
                        Ok(task) => {
                            any_success = true;
                            // Add agent identifier to messages
                            for msg in task.messages {
                                if msg.role == MessageRole::Agent {
                                    let mut annotated = msg.clone();
                                    annotated.metadata.insert(
                                        "source_agent".to_string(),
                                        serde_json::json!(self.agents[idx].info().id),
                                    );
                                    combined.add_message(annotated);
                                }
                            }
                            for artifact in task.artifacts {
                                combined.add_artifact(artifact);
                            }
                        }
                        Err(e) => {
                            combined.add_message(UnifiedMessage::agent(format!(
                                "Agent {} failed: {}",
                                self.agents[idx].info().id,
                                e
                            )));
                        }
                    }
                }
                combined.set_status(if any_success {
                    TaskStatus::Completed
                } else {
                    TaskStatus::Failed
                });
            }

            AggregationMode::FirstSuccess => {
                for result in results {
                    if let Ok(task) = result
                        && task.status == TaskStatus::Completed
                    {
                        for msg in task.messages {
                            combined.add_message(msg);
                        }
                        for artifact in task.artifacts {
                            combined.add_artifact(artifact);
                        }
                        combined.set_status(TaskStatus::Completed);
                        break;
                    }
                }
                if combined.status != TaskStatus::Completed {
                    combined.set_status(TaskStatus::Failed);
                    combined.add_message(UnifiedMessage::agent("No agent succeeded"));
                }
            }

            AggregationMode::FirstComplete => {
                // In true first-complete, we'd use select! - here we just take first result
                if let Some(result) = results.into_iter().next() {
                    match result {
                        Ok(task) => {
                            for msg in task.messages {
                                combined.add_message(msg);
                            }
                            for artifact in task.artifacts {
                                combined.add_artifact(artifact);
                            }
                            combined.set_status(task.status);
                        }
                        Err(e) => {
                            combined.add_message(UnifiedMessage::agent(format!("Error: {}", e)));
                            combined.set_status(TaskStatus::Failed);
                        }
                    }
                }
            }

            AggregationMode::RequireAll => {
                let mut all_success = true;
                for (idx, result) in results.into_iter().enumerate() {
                    match result {
                        Ok(task) => {
                            if task.status != TaskStatus::Completed {
                                all_success = false;
                            }
                            for msg in task.messages {
                                if msg.role == MessageRole::Agent {
                                    combined.add_message(msg);
                                }
                            }
                            for artifact in task.artifacts {
                                combined.add_artifact(artifact);
                            }
                        }
                        Err(e) => {
                            all_success = false;
                            combined.add_message(UnifiedMessage::agent(format!(
                                "Agent {} failed: {}",
                                self.agents[idx].info().id,
                                e
                            )));
                        }
                    }
                }
                combined.set_status(if all_success {
                    TaskStatus::Completed
                } else {
                    TaskStatus::Failed
                });
            }
        }

        // Store task
        let task_id = combined.id.clone();
        self.tasks.write().await.insert(task_id, combined.clone());

        Ok(combined)
    }

    async fn send_message_to_task(
        &self,
        _task_id: &str,
        message: UnifiedMessage,
    ) -> AgentResult<UnifiedTask> {
        self.send_message(message).await
    }

    async fn send_message_streaming(
        &self,
        message: UnifiedMessage,
    ) -> AgentResult<Pin<Box<dyn Stream<Item = AgentResult<StreamEvent>> + Send>>> {
        let task = self.send_message(message).await?;
        let task_id = task.id.clone();

        let stream = async_stream::stream! {
            yield Ok(StreamEvent::StatusUpdate {
                task_id: task_id.clone(),
                status: TaskStatus::Working,
                message: None,
            });

            for msg in &task.messages {
                yield Ok(StreamEvent::MessageAdded {
                    task_id: task_id.clone(),
                    message: msg.clone(),
                });
            }

            yield Ok(StreamEvent::StatusUpdate {
                task_id: task_id.clone(),
                status: task.status,
                message: None,
            });
        };

        Ok(Box::pin(stream))
    }

    async fn get_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
        self.tasks
            .read()
            .await
            .get(task_id)
            .cloned()
            .ok_or_else(|| AgentError::TaskNotFound(task_id.to_string()))
    }

    async fn cancel_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| AgentError::TaskNotFound(task_id.to_string()))?;
        task.set_status(TaskStatus::Cancelled);
        Ok(task.clone())
    }
}

// ============================================================================
// RouterAgent - Route messages based on rules
// ============================================================================

/// A routing rule for the RouterAgent.
pub struct RoutingRule {
    /// Human-readable name for the rule
    pub name: String,
    /// The condition to check
    pub condition: Box<dyn Fn(&UnifiedMessage) -> bool + Send + Sync>,
    /// The agent to route to if condition matches
    pub target: Arc<dyn UnifiedAgent>,
}

impl RoutingRule {
    /// Create a new routing rule.
    pub fn new(
        name: impl Into<String>,
        condition: impl Fn(&UnifiedMessage) -> bool + Send + Sync + 'static,
        target: Arc<dyn UnifiedAgent>,
    ) -> Self {
        Self {
            name: name.into(),
            condition: Box::new(condition),
            target,
        }
    }

    /// Create a rule that matches messages containing a keyword.
    pub fn keyword(keyword: impl Into<String>, target: Arc<dyn UnifiedAgent>) -> Self {
        let kw = keyword.into();
        let kw_lower = kw.to_lowercase();
        Self::new(
            format!("keyword:{}", kw),
            move |msg| msg.text_content().to_lowercase().contains(&kw_lower),
            target,
        )
    }

    /// Create a rule that matches based on capability.
    pub fn capability(capability_id: impl Into<String>, target: Arc<dyn UnifiedAgent>) -> Self {
        let cap_id = capability_id.into();
        let has_capability = target.capabilities().iter().any(|c| c.id == cap_id);
        Self::new(
            format!("capability:{}", cap_id),
            move |_| has_capability,
            target,
        )
    }
}

/// An agent that routes messages to different agents based on rules.
///
/// Rules are evaluated in order, and the first matching rule determines
/// the target agent. A fallback agent handles messages that match no rules.
///
/// # Example
/// ```rust,ignore
/// let router = RouterAgent::new("task-router", "Task Router")
///     .add_rule(RoutingRule::keyword("weather", weather_agent))
///     .add_rule(RoutingRule::keyword("search", search_agent))
///     .with_fallback(general_agent);
///
/// let result = router.send_message(UnifiedMessage::user("What's the weather?")).await?;
/// ```
pub struct RouterAgent {
    info: AgentInfo,
    rules: Vec<RoutingRule>,
    fallback: Option<Arc<dyn UnifiedAgent>>,
    tasks: tokio::sync::RwLock<HashMap<String, (UnifiedTask, String)>>, // task + routed agent id
}

impl RouterAgent {
    /// Create a new router agent.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            info: AgentInfo::new(id, name).with_description("Message routing agent"),
            rules: Vec::new(),
            fallback: None,
            tasks: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Add a routing rule.
    pub fn add_rule(mut self, rule: RoutingRule) -> Self {
        // Merge capabilities from target
        for cap in rule.target.capabilities() {
            if !self.info.capabilities.iter().any(|c| c.id == cap.id) {
                self.info.capabilities.push(cap.clone());
            }
        }
        self.rules.push(rule);
        self
    }

    /// Set the fallback agent.
    pub fn with_fallback(mut self, agent: Arc<dyn UnifiedAgent>) -> Self {
        // Merge capabilities
        for cap in agent.capabilities() {
            if !self.info.capabilities.iter().any(|c| c.id == cap.id) {
                self.info.capabilities.push(cap.clone());
            }
        }
        self.fallback = Some(agent);
        self
    }

    /// Find the target agent for a message.
    fn find_target(&self, message: &UnifiedMessage) -> Option<(&Arc<dyn UnifiedAgent>, &str)> {
        for rule in &self.rules {
            if (rule.condition)(message) {
                debug!(
                    router = %self.info.id,
                    rule = %rule.name,
                    target = %rule.target.info().id,
                    "Rule matched"
                );
                return Some((&rule.target, &rule.name));
            }
        }
        self.fallback.as_ref().map(|f| (f, "fallback"))
    }
}

#[async_trait]
impl UnifiedAgent for RouterAgent {
    fn info(&self) -> &AgentInfo {
        &self.info
    }

    async fn send_message(&self, message: UnifiedMessage) -> AgentResult<UnifiedTask> {
        let (target, rule_name) = self
            .find_target(&message)
            .ok_or_else(|| AgentError::Internal("No matching route found".to_string()))?;

        info!(
            router = %self.info.id,
            rule = %rule_name,
            target = %target.info().id,
            "Routing message"
        );

        let mut task = target.send_message(message).await?;

        // Add routing metadata
        task.metadata
            .insert("routed_by".to_string(), serde_json::json!(self.info.id));
        task.metadata
            .insert("routing_rule".to_string(), serde_json::json!(rule_name));
        task.metadata.insert(
            "target_agent".to_string(),
            serde_json::json!(target.info().id),
        );

        // Store task
        let task_id = task.id.clone();
        self.tasks
            .write()
            .await
            .insert(task_id, (task.clone(), target.info().id.clone()));

        Ok(task)
    }

    async fn send_message_to_task(
        &self,
        task_id: &str,
        message: UnifiedMessage,
    ) -> AgentResult<UnifiedTask> {
        // Find which agent handled this task
        let tasks = self.tasks.read().await;
        let (_, agent_id) = tasks
            .get(task_id)
            .ok_or_else(|| AgentError::TaskNotFound(task_id.to_string()))?;

        // Find the agent and forward
        let agent = self
            .rules
            .iter()
            .map(|r| &r.target)
            .chain(self.fallback.iter())
            .find(|a| a.info().id == *agent_id)
            .ok_or_else(|| AgentError::Internal("Routed agent not found".to_string()))?;

        drop(tasks);
        agent.send_message_to_task(task_id, message).await
    }

    async fn send_message_streaming(
        &self,
        message: UnifiedMessage,
    ) -> AgentResult<Pin<Box<dyn Stream<Item = AgentResult<StreamEvent>> + Send>>> {
        let (target, _) = self
            .find_target(&message)
            .ok_or_else(|| AgentError::Internal("No matching route found".to_string()))?;

        if target.supports_streaming() {
            target.send_message_streaming(message).await
        } else {
            let task = target.send_message(message).await?;
            let task_id = task.id.clone();

            let stream = async_stream::stream! {
                for msg in &task.messages {
                    yield Ok(StreamEvent::MessageAdded {
                        task_id: task_id.clone(),
                        message: msg.clone(),
                    });
                }
                yield Ok(StreamEvent::StatusUpdate {
                    task_id: task_id.clone(),
                    status: task.status,
                    message: None,
                });
            };

            Ok(Box::pin(stream))
        }
    }

    async fn get_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
        self.tasks
            .read()
            .await
            .get(task_id)
            .map(|(t, _)| t.clone())
            .ok_or_else(|| AgentError::TaskNotFound(task_id.to_string()))
    }

    async fn cancel_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
        let tasks = self.tasks.read().await;
        let (_, agent_id) = tasks
            .get(task_id)
            .ok_or_else(|| AgentError::TaskNotFound(task_id.to_string()))?;

        let agent = self
            .rules
            .iter()
            .map(|r| &r.target)
            .chain(self.fallback.iter())
            .find(|a| a.info().id == *agent_id);

        drop(tasks);

        if let Some(agent) = agent {
            agent.cancel_task(task_id).await
        } else {
            Err(AgentError::Internal("Routed agent not found".to_string()))
        }
    }
}

// ============================================================================
// SupervisorAgent - Coordinate complex workflows
// ============================================================================

/// Decision from the supervisor about what to do next.
#[derive(Debug, Clone)]
pub enum SupervisorDecision {
    /// Route to a specific agent
    RouteToAgent(String),
    /// Execute multiple agents in parallel
    ExecuteParallel(Vec<String>),
    /// Execute agents in sequence
    ExecuteSequence(Vec<String>),
    /// Task is complete
    Complete,
    /// Need more input from user
    NeedInput(String),
    /// Fail with error
    Fail(String),
}

/// Trait for supervisor decision-making logic.
#[async_trait]
pub trait SupervisorLogic: Send + Sync {
    /// Make a decision based on the current task state.
    async fn decide(
        &self,
        task: &UnifiedTask,
        available_agents: &[Arc<dyn UnifiedAgent>],
    ) -> SupervisorDecision;

    /// Process results from agent execution.
    async fn process_results(
        &self,
        task: &mut UnifiedTask,
        results: Vec<AgentResult<UnifiedTask>>,
    ) -> SupervisorDecision;
}

/// A simple capability-based supervisor logic.
pub struct CapabilityBasedSupervisor {
    /// Capability to agent mapping
    capability_map: HashMap<String, String>,
}

impl CapabilityBasedSupervisor {
    /// Create a new capability-based supervisor.
    pub fn new() -> Self {
        Self {
            capability_map: HashMap::new(),
        }
    }

    /// Map a capability to an agent ID.
    pub fn map_capability(
        mut self,
        capability: impl Into<String>,
        agent_id: impl Into<String>,
    ) -> Self {
        self.capability_map
            .insert(capability.into(), agent_id.into());
        self
    }
}

impl Default for CapabilityBasedSupervisor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SupervisorLogic for CapabilityBasedSupervisor {
    async fn decide(
        &self,
        task: &UnifiedTask,
        available_agents: &[Arc<dyn UnifiedAgent>],
    ) -> SupervisorDecision {
        // Get the last user message
        let last_message = task
            .messages
            .iter()
            .filter(|m| m.role == MessageRole::User)
            .next_back();

        if let Some(msg) = last_message {
            let text = msg.text_content().to_lowercase();

            // Check capability map
            for (capability, agent_id) in &self.capability_map {
                if text.contains(capability)
                    && available_agents.iter().any(|a| a.info().id == *agent_id)
                {
                    return SupervisorDecision::RouteToAgent(agent_id.clone());
                }
            }

            // Find first agent with matching capability
            for agent in available_agents {
                for cap in agent.capabilities() {
                    if text.contains(&cap.id.to_lowercase())
                        || cap
                            .description
                            .as_ref()
                            .is_some_and(|d| text.contains(&d.to_lowercase()))
                    {
                        return SupervisorDecision::RouteToAgent(agent.info().id.clone());
                    }
                }
            }
        }

        // No matching agent found
        if !available_agents.is_empty() {
            SupervisorDecision::RouteToAgent(available_agents[0].info().id.clone())
        } else {
            SupervisorDecision::Fail("No available agents".to_string())
        }
    }

    async fn process_results(
        &self,
        _task: &mut UnifiedTask,
        results: Vec<AgentResult<UnifiedTask>>,
    ) -> SupervisorDecision {
        // Check if all succeeded
        let all_ok = results.iter().all(|r| r.is_ok());
        if all_ok {
            SupervisorDecision::Complete
        } else {
            SupervisorDecision::Fail("Some agents failed".to_string())
        }
    }
}

/// An agent that coordinates complex workflows with decision-making.
///
/// The supervisor uses a `SupervisorLogic` implementation to decide
/// how to process tasks, which can involve routing to specific agents,
/// parallel execution, or sequential workflows.
///
/// # Example
/// ```rust,ignore
/// let logic = CapabilityBasedSupervisor::new()
///     .map_capability("weather", "weather-agent")
///     .map_capability("search", "search-agent");
///
/// let supervisor = SupervisorAgent::new("coordinator", "Task Coordinator", logic)
///     .add_agent(weather_agent)
///     .add_agent(search_agent);
///
/// let result = supervisor.send_message(UnifiedMessage::user("What's the weather?")).await?;
/// ```
pub struct SupervisorAgent<L: SupervisorLogic> {
    info: AgentInfo,
    agents: HashMap<String, Arc<dyn UnifiedAgent>>,
    logic: L,
    max_iterations: usize,
    tasks: tokio::sync::RwLock<HashMap<String, UnifiedTask>>,
}

impl<L: SupervisorLogic> SupervisorAgent<L> {
    /// Create a new supervisor agent.
    pub fn new(id: impl Into<String>, name: impl Into<String>, logic: L) -> Self {
        Self {
            info: AgentInfo::new(id, name).with_description("Workflow coordinator"),
            agents: HashMap::new(),
            logic,
            max_iterations: 10,
            tasks: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Add an agent to the supervisor's pool.
    pub fn add_agent(mut self, agent: Arc<dyn UnifiedAgent>) -> Self {
        // Merge capabilities
        for cap in agent.capabilities() {
            if !self.info.capabilities.iter().any(|c| c.id == cap.id) {
                self.info.capabilities.push(cap.clone());
            }
        }
        self.agents.insert(agent.info().id.clone(), agent);
        self
    }

    /// Set maximum iterations to prevent infinite loops.
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    /// Get available agents as a slice.
    fn agents_vec(&self) -> Vec<Arc<dyn UnifiedAgent>> {
        self.agents.values().cloned().collect()
    }
}

#[async_trait]
impl<L: SupervisorLogic + 'static> UnifiedAgent for SupervisorAgent<L> {
    fn info(&self) -> &AgentInfo {
        &self.info
    }

    async fn send_message(&self, message: UnifiedMessage) -> AgentResult<UnifiedTask> {
        let mut task = UnifiedTask::new_with_uuid();
        task.add_message(message);

        let available = self.agents_vec();
        let mut iterations = 0;

        loop {
            if iterations >= self.max_iterations {
                task.add_message(UnifiedMessage::agent(
                    "Maximum iterations reached".to_string(),
                ));
                task.set_status(TaskStatus::Failed);
                break;
            }
            iterations += 1;

            let decision = self.logic.decide(&task, &available).await;
            debug!(
                supervisor = %self.info.id,
                iteration = iterations,
                decision = ?decision,
                "Supervisor decision"
            );

            match decision {
                SupervisorDecision::RouteToAgent(agent_id) => {
                    if let Some(agent) = self.agents.get(&agent_id) {
                        let last_msg = task
                            .messages
                            .iter()
                            .filter(|m| m.role == MessageRole::User)
                            .next_back()
                            .cloned()
                            .unwrap_or_else(|| UnifiedMessage::user(""));

                        match agent.send_message(last_msg).await {
                            Ok(result) => {
                                for msg in &result.messages {
                                    if msg.role == MessageRole::Agent {
                                        task.add_message(msg.clone());
                                    }
                                }
                                for artifact in &result.artifacts {
                                    task.add_artifact(artifact.clone());
                                }
                                // Check if we should continue or complete
                                let results = vec![Ok(result)];
                                let next = self.logic.process_results(&mut task, results).await;
                                if matches!(next, SupervisorDecision::Complete) {
                                    task.set_status(TaskStatus::Completed);
                                    break;
                                }
                            }
                            Err(e) => {
                                task.add_message(UnifiedMessage::agent(format!("Error: {}", e)));
                            }
                        }
                    } else {
                        task.add_message(UnifiedMessage::agent(format!(
                            "Agent {} not found",
                            agent_id
                        )));
                    }
                }

                SupervisorDecision::ExecuteParallel(agent_ids) => {
                    let last_msg = task
                        .messages
                        .iter()
                        .filter(|m| m.role == MessageRole::User)
                        .next_back()
                        .cloned()
                        .unwrap_or_else(|| UnifiedMessage::user(""));

                    let futures: Vec<_> = agent_ids
                        .iter()
                        .filter_map(|id| self.agents.get(id))
                        .map(|a| a.send_message(last_msg.clone()))
                        .collect();

                    let results = futures::future::join_all(futures).await;

                    for t in results.iter().flatten() {
                        for msg in &t.messages {
                            if msg.role == MessageRole::Agent {
                                task.add_message(msg.clone());
                            }
                        }
                        for artifact in &t.artifacts {
                            task.add_artifact(artifact.clone());
                        }
                    }

                    let next = self.logic.process_results(&mut task, results).await;
                    if matches!(next, SupervisorDecision::Complete) {
                        task.set_status(TaskStatus::Completed);
                        break;
                    }
                }

                SupervisorDecision::ExecuteSequence(agent_ids) => {
                    let mut current_input = task
                        .messages
                        .iter()
                        .filter(|m| m.role == MessageRole::User)
                        .next_back()
                        .cloned()
                        .unwrap_or_else(|| UnifiedMessage::user(""));

                    for agent_id in agent_ids {
                        if let Some(agent) = self.agents.get(&agent_id) {
                            match agent.send_message(current_input).await {
                                Ok(result) => {
                                    // Get output for next stage
                                    let output = result
                                        .messages
                                        .iter()
                                        .filter(|m| m.role == MessageRole::Agent)
                                        .next_back()
                                        .map(|m| m.text_content())
                                        .unwrap_or_default();

                                    for msg in result.messages {
                                        if msg.role == MessageRole::Agent {
                                            task.add_message(msg);
                                        }
                                    }
                                    for artifact in result.artifacts {
                                        task.add_artifact(artifact);
                                    }

                                    current_input = UnifiedMessage::user(output);
                                }
                                Err(e) => {
                                    task.add_message(UnifiedMessage::agent(format!(
                                        "Agent {} failed: {}",
                                        agent_id, e
                                    )));
                                    break;
                                }
                            }
                        }
                    }
                    task.set_status(TaskStatus::Completed);
                    break;
                }

                SupervisorDecision::Complete => {
                    task.set_status(TaskStatus::Completed);
                    break;
                }

                SupervisorDecision::NeedInput(prompt) => {
                    task.add_message(UnifiedMessage::agent(prompt));
                    task.set_status(TaskStatus::InputRequired);
                    break;
                }

                SupervisorDecision::Fail(reason) => {
                    task.add_message(UnifiedMessage::agent(format!("Failed: {}", reason)));
                    task.set_status(TaskStatus::Failed);
                    break;
                }
            }
        }

        // Store task
        let task_id = task.id.clone();
        self.tasks.write().await.insert(task_id, task.clone());

        Ok(task)
    }

    async fn send_message_to_task(
        &self,
        task_id: &str,
        message: UnifiedMessage,
    ) -> AgentResult<UnifiedTask> {
        // Get existing task and continue
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.add_message(message.clone());
            task.set_status(TaskStatus::Working);
            drop(tasks);

            // Re-run supervisor logic
            return self.send_message(message).await;
        }
        Err(AgentError::TaskNotFound(task_id.to_string()))
    }

    async fn send_message_streaming(
        &self,
        message: UnifiedMessage,
    ) -> AgentResult<Pin<Box<dyn Stream<Item = AgentResult<StreamEvent>> + Send>>> {
        let task = self.send_message(message).await?;
        let task_id = task.id.clone();

        let stream = async_stream::stream! {
            yield Ok(StreamEvent::StatusUpdate {
                task_id: task_id.clone(),
                status: TaskStatus::Working,
                message: None,
            });

            for msg in &task.messages {
                yield Ok(StreamEvent::MessageAdded {
                    task_id: task_id.clone(),
                    message: msg.clone(),
                });
            }

            yield Ok(StreamEvent::StatusUpdate {
                task_id: task_id.clone(),
                status: task.status,
                message: None,
            });
        };

        Ok(Box::pin(stream))
    }

    async fn get_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
        self.tasks
            .read()
            .await
            .get(task_id)
            .cloned()
            .ok_or_else(|| AgentError::TaskNotFound(task_id.to_string()))
    }

    async fn cancel_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| AgentError::TaskNotFound(task_id.to_string()))?;
        task.set_status(TaskStatus::Cancelled);
        Ok(task.clone())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Capability;

    /// A simple mock agent for testing.
    struct MockAgent {
        info: AgentInfo,
        response: String,
    }

    impl MockAgent {
        fn new(id: &str, response: &str) -> Arc<Self> {
            Arc::new(Self {
                info: AgentInfo::new(id, id).with_capability(Capability::new(id, id)),
                response: response.to_string(),
            })
        }
    }

    #[async_trait]
    impl UnifiedAgent for MockAgent {
        fn info(&self) -> &AgentInfo {
            &self.info
        }

        async fn send_message(&self, message: UnifiedMessage) -> AgentResult<UnifiedTask> {
            let mut task = UnifiedTask::new_with_uuid();
            task.add_message(message);
            task.add_message(UnifiedMessage::agent(&self.response));
            task.set_status(TaskStatus::Completed);
            Ok(task)
        }

        async fn send_message_to_task(
            &self,
            _task_id: &str,
            message: UnifiedMessage,
        ) -> AgentResult<UnifiedTask> {
            self.send_message(message).await
        }

        async fn send_message_streaming(
            &self,
            message: UnifiedMessage,
        ) -> AgentResult<Pin<Box<dyn Stream<Item = AgentResult<StreamEvent>> + Send>>> {
            let task = self.send_message(message).await?;
            let task_id = task.id.clone();

            let stream = async_stream::stream! {
                yield Ok(StreamEvent::StatusUpdate {
                    task_id: task_id.clone(),
                    status: TaskStatus::Completed,
                    message: None,
                });
            };

            Ok(Box::pin(stream))
        }

        async fn get_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
            Err(AgentError::TaskNotFound(task_id.to_string()))
        }

        async fn cancel_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
            Err(AgentError::TaskNotFound(task_id.to_string()))
        }
    }

    #[tokio::test]
    async fn test_sequential_pipeline() {
        let agent1 = MockAgent::new("agent1", "Step 1 complete");
        let agent2 = MockAgent::new("agent2", "Step 2 complete");

        let pipeline = SequentialPipeline::new("test-pipeline", "Test Pipeline")
            .add_stage(agent1)
            .add_stage(agent2);

        assert_eq!(pipeline.stage_count(), 2);

        let result = pipeline
            .send_message(UnifiedMessage::user("Start"))
            .await
            .unwrap();

        assert_eq!(result.status, TaskStatus::Completed);
        // Should have messages from both stages
        assert!(result.messages.len() >= 2);
    }

    #[tokio::test]
    async fn test_parallel_agent() {
        let agent1 = MockAgent::new("search1", "Result from search 1");
        let agent2 = MockAgent::new("search2", "Result from search 2");

        let parallel = ParallelAgent::new("multi-search", "Multi Search")
            .add_agent(agent1)
            .add_agent(agent2)
            .with_aggregation(AggregationMode::CollectAll);

        assert_eq!(parallel.agent_count(), 2);

        let result = parallel
            .send_message(UnifiedMessage::user("Search query"))
            .await
            .unwrap();

        assert_eq!(result.status, TaskStatus::Completed);
        // Should have results from both agents
        let agent_messages: Vec<_> = result
            .messages
            .iter()
            .filter(|m| m.role == MessageRole::Agent)
            .collect();
        assert_eq!(agent_messages.len(), 2);
    }

    #[tokio::test]
    async fn test_router_agent() {
        let weather_agent = MockAgent::new("weather", "Sunny and warm");
        let search_agent = MockAgent::new("search", "Search results");

        let router = RouterAgent::new("router", "Task Router")
            .add_rule(RoutingRule::keyword("weather", weather_agent))
            .with_fallback(search_agent);

        let result = router
            .send_message(UnifiedMessage::user("What's the weather today?"))
            .await
            .unwrap();

        assert_eq!(result.status, TaskStatus::Completed);
        assert!(
            result
                .messages
                .iter()
                .any(|m| m.text_content().contains("Sunny"))
        );
    }

    #[tokio::test]
    async fn test_router_fallback() {
        let fallback = MockAgent::new("fallback", "Fallback response");

        let router = RouterAgent::new("router", "Router").with_fallback(fallback);

        let result = router
            .send_message(UnifiedMessage::user("Random query"))
            .await
            .unwrap();

        assert_eq!(result.status, TaskStatus::Completed);
        assert!(
            result
                .messages
                .iter()
                .any(|m| m.text_content().contains("Fallback"))
        );
    }

    #[tokio::test]
    async fn test_supervisor_agent() {
        let agent1 = MockAgent::new("helper", "I can help!");

        let logic = CapabilityBasedSupervisor::new().map_capability("help", "helper");

        let supervisor = SupervisorAgent::new("supervisor", "Supervisor", logic).add_agent(agent1);

        let result = supervisor
            .send_message(UnifiedMessage::user("I need help with something"))
            .await
            .unwrap();

        assert_eq!(result.status, TaskStatus::Completed);
    }

    #[test]
    fn test_transform_mode_default() {
        let mode = TransformMode::default();
        assert!(matches!(mode, TransformMode::LastMessage));
    }

    #[test]
    fn test_aggregation_mode_default() {
        let mode = AggregationMode::default();
        assert!(matches!(mode, AggregationMode::CollectAll));
    }
}
