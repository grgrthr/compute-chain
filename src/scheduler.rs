//! Distributed Scheduler — Manages job distribution across worker nodes.
//!
//! Handles job lifecycle: Submitted → Queued → Assigned → Running → Verified → Completed.
//! Tracks worker availability, reputation, and handles failure recovery.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ═══════════════════════════════════════════
// Job Types
// ═══════════════════════════════════════════

/// Unique identifier for a job.
pub type JobId = String;

/// Unique identifier for a worker.
pub type WorkerId = String;

/// The current status of a job in the lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    Submitted,
    Queued,
    Assigned,
    Running,
    ProofGenerated,
    Verified,
    Completed,
    Failed(String),
    Cancelled,
}

/// A compute job tracked by the scheduler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: JobId,
    pub status: JobStatus,
    pub assigned_worker: Option<WorkerId>,
    pub submitted_at: u64,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
    pub retry_count: u32,
    pub max_retries: u32,
    pub priority: u32,
    pub result_hash: Option<String>,
}

impl Job {
    pub fn new(id: &str, priority: u32) -> Self {
        Job {
            id: id.to_string(),
            status: JobStatus::Submitted,
            assigned_worker: None,
            submitted_at: now(),
            started_at: None,
            completed_at: None,
            retry_count: 0,
            max_retries: 3,
            priority,
            result_hash: None,
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            JobStatus::Completed | JobStatus::Failed(_) | JobStatus::Cancelled
        )
    }

    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }
}

// ═══════════════════════════════════════════
// Worker Types
// ═══════════════════════════════════════════

/// Worker status tracked by the scheduler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerStatus {
    Online,
    Busy,
    Offline,
    Degraded,
}

/// A worker registered in the scheduler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worker {
    pub id: WorkerId,
    pub status: WorkerStatus,
    pub active_jobs: Vec<JobId>,
    pub completed_jobs: u64,
    pub failed_jobs: u64,
    pub last_heartbeat: u64,
    pub reputation: f64,
    pub max_concurrent_jobs: usize,
}

impl Worker {
    pub fn new(id: &str, max_concurrent: usize) -> Self {
        Worker {
            id: id.to_string(),
            status: WorkerStatus::Online,
            active_jobs: vec![],
            completed_jobs: 0,
            failed_jobs: 0,
            last_heartbeat: now(),
            reputation: 1.0,
            max_concurrent_jobs: max_concurrent,
        }
    }

    pub fn is_available(&self) -> bool {
        self.status != WorkerStatus::Offline && self.active_jobs.len() < self.max_concurrent_jobs
    }

    pub fn is_timed_out(&self, timeout_secs: u64) -> bool {
        let elapsed = now().saturating_sub(self.last_heartbeat);
        elapsed > timeout_secs
    }

    pub fn success_rate(&self) -> f64 {
        let total = self.completed_jobs + self.failed_jobs;
        if total == 0 {
            return 1.0;
        }
        self.completed_jobs as f64 / total as f64
    }
}

// ═══════════════════════════════════════════
// Scheduler
// ═══════════════════════════════════════════

/// Configuration for the scheduler.
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    pub worker_timeout_secs: u64,
    pub max_retries: u32,
    pub heartbeat_interval_secs: u64,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        SchedulerConfig {
            worker_timeout_secs: 30,
            max_retries: 3,
            heartbeat_interval_secs: 10,
        }
    }
}

/// The distributed scheduler.
pub struct Scheduler {
    pub config: SchedulerConfig,
    queue: VecDeque<JobId>,
    jobs: HashMap<JobId, Job>,
    workers: BTreeMap<WorkerId, Worker>,
    completed_jobs: Vec<JobId>,
}

impl Scheduler {
    pub fn new(config: SchedulerConfig) -> Self {
        Scheduler {
            config,
            queue: VecDeque::new(),
            jobs: HashMap::new(),
            workers: BTreeMap::new(),
            completed_jobs: Vec::new(),
        }
    }

    // ═══ JOB QUEUE ═══

    /// Submit a job to the queue.
    pub fn enqueue(&mut self, job: Job) -> JobId {
        let id = job.id.clone();
        self.jobs.insert(id.clone(), job);
        self.queue.push_back(id.clone());
        id
    }

    /// Get the next job from the queue (FIFO by priority).
    pub fn dequeue(&mut self) -> Option<JobId> {
        // Sort by priority (highest first), then FIFO
        let mut entries: Vec<(u32, usize, JobId)> = self
            .queue
            .iter()
            .enumerate()
            .filter_map(|(idx, jid)| self.jobs.get(jid).map(|j| (j.priority, idx, jid.clone())))
            .collect();

        entries.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

        if let Some((_, _, job_id)) = entries.first() {
            // Remove from queue
            self.queue.retain(|jid| jid != job_id);
            Some(job_id.clone())
        } else {
            None
        }
    }

