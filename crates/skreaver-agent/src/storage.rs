//! Persistent task storage for agent workflows.
//!
//! This module provides abstractions for storing and retrieving tasks,
//! enabling workflow persistence, recovery, and history tracking.
//!
//! # Features
//!
//! - **TaskStore trait**: Async interface for task persistence
//! - **InMemoryTaskStore**: Fast in-memory storage for testing/development
//! - **FileTaskStore**: JSON file-based storage for simple persistence
//! - **Query support**: Filter tasks by status, time range, session
//!
//! # Example: Basic Usage
//!
//! ```rust,ignore
//! use skreaver_agent::{TaskStore, InMemoryTaskStore, UnifiedTask};
//!
//! let store = InMemoryTaskStore::new();
//!
//! // Save a task
//! let task = UnifiedTask::new_with_uuid();
//! store.save(&task).await?;
//!
//! // Retrieve later
//! if let Some(task) = store.get(&task.id).await? {
//!     println!("Found task: {}", task.id);
//! }
//!
//! // Query by status
//! let pending = store.query(TaskQuery::new().with_status(TaskStatus::Pending)).await?;
//! ```
//!
//! # Example: File-based Persistence
//!
//! ```rust,ignore
//! use skreaver_agent::{FileTaskStore, UnifiedTask};
//!
//! // Store tasks in a directory
//! let store = FileTaskStore::new("/var/lib/skreaver/tasks")?;
//!
//! let task = UnifiedTask::new_with_uuid();
//! store.save(&task).await?;
//!
//! // Tasks persist across restarts
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::error::{AgentError, AgentResult};
use crate::types::{TaskStatus, UnifiedTask};

// ============================================================================
// Task Query
// ============================================================================

/// Query parameters for filtering tasks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskQuery {
    /// Filter by task status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<TaskStatus>,
    /// Filter by multiple statuses (OR)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub statuses: Vec<TaskStatus>,
    /// Filter by session ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Filter by creation time (after)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_after: Option<DateTime<Utc>>,
    /// Filter by creation time (before)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_before: Option<DateTime<Utc>>,
    /// Filter by update time (after)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_after: Option<DateTime<Utc>>,
    /// Filter by update time (before)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_before: Option<DateTime<Utc>>,
    /// Filter by metadata key presence
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_metadata_key: Option<String>,
    /// Include only terminal tasks
    #[serde(default)]
    pub terminal_only: bool,
    /// Include only non-terminal tasks
    #[serde(default)]
    pub active_only: bool,
    /// Maximum number of results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    /// Offset for pagination
    #[serde(default)]
    pub offset: usize,
    /// Sort order (true = newest first)
    #[serde(default = "TaskQuery::default_newest_first")]
    pub newest_first: bool,
}

impl TaskQuery {
    /// Default value for newest_first field (used by serde).
    fn default_newest_first() -> bool {
        true
    }
    /// Create a new empty query.
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by single status.
    pub fn with_status(mut self, status: TaskStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Filter by multiple statuses (OR).
    pub fn with_statuses(mut self, statuses: Vec<TaskStatus>) -> Self {
        self.statuses = statuses;
        self
    }

    /// Filter by session ID.
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Filter by creation time range.
    pub fn created_between(mut self, after: DateTime<Utc>, before: DateTime<Utc>) -> Self {
        self.created_after = Some(after);
        self.created_before = Some(before);
        self
    }

    /// Filter by tasks created after a time.
    pub fn created_after(mut self, after: DateTime<Utc>) -> Self {
        self.created_after = Some(after);
        self
    }

    /// Filter by tasks updated after a time.
    pub fn updated_after(mut self, after: DateTime<Utc>) -> Self {
        self.updated_after = Some(after);
        self
    }

    /// Include only terminal tasks.
    pub fn terminal_only(mut self) -> Self {
        self.terminal_only = true;
        self.active_only = false;
        self
    }

    /// Include only active (non-terminal) tasks.
    pub fn active_only(mut self) -> Self {
        self.active_only = true;
        self.terminal_only = false;
        self
    }

    /// Require a metadata key.
    pub fn with_metadata_key(mut self, key: impl Into<String>) -> Self {
        self.has_metadata_key = Some(key.into());
        self
    }

    /// Set maximum results.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set offset for pagination.
    pub fn with_offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }

