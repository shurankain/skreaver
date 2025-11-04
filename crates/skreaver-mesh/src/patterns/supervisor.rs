//! Supervisor/Worker pattern for task distribution
//!
//! Implements a supervisor that distributes tasks to a pool of workers,
//! with load balancing, health monitoring, and fault tolerance.

use crate::{error::MeshResult, mesh::AgentMesh, message::Message, types::AgentId};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Task status in the system
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    /// Task is queued, waiting for assignment
    Queued,
    /// Task is assigned to a worker
    Assigned {
        worker: AgentId,
        assigned_at: Instant,
    },
    /// Task completed successfully
    Completed { worker: AgentId, duration: Duration },
    /// Task failed
    Failed { worker: AgentId, error: String },
}

/// Task in the system
#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    pub message: Message,
    pub status: TaskStatus,
    pub retry_count: u32,
}

/// Worker in the pool
#[derive(Debug, Clone)]
pub struct Worker {
    pub id: AgentId,
    pub active_tasks: usize,
    pub completed_tasks: u64,
    pub failed_tasks: u64,
    pub last_heartbeat: Instant,
    pub available: bool,
}

impl Worker {
    fn new(id: AgentId) -> Self {
        Self {
            id,
            active_tasks: 0,
            completed_tasks: 0,
            failed_tasks: 0,
            last_heartbeat: Instant::now(),
            available: true,
        }
    }
}

/// Configuration for supervisor
#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    /// Maximum tasks per worker
    pub max_tasks_per_worker: usize,
    /// Worker heartbeat timeout
    pub heartbeat_timeout: Duration,
    /// Maximum task retries
    pub max_retries: u32,
    /// Task assignment timeout
    pub task_timeout: Duration,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            max_tasks_per_worker: 10,
            heartbeat_timeout: Duration::from_secs(30),
            max_retries: 3,
            task_timeout: Duration::from_secs(300),
        }
    }
}

/// Worker pool manager
pub struct WorkerPool {
    workers: Arc<RwLock<HashMap<AgentId, Worker>>>,
    config: SupervisorConfig,
}