    /// Cancel a job.
    pub fn cancel(&mut self, job_id: &str) -> Result<(), String> {
        if let Some(job) = self.jobs.get_mut(job_id) {
            if job.is_terminal() {
                return Err(format!("Job {} is already terminal", job_id));
            }
            job.status = JobStatus::Cancelled;
            self.queue.retain(|jid| jid != job_id);
            // Release worker
            if let Some(wid) = job.assigned_worker.clone() {
                if let Some(worker) = self.workers.get_mut(&wid) {
                    worker.active_jobs.retain(|jid| jid != job_id);
                }
            }
            Ok(())
        } else {
            Err(format!("Job {} not found", job_id))
        }
    }

    /// Retry a failed job.
    pub fn retry(&mut self, job_id: &str) -> Result<(), String> {
        if let Some(job) = self.jobs.get_mut(job_id) {
            if !job.can_retry() {
                return Err(format!("Job {} exceeded max retries", job_id));
            }
            job.retry_count += 1;
            job.status = JobStatus::Queued;
            job.assigned_worker = None;
            job.started_at = None;
            self.queue.push_back(job_id.to_string());
            Ok(())
        } else {
            Err(format!("Job {} not found", job_id))
        }
    }

    // ═══ WORKER REGISTRY ═══

    /// Register a new worker.
    pub fn register_worker(&mut self, worker: Worker) {
        self.workers.insert(worker.id.clone(), worker);
    }

    /// Remove a worker.
    pub fn remove_worker(&mut self, worker_id: &str) {
        self.workers.remove(worker_id);
    }

    /// Update worker heartbeat.
    pub fn heartbeat(&mut self, worker_id: &str) -> Result<(), String> {
        if let Some(worker) = self.workers.get_mut(worker_id) {
            worker.last_heartbeat = now();
            if worker.status == WorkerStatus::Offline {
                worker.status = WorkerStatus::Online;
            }
            Ok(())
        } else {
            Err(format!("Worker {} not found", worker_id))
        }
    }

    /// Get a reference to a worker.
    pub fn get_worker(&self, worker_id: &str) -> Option<&Worker> {
        self.workers.get(worker_id)
    }

    /// Get all registered workers.
    pub fn get_workers(&self) -> Vec<&Worker> {
        self.workers.values().collect()
    }

    // ═══ SCHEDULING ═══

    /// Assign a job to the best available worker.
    /// Returns Some((job_id, worker_id)) if assignment succeeded.
    pub fn assign_job(&mut self) -> Option<(JobId, WorkerId)> {
        let job_id = self.dequeue()?;

        // Find best worker
        let worker_id = self.select_worker()?;

        // Update job
        if let Some(job) = self.jobs.get_mut(&job_id) {
            job.status = JobStatus::Assigned;
            job.assigned_worker = Some(worker_id.clone());
            job.started_at = Some(now());
        }

        // Update worker
        if let Some(worker) = self.workers.get_mut(&worker_id) {
            worker.active_jobs.push(job_id.clone());
            if worker.active_jobs.len() >= worker.max_concurrent_jobs {
                worker.status = WorkerStatus::Busy;
            }
        }

        Some((job_id, worker_id))
    }

    /// Select the best worker based on availability and reputation.
    fn select_worker(&self) -> Option<WorkerId> {
        let mut candidates: Vec<&Worker> =
            self.workers.values().filter(|w| w.is_available()).collect();

        if candidates.is_empty() {
            return None;
        }

        // Sort by: reputation (desc), then active_jobs (asc)
        candidates.sort_by(|a, b| {
            b.reputation
                .partial_cmp(&a.reputation)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.active_jobs.len().cmp(&b.active_jobs.len()))
        });