    /// Sort oldest first.
    pub fn oldest_first(mut self) -> Self {
        self.newest_first = false;
        self
    }

    /// Check if a task matches this query.
    pub fn matches(&self, task: &UnifiedTask) -> bool {
        // Check single status
        if let Some(status) = self.status
            && task.status != status
        {
            return false;
        }

        // Check multiple statuses (OR)
        if !self.statuses.is_empty() && !self.statuses.contains(&task.status) {
            return false;
        }

        // Check session ID
        if self
            .session_id
            .as_ref()
            .is_some_and(|s| task.session_id.as_ref() != Some(s))
        {
            return false;
        }

        // Check created_after
        if let Some(after) = self.created_after
            && task.created_at.is_some_and(|t| t < after)
        {
            return false;
        }

        // Check created_before
        if let Some(before) = self.created_before
            && task.created_at.is_some_and(|t| t > before)
        {
            return false;
        }

        // Check updated_after
        if let Some(after) = self.updated_after
            && task.updated_at.is_some_and(|t| t < after)
        {
            return false;
        }

        // Check updated_before
        if let Some(before) = self.updated_before
            && task.updated_at.is_some_and(|t| t > before)
        {
            return false;
        }

        // Check metadata key
        if self
            .has_metadata_key
            .as_ref()
            .is_some_and(|k| !task.metadata.contains_key(k))
        {
            return false;
        }

        // Check terminal_only
        if self.terminal_only && !task.is_terminal() {
            return false;
        }

        // Check active_only
        if self.active_only && task.is_terminal() {
            return false;
        }

        true
    }
}

// ============================================================================
// Task Store Trait
// ============================================================================

/// Async trait for task persistence.
#[async_trait]
pub trait TaskStore: Send + Sync {
    /// Save or update a task.
    async fn save(&self, task: &UnifiedTask) -> AgentResult<()>;

    /// Get a task by ID.
    async fn get(&self, task_id: &str) -> AgentResult<Option<UnifiedTask>>;

    /// Delete a task by ID.
    async fn delete(&self, task_id: &str) -> AgentResult<bool>;

    /// Query tasks with filters.
    async fn query(&self, query: &TaskQuery) -> AgentResult<Vec<UnifiedTask>>;

    /// List all task IDs.
    async fn list_ids(&self) -> AgentResult<Vec<String>>;

    /// Count tasks matching a query.
    async fn count(&self, query: &TaskQuery) -> AgentResult<usize> {
        Ok(self.query(query).await?.len())
    }

    /// Check if a task exists.
    async fn exists(&self, task_id: &str) -> AgentResult<bool> {
        Ok(self.get(task_id).await?.is_some())
    }

    /// Get multiple tasks by IDs.
    async fn get_many(&self, task_ids: &[String]) -> AgentResult<Vec<UnifiedTask>> {
        let mut tasks = Vec::new();
        for id in task_ids {
            if let Some(task) = self.get(id).await? {
                tasks.push(task);
            }
        }
        Ok(tasks)
    }

    /// Delete all tasks matching a query.
    async fn delete_matching(&self, query: &TaskQuery) -> AgentResult<usize> {
        let tasks = self.query(query).await?;
        let mut deleted = 0;
        for task in tasks {
            if self.delete(&task.id).await? {
                deleted += 1;
            }
        }
        Ok(deleted)
    }

    /// Clear all tasks.
    async fn clear(&self) -> AgentResult<usize>;
}

// ============================================================================
// Task Cache (Lightweight)
// ============================================================================

/// Lightweight in-memory task cache for agent implementations.
///
/// This is a simpler alternative to `InMemoryTaskStore` for agents that just need
/// basic get/save/cancel operations without full query support.
///
/// Thread-safe with interior mutability via `RwLock`.
///
/// # Example
///
/// ```rust,ignore
/// use skreaver_agent::storage::TaskCache;
/// use skreaver_agent::types::UnifiedTask;
///
/// let cache = TaskCache::new();
///
/// // Store a task
/// let task = UnifiedTask::new_with_uuid();
/// cache.insert(task.clone()).await;
///
/// // Retrieve it
/// if let Some(task) = cache.get(&task.id).await {
///     println!("Found task: {}", task.id);
/// }
/// ```
#[derive(Debug, Default)]
pub struct TaskCache {
    tasks: RwLock<HashMap<String, UnifiedTask>>,
}