impl WorkerPool {
    /// Create a new worker pool
    pub fn new(config: SupervisorConfig) -> Self {
        Self {
            workers: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Register a worker
    pub async fn register_worker(&self, worker_id: AgentId) {
        let mut workers = self.workers.write().await;
        workers.insert(worker_id.clone(), Worker::new(worker_id));
        debug!("Registered worker");
    }

    /// Remove a worker
    pub async fn remove_worker(&self, worker_id: &AgentId) {
        let mut workers = self.workers.write().await;
        workers.remove(worker_id);
        debug!("Removed worker");
    }

    /// Update worker heartbeat
    pub async fn heartbeat(&self, worker_id: &AgentId) {
        let mut workers = self.workers.write().await;
        if let Some(worker) = workers.get_mut(worker_id) {
            worker.last_heartbeat = Instant::now();
        }
    }

    /// Get available worker (load balancing)
    pub async fn get_available_worker(&self) -> Option<AgentId> {
        let workers = self.workers.read().await;

        workers
            .values()
            .filter(|w| w.available && w.active_tasks < self.config.max_tasks_per_worker)
            .min_by_key(|w| w.active_tasks)
            .map(|w| w.id.clone())
    }

    /// Mark worker as busy with task
    pub async fn assign_task(&self, worker_id: &AgentId) {
        let mut workers = self.workers.write().await;
        if let Some(worker) = workers.get_mut(worker_id) {
            worker.active_tasks += 1;
        }
    }

    /// Mark task completed on worker
    pub async fn complete_task(&self, worker_id: &AgentId, success: bool) {
        let mut workers = self.workers.write().await;
        if let Some(worker) = workers.get_mut(worker_id) {
            worker.active_tasks = worker.active_tasks.saturating_sub(1);
            if success {
                worker.completed_tasks += 1;
            } else {
                worker.failed_tasks += 1;
            }
        }
    }

    /// Check for unhealthy workers
    pub async fn check_health(&self) -> Vec<AgentId> {
        let mut workers = self.workers.write().await;
        let now = Instant::now();
        let timeout = self.config.heartbeat_timeout;

        let mut unhealthy = Vec::new();

        for (id, worker) in workers.iter_mut() {
            if now.duration_since(worker.last_heartbeat) > timeout {
                warn!("Worker {} missed heartbeat", id);
                worker.available = false;
                unhealthy.push(id.clone());
            }
        }

        unhealthy
    }

    /// Get worker statistics
    pub async fn stats(&self) -> HashMap<AgentId, (usize, u64, u64)> {
        let workers = self.workers.read().await;
        workers
            .iter()
            .map(|(id, w)| {
                (
                    id.clone(),
                    (w.active_tasks, w.completed_tasks, w.failed_tasks),
                )
            })
            .collect()
    }

    /// Get total worker count
    pub async fn worker_count(&self) -> usize {
        self.workers.read().await.len()
    }
}

/// Supervisor for task distribution
pub struct Supervisor<M: AgentMesh> {
    mesh: Arc<M>,
    config: SupervisorConfig,
    worker_pool: WorkerPool,
    task_queue: Arc<RwLock<VecDeque<Task>>>,
    active_tasks: Arc<RwLock<HashMap<String, Task>>>,
}

impl<M: AgentMesh> Supervisor<M> {
    /// Create a new supervisor
    pub fn new(mesh: Arc<M>, config: SupervisorConfig) -> Self {
        Self {
            mesh,
            config: config.clone(),
            worker_pool: WorkerPool::new(config),
            task_queue: Arc::new(RwLock::new(VecDeque::new())),
            active_tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create with default configuration
    pub fn with_defaults(mesh: Arc<M>) -> Self {
        Self::new(mesh, SupervisorConfig::default())
    }

    /// Submit a task
    pub async fn submit_task(&self, message: Message) -> String {
        let task_id = message.id.to_string();
        let task = Task {
            id: task_id.clone(),
            message,
            status: TaskStatus::Queued,
            retry_count: 0,
        };

        let mut queue = self.task_queue.write().await;
        queue.push_back(task);

        debug!("Submitted task {}", task_id);
        task_id
    }

    /// Assign tasks to available workers
    pub async fn assign_tasks(&self) -> MeshResult<usize> {
        let mut assigned = 0;

        loop {
            // Get next task
            let task = {
                let mut queue = self.task_queue.write().await;
                queue.pop_front()
            };

            let Some(mut task) = task else {
                break;
            };

            // Get available worker
            let Some(worker_id) = self.worker_pool.get_available_worker().await else {
                // No workers available, put task back
                let mut queue = self.task_queue.write().await;
                queue.push_front(task);
                break;
            };

            // Assign task
            task.status = TaskStatus::Assigned {
                worker: worker_id.clone(),
                assigned_at: Instant::now(),
            };

            self.worker_pool.assign_task(&worker_id).await;
            self.mesh.send(&worker_id, task.message.clone()).await?;

            let mut active = self.active_tasks.write().await;
            active.insert(task.id.clone(), task);

            assigned += 1;
            debug!("Assigned task to worker {}", worker_id);
        }

        Ok(assigned)
    }

    /// Mark task as completed
    pub async fn complete_task(&self, task_id: &str, worker_id: &AgentId, success: bool) {
        let mut active = self.active_tasks.write().await;

        if let Some(mut task) = active.remove(task_id) {
            task.status = if success {
                TaskStatus::Completed {
                    worker: worker_id.clone(),
                    duration: Duration::from_secs(0), // Would track actual duration
                }
            } else {
                TaskStatus::Failed {
                    worker: worker_id.clone(),
                    error: "Task failed".to_string(),
                }
            };

            self.worker_pool.complete_task(worker_id, success).await;

            // Retry failed tasks
            if !success && task.retry_count < self.config.max_retries {
                let retry_count = task.retry_count + 1;
                task.retry_count = retry_count;
                task.status = TaskStatus::Queued;
                let mut queue = self.task_queue.write().await;
                queue.push_back(task);
                debug!("Requeued failed task {} (retry {})", task_id, retry_count);
            }
        }
    }

    /// Register a worker
    pub async fn register_worker(&self, worker_id: AgentId) {
        self.worker_pool.register_worker(worker_id).await;
    }

    /// Worker heartbeat
    pub async fn worker_heartbeat(&self, worker_id: &AgentId) {
        self.worker_pool.heartbeat(worker_id).await;
    }

    /// Get queue size
    pub async fn queue_size(&self) -> usize {
        self.task_queue.read().await.len()
    }

    /// Get active task count
    pub async fn active_task_count(&self) -> usize {
        self.active_tasks.read().await.len()
    }

    /// Get worker statistics
    pub async fn worker_stats(&self) -> HashMap<AgentId, (usize, u64, u64)> {
        self.worker_pool.stats().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_worker_pool_registration() {
        let pool = WorkerPool::new(SupervisorConfig::default());

        let worker1 = AgentId::new_unchecked("worker-1");
        pool.register_worker(worker1.clone()).await;

        assert_eq!(pool.worker_count().await, 1);

        pool.remove_worker(&worker1).await;
        assert_eq!(pool.worker_count().await, 0);
    }

    #[tokio::test]
    async fn test_worker_load_balancing() {
        let pool = WorkerPool::new(SupervisorConfig::default());

        let worker1 = AgentId::new_unchecked("worker-1");
        let worker2 = AgentId::new_unchecked("worker-2");

        pool.register_worker(worker1.clone()).await;
        pool.register_worker(worker2.clone()).await;

        // First assignment should go to worker with 0 tasks
        let assigned = pool.get_available_worker().await;
        assert!(assigned.is_some());

        // Simulate task assignment
        pool.assign_task(&worker1).await;

        // Next assignment should prefer worker2 (0 tasks vs 1 task)
        let assigned = pool.get_available_worker().await;
        assert_eq!(assigned, Some(worker2));
    }

    #[tokio::test]
    async fn test_worker_heartbeat_timeout() {
        let config = SupervisorConfig {
            heartbeat_timeout: Duration::from_millis(100),
            ..Default::default()
        };
        let pool = WorkerPool::new(config);

        let worker1 = AgentId::new_unchecked("worker-1");
        pool.register_worker(worker1.clone()).await;

        // Wait for heartbeat timeout
        tokio::time::sleep(Duration::from_millis(150)).await;

        let unhealthy = pool.check_health().await;
        assert_eq!(unhealthy.len(), 1);
        assert_eq!(unhealthy[0], worker1);
    }
}