        Some(candidates[0].id.clone())
    }

    /// Mark a job as completed.
    pub fn complete_job(
        &mut self,
        job_id: &str,
        success: bool,
        result_hash: Option<&str>,
    ) -> Result<(), String> {
        if let Some(job) = self.jobs.get_mut(job_id) {
            job.status = if success {
                JobStatus::Completed
            } else {
                JobStatus::Failed("Execution failed".into())
            };
            job.completed_at = Some(now());
            if let Some(hash) = result_hash {
                job.result_hash = Some(hash.to_string());
            }

            // Release worker
            if let Some(wid) = job.assigned_worker.clone() {
                if let Some(worker) = self.workers.get_mut(&wid) {
                    worker.active_jobs.retain(|jid| jid != job_id);
                    if success {
                        worker.completed_jobs += 1;
                    } else {
                        worker.failed_jobs += 1;
                    }
                    worker.reputation = worker.success_rate();
                    if worker.status == WorkerStatus::Busy
                        && worker.active_jobs.len() < worker.max_concurrent_jobs
                    {
                        worker.status = WorkerStatus::Online;
                    }
                }
            }

            self.completed_jobs.push(job_id.to_string());
            Ok(())
        } else {
            Err(format!("Job {} not found", job_id))
        }
    }

    // ═══ FAILURE HANDLING ═══

    /// Detect and handle timed-out workers.
    /// Returns list of job IDs that need reassignment.
    pub fn detect_timeouts(&mut self) -> Vec<JobId> {
        let mut orphaned_jobs = Vec::new();
        let timeout = self.config.worker_timeout_secs;

        for worker in self.workers.values_mut() {
            if worker.is_timed_out(timeout) && worker.status != WorkerStatus::Offline {
                worker.status = WorkerStatus::Offline;
                // Collect active jobs for reassignment
                for job_id in &worker.active_jobs {
                    orphaned_jobs.push(job_id.clone());
                }
                worker.active_jobs.clear();
            }
        }

        // Reassign orphaned jobs
        for job_id in &orphaned_jobs {
            if let Some(job) = self.jobs.get_mut(job_id) {
                job.status = JobStatus::Queued;
                job.assigned_worker = None;
                self.queue.push_back(job_id.clone());
            }
        }

        orphaned_jobs
    }

    /// Prevent duplicate completion: check if job is already terminal.
    pub fn is_duplicate_completion(&self, job_id: &str) -> bool {
        self.jobs
            .get(job_id)
            .map(|j| j.is_terminal())
            .unwrap_or(true)
    }

    // ═══ STATUS ═══

    /// Get scheduler statistics.
    pub fn stats(&self) -> SchedulerStats {
        let queued = self.queue.len();
        let active = self.workers.values().map(|w| w.active_jobs.len()).sum();
        let completed = self.completed_jobs.len();
        let workers_online = self
            .workers
            .values()
            .filter(|w| w.status != WorkerStatus::Offline)
            .count();

        SchedulerStats {
            queued_jobs: queued,
            active_jobs: active,
            completed_jobs: completed,
            total_workers: self.workers.len(),
            workers_online,
        }
    }

    /// Get a reference to a job.
    pub fn get_job(&self, job_id: &str) -> Option<&Job> {
        self.jobs.get(job_id)
    }

    /// Get queue length.
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new(SchedulerConfig::default())
    }
}

// ═══════════════════════════════════════════
// Statistics
// ═══════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerStats {
    pub queued_jobs: usize,
    pub active_jobs: usize,
    pub completed_jobs: usize,
    pub total_workers: usize,
    pub workers_online: usize,
}