impl TaskCache {
    /// Create a new empty task cache.
    pub fn new() -> Self {
        Self {
            tasks: RwLock::new(HashMap::new()),
        }
    }

    /// Insert or update a task in the cache.
    pub async fn insert(&self, task: UnifiedTask) {
        self.tasks.write().await.insert(task.id.clone(), task);
    }

    /// Get a task by ID.
    pub async fn get(&self, task_id: &str) -> Option<UnifiedTask> {
        self.tasks.read().await.get(task_id).cloned()
    }

    /// Get a mutable reference to a task and apply a function to it.
    ///
    /// Returns the result of applying the function, or None if task not found.
    pub async fn update<F, R>(&self, task_id: &str, f: F) -> Option<R>
    where
        F: FnOnce(&mut UnifiedTask) -> R,
    {
        let mut tasks = self.tasks.write().await;
        tasks.get_mut(task_id).map(f)
    }

    /// Remove a task from the cache.
    pub async fn remove(&self, task_id: &str) -> Option<UnifiedTask> {
        self.tasks.write().await.remove(task_id)
    }

    /// Check if a task exists.
    pub async fn contains(&self, task_id: &str) -> bool {
        self.tasks.read().await.contains_key(task_id)
    }

    /// Get the number of cached tasks.
    pub async fn len(&self) -> usize {
        self.tasks.read().await.len()
    }

    /// Check if the cache is empty.
    pub async fn is_empty(&self) -> bool {
        self.tasks.read().await.is_empty()
    }

    /// Clear all tasks from the cache.
    pub async fn clear(&self) {
        self.tasks.write().await.clear();
    }

    /// Get all task IDs.
    pub async fn task_ids(&self) -> Vec<String> {
        self.tasks.read().await.keys().cloned().collect()
    }

    /// Get all tasks.
    pub async fn all_tasks(&self) -> Vec<UnifiedTask> {
        self.tasks.read().await.values().cloned().collect()
    }
}

// ============================================================================
// In-Memory Task Store
// ============================================================================

/// In-memory task store for testing and development.
///
/// Tasks are stored in memory and lost when the process exits.
/// Thread-safe with interior mutability.
#[derive(Debug)]
pub struct InMemoryTaskStore {
    tasks: RwLock<HashMap<String, UnifiedTask>>,
}

impl InMemoryTaskStore {
    /// Create a new in-memory store.
    pub fn new() -> Self {
        Self {
            tasks: RwLock::new(HashMap::new()),
        }
    }

    /// Create a store wrapped in Arc for sharing.
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    /// Get the number of stored tasks.
    pub async fn len(&self) -> usize {
        self.tasks.read().await.len()
    }

    /// Check if the store is empty.
    pub async fn is_empty(&self) -> bool {
        self.tasks.read().await.is_empty()
    }
}

impl Default for InMemoryTaskStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskStore for InMemoryTaskStore {
    async fn save(&self, task: &UnifiedTask) -> AgentResult<()> {
        let mut tasks = self.tasks.write().await;
        debug!(task_id = %task.id, status = %task.status, "Saving task to memory");
        tasks.insert(task.id.clone(), task.clone());
        Ok(())
    }

    async fn get(&self, task_id: &str) -> AgentResult<Option<UnifiedTask>> {
        let tasks = self.tasks.read().await;
        Ok(tasks.get(task_id).cloned())
    }

    async fn delete(&self, task_id: &str) -> AgentResult<bool> {
        let mut tasks = self.tasks.write().await;
        Ok(tasks.remove(task_id).is_some())
    }