// ═══════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ═══════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn make_job(id: &str, priority: u32) -> Job {
        Job::new(id, priority)
    }

    #[test]
    fn test_enqueue_dequeue_order() {
        let mut s = Scheduler::default();
        s.enqueue(make_job("j1", 1));
        s.enqueue(make_job("j2", 10));
        s.enqueue(make_job("j3", 5));

        // Priority order: j2(10) → j3(5) → j1(1)
        assert_eq!(s.dequeue().unwrap(), "j2");
        assert_eq!(s.dequeue().unwrap(), "j3");
        assert_eq!(s.dequeue().unwrap(), "j1");
        assert!(s.dequeue().is_none());
    }

    #[test]
    fn test_cancel_job() {
        let mut s = Scheduler::default();
        s.enqueue(make_job("j1", 1));
        s.cancel("j1").unwrap();
        assert!(s.get_job("j1").unwrap().is_terminal());
        assert!(s.dequeue().is_none());
    }

    #[test]
    fn test_retry_job() {
        let mut s = Scheduler::default();
        let mut job = make_job("j1", 1);
        job.status = JobStatus::Failed("err".into());
        s.jobs.insert("j1".into(), job);

        s.retry("j1").unwrap();
        let job = s.get_job("j1").unwrap();
        assert_eq!(job.retry_count, 1);
        assert_eq!(job.status, JobStatus::Queued);
    }

    #[test]
    fn test_retry_exceeds_max() {
        let mut s = Scheduler::default();
        let mut job = make_job("j1", 1);
        job.retry_count = 3;
        job.status = JobStatus::Failed("err".into());
        s.jobs.insert("j1".into(), job);

        assert!(s.retry("j1").is_err());
    }

    #[test]
    fn test_worker_selection() {
        let mut s = Scheduler::default();
        s.register_worker(Worker::new("w1", 2));
        s.register_worker(Worker::new("w2", 2));
        s.enqueue(make_job("j1", 1));

        let (jid, wid) = s.assign_job().unwrap();
        assert_eq!(jid, "j1");
        assert!(!wid.is_empty());
    }

    #[test]
    fn test_worker_selection_by_reputation() {
        let mut s = Scheduler::default();
        let w1 = Worker::new("w1", 2);
        let mut w2 = Worker::new("w2", 2);
        w2.reputation = 0.5;
        w2.completed_jobs = 1;
        w2.failed_jobs = 1;
        s.register_worker(w1);
        s.register_worker(w2);
        s.enqueue(make_job("j1", 1));

        let (_, wid) = s.assign_job().unwrap();
        assert_eq!(wid, "w1", "Should pick higher reputation worker");
    }

    #[test]
    fn test_no_available_worker() {
        let mut s = Scheduler::default();
        let mut w = Worker::new("w1", 1);
        w.status = WorkerStatus::Busy;
        w.active_jobs = vec!["existing".into()];
        s.register_worker(w);
        s.enqueue(make_job("j1", 1));

        assert!(s.assign_job().is_none());
    }

    #[test]
    fn test_timeout_detection() {
        let mut s = Scheduler::new(SchedulerConfig {
            worker_timeout_secs: 0, // Immediate timeout
            ..Default::default()
        });
        let mut w = Worker::new("w1", 2);
        w.last_heartbeat = 0; // Very old heartbeat
        w.active_jobs = vec!["j1".into()];
        s.register_worker(w);
        s.jobs.insert("j1".into(), make_job("j1", 1));

        let orphaned = s.detect_timeouts();
        assert_eq!(orphaned.len(), 1);
        assert_eq!(orphaned[0], "j1");

        let worker = s.get_worker("w1").unwrap();
        assert_eq!(worker.status, WorkerStatus::Offline);
    }

    #[test]
    fn test_duplicate_completion_prevention() {
        let mut s = Scheduler::default();
        let mut job = make_job("j1", 1);
        job.status = JobStatus::Completed;
        s.jobs.insert("j1".into(), job);

        assert!(s.is_duplicate_completion("j1"));
        assert!(s.complete_job("j1", true, None).is_ok()); // Still succeeds but no double-count
    }

    #[test]
    fn test_complete_job_updates_worker() {
        let mut s = Scheduler::default();
        let mut w = Worker::new("w1", 2);
        w.active_jobs = vec!["j1".into()];
        s.register_worker(w);

        let mut job = make_job("j1", 1);
        job.assigned_worker = Some("w1".into());
        s.jobs.insert("j1".into(), job);

        s.complete_job("j1", true, Some("hash123")).unwrap();

        let worker = s.get_worker("w1").unwrap();
        assert_eq!(worker.completed_jobs, 1);
        assert!(worker.active_jobs.is_empty());

        let job = s.get_job("j1").unwrap();
        assert_eq!(job.status, JobStatus::Completed);
        assert_eq!(job.result_hash.as_deref(), Some("hash123"));
    }

    #[test]
    fn test_deterministic_scheduling() {
        let mut s1 = Scheduler::default();
        let mut s2 = Scheduler::default();

        for i in 1..=5 {
            s1.register_worker(Worker::new(&format!("w{}", i), 2));
            s2.register_worker(Worker::new(&format!("w{}", i), 2));
        }
        for i in 1..=5 {
            s1.enqueue(make_job(&format!("j{}", i), (6 - i) as u32));
            s2.enqueue(make_job(&format!("j{}", i), (6 - i) as u32));
        }

        let mut results1 = vec![];
        let mut results2 = vec![];
        while let Some((jid, wid)) = s1.assign_job() {
            results1.push((jid, wid));
        }
        while let Some((jid, wid)) = s2.assign_job() {
            results2.push((jid, wid));
        }

        assert_eq!(
            results1, results2,
            "Identical schedulers must produce identical assignments"
        );
    }

    #[test]
    fn test_stats() {
        let mut s = Scheduler::default();
        s.register_worker(Worker::new("w1", 2));
        s.enqueue(make_job("j1", 1));
        s.enqueue(make_job("j2", 2));

        let stats = s.stats();
        assert_eq!(stats.queued_jobs, 2);
        assert_eq!(stats.total_workers, 1);
        assert_eq!(stats.workers_online, 1);
    }
}