    async fn query(&self, query: &TaskQuery) -> AgentResult<Vec<UnifiedTask>> {
        let tasks = self.tasks.read().await;
        let mut results: Vec<_> = tasks
            .values()
            .filter(|t| query.matches(t))
            .cloned()
            .collect();

        // Sort by created_at
        results.sort_by(|a, b| {
            let a_time = a.created_at.unwrap_or(DateTime::UNIX_EPOCH);
            let b_time = b.created_at.unwrap_or(DateTime::UNIX_EPOCH);
            if query.newest_first {
                b_time.cmp(&a_time)
            } else {
                a_time.cmp(&b_time)
            }
        });

        // Apply offset
        let results: Vec<_> = results.into_iter().skip(query.offset).collect();

        // Apply limit
        let results = if let Some(limit) = query.limit {
            results.into_iter().take(limit).collect()
        } else {
            results
        };

        Ok(results)
    }

    async fn list_ids(&self) -> AgentResult<Vec<String>> {
        let tasks = self.tasks.read().await;
        Ok(tasks.keys().cloned().collect())
    }

    async fn clear(&self) -> AgentResult<usize> {
        let mut tasks = self.tasks.write().await;
        let count = tasks.len();
        tasks.clear();
        Ok(count)
    }
}

// ============================================================================
// File-based Task Store
// ============================================================================

/// File-based task store using JSON files.
///
/// Each task is stored as a separate JSON file in the specified directory.
/// Suitable for simple persistence needs with low to moderate throughput.
#[derive(Debug)]
pub struct FileTaskStore {
    directory: PathBuf,
    /// Optional in-memory cache for faster reads
    cache: Option<RwLock<HashMap<String, UnifiedTask>>>,
}

impl FileTaskStore {
    /// Create a new file-based store.
    ///
    /// Creates the directory if it doesn't exist.
    pub fn new(directory: impl Into<PathBuf>) -> AgentResult<Self> {
        let dir = directory.into();
        std::fs::create_dir_all(&dir)?;
        info!(directory = %dir.display(), "Created file task store");
        Ok(Self {
            directory: dir,
            cache: None,
        })
    }

    /// Create with in-memory caching for faster reads.
    pub fn with_cache(directory: impl Into<PathBuf>) -> AgentResult<Self> {
        let dir = directory.into();
        std::fs::create_dir_all(&dir)?;
        info!(directory = %dir.display(), "Created cached file task store");
        Ok(Self {
            directory: dir,
            cache: Some(RwLock::new(HashMap::new())),
        })
    }

    /// Create a store wrapped in Arc for sharing.
    pub fn shared(directory: impl Into<PathBuf>) -> AgentResult<Arc<Self>> {
        Ok(Arc::new(Self::new(directory)?))
    }

    /// Get the file path for a task.
    fn task_path(&self, task_id: &str) -> PathBuf {
        self.directory.join(format!("{}.json", task_id))
    }

    /// Load all tasks into cache (call once at startup if using cache).
    pub async fn load_cache(&self) -> AgentResult<usize> {
        if let Some(ref cache) = self.cache {
            let mut cache_guard = cache.write().await;
            cache_guard.clear();

            let entries = std::fs::read_dir(&self.directory)?;
            let mut count = 0;

            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "json") {
                    match std::fs::read_to_string(&path) {
                        Ok(content) => match serde_json::from_str::<UnifiedTask>(&content) {
                            Ok(task) => {
                                cache_guard.insert(task.id.clone(), task);
                                count += 1;
                            }
                            Err(e) => {
                                warn!(path = %path.display(), error = %e, "Failed to parse task file");
                            }
                        },
                        Err(e) => {
                            warn!(path = %path.display(), error = %e, "Failed to read task file");
                        }
                    }
                }
            }

            info!(count, "Loaded tasks into cache");
            Ok(count)
        } else {
            Ok(0)
        }
    }
}

#[async_trait]
impl TaskStore for FileTaskStore {
    async fn save(&self, task: &UnifiedTask) -> AgentResult<()> {
        let path = self.task_path(&task.id);
        let content = serde_json::to_string_pretty(task)?;
        let task_id = task.id.clone();

        // Write atomically using temp file + rename (blocking I/O)
        let temp_path = path.with_extension("json.tmp");
        let path_clone = path.clone();
        tokio::task::spawn_blocking(move || -> std::io::Result<()> {
            std::fs::write(&temp_path, &content)?;
            std::fs::rename(&temp_path, &path_clone)?;
            Ok(())
        })
        .await
        .map_err(|e| AgentError::Internal(format!("Task join error: {}", e)))??;

        debug!(task_id = %task_id, path = %path.display(), "Saved task to file");

        // Update cache if enabled
        if let Some(ref cache) = self.cache {
            cache.write().await.insert(task.id.clone(), task.clone());
        }

        Ok(())
    }

    async fn get(&self, task_id: &str) -> AgentResult<Option<UnifiedTask>> {
        // Check cache first
        if let Some(ref cache) = self.cache {
            let cache_guard = cache.read().await;
            if let Some(task) = cache_guard.get(task_id) {
                return Ok(Some(task.clone()));
            }
        }

        // Read from file
        let path = self.task_path(task_id);
        if !path.exists() {
            return Ok(None);
        }

        let path_clone = path.clone();
        let content = tokio::task::spawn_blocking(move || std::fs::read_to_string(&path_clone))
            .await
            .map_err(|e| AgentError::Internal(format!("Task join error: {}", e)))??;
        let task: UnifiedTask = serde_json::from_str(&content)?;

        // Update cache
        if let Some(ref cache) = self.cache {
            cache.write().await.insert(task.id.clone(), task.clone());
        }

        Ok(Some(task))
    }

    async fn delete(&self, task_id: &str) -> AgentResult<bool> {
        let path = self.task_path(task_id);

        // Remove from cache
        if let Some(ref cache) = self.cache {
            cache.write().await.remove(task_id);
        }

        if path.exists() {
            let path_clone = path.clone();
            tokio::task::spawn_blocking(move || std::fs::remove_file(&path_clone))
                .await
                .map_err(|e| AgentError::Internal(format!("Task join error: {}", e)))??;
            debug!(task_id = %task_id, "Deleted task file");
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn query(&self, query: &TaskQuery) -> AgentResult<Vec<UnifiedTask>> {
        // If we have a cache, use it
        if let Some(ref cache) = self.cache {
            let cache_guard = cache.read().await;
            let mut results: Vec<_> = cache_guard
                .values()
                .filter(|t| query.matches(t))
                .cloned()
                .collect();

            // Sort
            results.sort_by(|a, b| {
                let a_time = a.created_at.unwrap_or(DateTime::UNIX_EPOCH);
                let b_time = b.created_at.unwrap_or(DateTime::UNIX_EPOCH);
                if query.newest_first {
                    b_time.cmp(&a_time)
                } else {
                    a_time.cmp(&b_time)
                }
            });

            // Apply offset and limit
            let results: Vec<_> = results.into_iter().skip(query.offset).collect();
            let results = if let Some(limit) = query.limit {
                results.into_iter().take(limit).collect()
            } else {
                results
            };

            return Ok(results);
        }

        // No cache - read all files
        let dir = self.directory.clone();
        let results = tokio::task::spawn_blocking(move || -> AgentResult<Vec<UnifiedTask>> {
            let mut tasks = Vec::new();
            let entries = std::fs::read_dir(&dir)?;

            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "json")
                    && let Ok(content) = std::fs::read_to_string(&path)
                    && let Ok(task) = serde_json::from_str::<UnifiedTask>(&content)
                {
                    tasks.push(task);
                }
            }
            Ok(tasks)
        })
        .await
        .map_err(|e| AgentError::Internal(format!("Task join error: {}", e)))??;

        // Filter by query
        let mut results: Vec<_> = results.into_iter().filter(|t| query.matches(t)).collect();

        // Sort
        results.sort_by(|a, b| {
            let a_time = a.created_at.unwrap_or(DateTime::UNIX_EPOCH);
            let b_time = b.created_at.unwrap_or(DateTime::UNIX_EPOCH);
            if query.newest_first {
                b_time.cmp(&a_time)
            } else {
                a_time.cmp(&b_time)
            }
        });

        // Apply offset and limit
        let results: Vec<_> = results.into_iter().skip(query.offset).collect();
        let results = if let Some(limit) = query.limit {
            results.into_iter().take(limit).collect()
        } else {
            results
        };

        Ok(results)
    }

    async fn list_ids(&self) -> AgentResult<Vec<String>> {
        let mut ids = Vec::new();
        let entries = std::fs::read_dir(&self.directory)?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json")
                && let Some(stem) = path.file_stem()
            {
                ids.push(stem.to_string_lossy().to_string());
            }
        }

        Ok(ids)
    }

    async fn clear(&self) -> AgentResult<usize> {
        let ids = self.list_ids().await?;
        let mut count = 0;

        for id in &ids {
            if self.delete(id).await? {
                count += 1;
            }
        }

        // Clear cache
        if let Some(ref cache) = self.cache {
            cache.write().await.clear();
        }

        Ok(count)
    }
}

// ============================================================================
// Task Store Extensions
// ============================================================================

/// Extension trait for common task store operations.
#[async_trait]
pub trait TaskStoreExt: TaskStore {
    /// Get all active (non-terminal) tasks.
    async fn get_active_tasks(&self) -> AgentResult<Vec<UnifiedTask>> {
        self.query(&TaskQuery::new().active_only()).await
    }

    /// Get all tasks for a session.
    async fn get_session_tasks(&self, session_id: &str) -> AgentResult<Vec<UnifiedTask>> {
        self.query(&TaskQuery::new().with_session(session_id)).await
    }

    /// Get recent tasks.
    async fn get_recent(&self, limit: usize) -> AgentResult<Vec<UnifiedTask>> {
        self.query(&TaskQuery::new().with_limit(limit)).await
    }

    /// Get tasks by status.
    async fn get_by_status(&self, status: TaskStatus) -> AgentResult<Vec<UnifiedTask>> {
        self.query(&TaskQuery::new().with_status(status)).await
    }

    /// Archive old terminal tasks (move to different store or mark).
    async fn cleanup_old_tasks(&self, older_than: DateTime<Utc>) -> AgentResult<usize> {
        self.delete_matching(
            &TaskQuery::new()
                .terminal_only()
                .created_between(DateTime::UNIX_EPOCH, older_than),
        )
        .await
    }
}

// Blanket implementation for all TaskStore types
impl<T: TaskStore + ?Sized> TaskStoreExt for T {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::UnifiedMessage;

    #[test]
    fn test_task_query_builder() {
        let query = TaskQuery::new()
            .with_status(TaskStatus::Completed)
            .with_session("session-1")
            .with_limit(10)
            .oldest_first();

        assert_eq!(query.status, Some(TaskStatus::Completed));
        assert_eq!(query.session_id, Some("session-1".to_string()));
        assert_eq!(query.limit, Some(10));
        assert!(!query.newest_first);
    }

    #[test]
    fn test_task_query_matches() {
        let mut task = UnifiedTask::new("task-1");
        task.session_id = Some("session-1".to_string());
        task.set_status(TaskStatus::Completed);

        // Basic status match
        assert!(
            TaskQuery::new()
                .with_status(TaskStatus::Completed)
                .matches(&task)
        );
        assert!(
            !TaskQuery::new()
                .with_status(TaskStatus::Pending)
                .matches(&task)
        );

        // Session match
        assert!(TaskQuery::new().with_session("session-1").matches(&task));
        assert!(!TaskQuery::new().with_session("session-2").matches(&task));

        // Terminal only
        assert!(TaskQuery::new().terminal_only().matches(&task));
        assert!(!TaskQuery::new().active_only().matches(&task));

        // Multiple statuses
        assert!(
            TaskQuery::new()
                .with_statuses(vec![TaskStatus::Completed, TaskStatus::Failed])
                .matches(&task)
        );
    }

    #[tokio::test]
    async fn test_in_memory_store() {
        let store = InMemoryTaskStore::new();

        // Save a task
        let mut task = UnifiedTask::new("task-1");
        task.add_message(UnifiedMessage::user("Hello"));
        store.save(&task).await.unwrap();

        // Retrieve it
        let retrieved = store.get("task-1").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "task-1");

        // Query
        let all = store.query(&TaskQuery::new()).await.unwrap();
        assert_eq!(all.len(), 1);

        // Delete
        assert!(store.delete("task-1").await.unwrap());
        assert!(store.get("task-1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_in_memory_store_query() {
        let store = InMemoryTaskStore::new();

        // Create tasks with different statuses
        let mut task1 = UnifiedTask::new("task-1");
        task1.set_status(TaskStatus::Completed);
        store.save(&task1).await.unwrap();

        let mut task2 = UnifiedTask::new("task-2");
        task2.set_status(TaskStatus::Working);
        store.save(&task2).await.unwrap();

        let mut task3 = UnifiedTask::new("task-3");
        task3.set_status(TaskStatus::Failed);
        store.save(&task3).await.unwrap();

        // Query by status
        let completed = store
            .query(&TaskQuery::new().with_status(TaskStatus::Completed))
            .await
            .unwrap();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, "task-1");

        // Query terminal only
        let terminal = store
            .query(&TaskQuery::new().terminal_only())
            .await
            .unwrap();
        assert_eq!(terminal.len(), 2); // Completed and Failed

        // Query active only
        let active = store.query(&TaskQuery::new().active_only()).await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, "task-2");
    }

    #[tokio::test]
    async fn test_in_memory_store_pagination() {
        let store = InMemoryTaskStore::new();

        // Create 5 tasks
        for i in 0..5 {
            let task = UnifiedTask::new(format!("task-{}", i));
            store.save(&task).await.unwrap();
        }

        // Get first 2
        let page1 = store
            .query(&TaskQuery::new().with_limit(2).oldest_first())
            .await
            .unwrap();
        assert_eq!(page1.len(), 2);

        // Get next 2
        let page2 = store
            .query(&TaskQuery::new().with_limit(2).with_offset(2).oldest_first())
            .await
            .unwrap();
        assert_eq!(page2.len(), 2);

        // Get last 1
        let page3 = store
            .query(&TaskQuery::new().with_limit(2).with_offset(4).oldest_first())
            .await
            .unwrap();
        assert_eq!(page3.len(), 1);
    }

    #[tokio::test]
    async fn test_file_store() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = FileTaskStore::new(temp_dir.path()).unwrap();

        // Save a task
        let mut task = UnifiedTask::new("task-file-1");
        task.add_message(UnifiedMessage::user("Test"));
        store.save(&task).await.unwrap();

        // Verify file exists
        let path = temp_dir.path().join("task-file-1.json");
        assert!(path.exists());

        // Retrieve it
        let retrieved = store.get("task-file-1").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().messages.len(), 1);

        // List IDs
        let ids = store.list_ids().await.unwrap();
        assert!(ids.contains(&"task-file-1".to_string()));

        // Delete
        assert!(store.delete("task-file-1").await.unwrap());
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn test_file_store_with_cache() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = FileTaskStore::with_cache(temp_dir.path()).unwrap();

        // Save tasks
        for i in 0..3 {
            let task = UnifiedTask::new(format!("cached-{}", i));
            store.save(&task).await.unwrap();
        }

        // Load cache
        let count = store.load_cache().await.unwrap();
        assert_eq!(count, 3);

        // Query uses cache
        let all = store.query(&TaskQuery::new()).await.unwrap();
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn test_task_store_ext() {
        let store = InMemoryTaskStore::new();

        // Create tasks in different states
        let mut task1 = UnifiedTask::new("active-1").with_session("session-a");
        task1.set_status(TaskStatus::Working);
        store.save(&task1).await.unwrap();

        let mut task2 = UnifiedTask::new("done-1").with_session("session-a");
        task2.set_status(TaskStatus::Completed);
        store.save(&task2).await.unwrap();

        // Get active tasks
        let active = store.get_active_tasks().await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, "active-1");

        // Get session tasks
        let session = store.get_session_tasks("session-a").await.unwrap();
        assert_eq!(session.len(), 2);

        // Get by status
        let completed = store.get_by_status(TaskStatus::Completed).await.unwrap();
        assert_eq!(completed.len(), 1);
    }

    #[tokio::test]
    async fn test_store_clear() {
        let store = InMemoryTaskStore::new();

        for i in 0..5 {
            store
                .save(&UnifiedTask::new(format!("t-{}", i)))
                .await
                .unwrap();
        }

        assert_eq!(store.len().await, 5);

        let cleared = store.clear().await.unwrap();
        assert_eq!(cleared, 5);
        assert!(store.is_empty().await);
    }
}
