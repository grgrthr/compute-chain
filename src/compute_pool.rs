//! Distributed Compute Pool

use crate::p2p::proof_network::{ComputeJob, ComputeJobResult, WorkerNode};
use crate::scheduler::{Job, Scheduler, SchedulerConfig, Worker};
use crate::stark::proof_manager::ProofManager;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub type PoolJobId = String;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PoolJobStatus {
    Pending,
    Assigned,
    Running,
    Completed,
    Failed(String),
    Expired,
    Rejected(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PoolJob {
    pub id: PoolJobId,
    pub compute_job: ComputeJob,
    pub status: PoolJobStatus,
    pub assigned_worker: Option<String>,
    pub submitted_at: u64,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
    pub result_hash: Option<String>,
    pub retry_count: u32,
    pub max_retries: u32,
}

impl PoolJob {
    pub fn new(compute_job: ComputeJob) -> Self {
        PoolJob {
            id: compute_job.job_id.clone(),
            compute_job,
            status: PoolJobStatus::Pending,
            assigned_worker: None,
            submitted_at: now(),
            started_at: None,
            completed_at: None,
            result_hash: None,
            retry_count: 0,
            max_retries: 3,
        }
    }
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            PoolJobStatus::Completed
                | PoolJobStatus::Failed(_)
                | PoolJobStatus::Expired
                | PoolJobStatus::Rejected(_)
        )
    }
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries && !matches!(self.status, PoolJobStatus::Completed)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStats {
    pub pending_jobs: usize,
    pub running_jobs: usize,
    pub completed_jobs: usize,
    pub failed_jobs: usize,
    pub expired_jobs: usize,
    pub total_workers: usize,
    pub available_workers: usize,
    pub proofs_verified: usize,
    pub proofs_rejected: usize,
}

#[derive(Debug, Clone)]
pub struct PoolConfig {
    pub job_timeout_secs: u64,
    pub max_retries: u32,
    pub max_workers: usize,
}

impl Default for PoolConfig {
    fn default() -> Self {
        PoolConfig {
            job_timeout_secs: 120,
            max_retries: 3,
            max_workers: 100,
        }
    }
}

pub struct ComputePool {
    config: PoolConfig,
    scheduler: Scheduler,
    proof_manager: ProofManager,
    jobs: Arc<Mutex<HashMap<PoolJobId, PoolJob>>>,
    results: Arc<Mutex<HashMap<PoolJobId, ComputeJobResult>>>,
    verified_jobs: Arc<Mutex<Vec<PoolJobId>>>,
    rejected_jobs: Arc<Mutex<Vec<PoolJobId>>>,
}

impl ComputePool {
    pub fn new(config: PoolConfig) -> Self {
        ComputePool {
            config,
            scheduler: Scheduler::new(SchedulerConfig::default()),
            proof_manager: ProofManager::new(),
            jobs: Arc::new(Mutex::new(HashMap::new())),
            results: Arc::new(Mutex::new(HashMap::new())),
            verified_jobs: Arc::new(Mutex::new(Vec::new())),
            rejected_jobs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn submit_job(&mut self, compute_job: ComputeJob) -> PoolJobId {
        let pool_job = PoolJob::new(compute_job);
        let id = pool_job.id.clone();
        self.jobs.lock().unwrap().insert(id.clone(), pool_job);
        self.scheduler.enqueue(Job::new(&id, 1));
        id
    }

    pub fn submit_batch(&mut self, jobs: Vec<ComputeJob>) -> Vec<PoolJobId> {
        jobs.into_iter().map(|job| self.submit_job(job)).collect()
    }

    pub fn register_worker(&mut self, worker_id: &str, max_concurrent: usize) {
        self.scheduler
            .register_worker(Worker::new(worker_id, max_concurrent));
    }

    pub fn remove_worker(&mut self, worker_id: &str) {
        self.scheduler.remove_worker(worker_id);
    }

    pub fn worker_heartbeat(&mut self, worker_id: &str) -> Result<(), String> {
        self.scheduler.heartbeat(worker_id)
    }

    pub fn assign_next_job(&mut self) -> Option<(PoolJobId, String)> {
        let (job_id, worker_id) = self.scheduler.assign_job()?;
        if let Some(job) = self.jobs.lock().unwrap().get_mut(&job_id) {
            job.status = PoolJobStatus::Assigned;
            job.assigned_worker = Some(worker_id.clone());
            job.started_at = Some(now());
        }
        Some((job_id, worker_id))
    }

    pub fn submit_result(
        &mut self,
        job_id: &str,
        result: ComputeJobResult,
    ) -> Result<bool, String> {
        let mut jobs = self.jobs.lock().unwrap();
        let job = jobs
            .get_mut(job_id)
            .ok_or_else(|| format!("Job {} not found", job_id))?;
        if job.is_terminal() {
            return Err(format!("Job {} already terminal", job_id));
        }
        let verified = result.proof_verified && result.proof.is_some();
        if verified {
            job.status = PoolJobStatus::Completed;
            job.completed_at = Some(now());
            job.result_hash = Some(result.trace_root.clone());
            self.verified_jobs.lock().unwrap().push(job_id.to_string());
            self.results
                .lock()
                .unwrap()
                .insert(job_id.to_string(), result);
            self.scheduler
                .complete_job(
                    job_id,
                    true,
                    Some(&job.result_hash.clone().unwrap_or_default()),
                )
                .ok();
        } else {
            job.status = PoolJobStatus::Rejected("Proof verification failed".into());
            self.rejected_jobs.lock().unwrap().push(job_id.to_string());
        }
        Ok(verified)
    }

    pub fn detect_timeouts(&mut self) -> Vec<PoolJobId> {
        let timeout = self.config.job_timeout_secs;
        let now_ts = now();
        let mut reassigned = Vec::new();
        let mut jobs = self.jobs.lock().unwrap();
        for (id, job) in jobs.iter_mut() {
            if matches!(job.status, PoolJobStatus::Assigned | PoolJobStatus::Running) {
                if let Some(started) = job.started_at {
                    if now_ts.saturating_sub(started) > timeout {
                        if job.can_retry() {
                            job.status = PoolJobStatus::Pending;
                            job.retry_count += 1;
                            job.assigned_worker = None;
                            job.started_at = None;
                            reassigned.push(id.clone());
                        } else {
                            job.status = PoolJobStatus::Expired;
                            job.completed_at = Some(now_ts);
                        }
                    }
                }
            }
        }
        for job_id in &reassigned {
            self.scheduler.enqueue(Job::new(job_id, 1));
        }
        reassigned
    }

    pub fn is_duplicate(&self, job_id: &str) -> bool {
        self.jobs
            .lock()
            .unwrap()
            .get(job_id)
            .map(|j| matches!(j.status, PoolJobStatus::Completed))
            .unwrap_or(false)
    }

    pub fn stats(&self) -> PoolStats {
        let jobs = self.jobs.lock().unwrap();
        let pending = jobs
            .values()
            .filter(|j| j.status == PoolJobStatus::Pending)
            .count();
        let running = jobs
            .values()
            .filter(|j| matches!(j.status, PoolJobStatus::Assigned | PoolJobStatus::Running))
            .count();
        let completed = jobs
            .values()
            .filter(|j| j.status == PoolJobStatus::Completed)
            .count();
        let failed = jobs
            .values()
            .filter(|j| matches!(j.status, PoolJobStatus::Failed(_)))
            .count();
        let expired = jobs
            .values()
            .filter(|j| j.status == PoolJobStatus::Expired)
            .count();
        let workers = self.scheduler.get_workers();
        PoolStats {
            pending_jobs: pending,
            running_jobs: running,
            completed_jobs: completed,
            failed_jobs: failed,
            expired_jobs: expired,
            total_workers: workers.len(),
            available_workers: workers.iter().filter(|w| w.is_available()).count(),
            proofs_verified: self.verified_jobs.lock().unwrap().len(),
            proofs_rejected: self.rejected_jobs.lock().unwrap().len(),
        }
    }

    pub fn get_job(&self, job_id: &str) -> Option<PoolJob> {
        self.jobs.lock().unwrap().get(job_id).cloned()
    }
    pub fn get_result(&self, job_id: &str) -> Option<ComputeJobResult> {
        self.results.lock().unwrap().get(job_id).cloned()
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p::proof_network::{sample_compute_job, WorkerNode};

    #[test]
    fn test_submit_and_assign() {
        let mut p = ComputePool::new(PoolConfig::default());
        p.register_worker("w1", 5);
        let id = p.submit_job(sample_compute_job());
        let a = p.assign_next_job().unwrap();
        assert_eq!(a.0, id);
    }
    #[test]
    fn test_multiple_jobs() {
        let mut p = ComputePool::new(PoolConfig::default());
        p.register_worker("w1", 5);
        p.register_worker("w2", 5);
        p.submit_batch(vec![sample_compute_job(), sample_compute_job()]);
        let a1 = p.assign_next_job().unwrap();
        let a2 = p.assign_next_job().unwrap();
        assert_ne!(a1.0, a2.0);
    }
    #[test]
    fn test_submit_result() {
        let mut p = ComputePool::new(PoolConfig::default());
        p.register_worker("w1", 5);
        let j = sample_compute_job();
        let id = p.submit_job(j.clone());
        p.assign_next_job();
        let w = WorkerNode::new("w1");
        assert!(p.submit_result(&id, w.execute_job(&j)).unwrap());
    }
    #[test]
    fn test_duplicate() {
        let mut p = ComputePool::new(PoolConfig::default());
        p.register_worker("w1", 5);
        let j = sample_compute_job();
        let id = p.submit_job(j.clone());
        p.assign_next_job();
        p.submit_result(&id, WorkerNode::new("w1").execute_job(&j))
            .unwrap();
        assert!(p.is_duplicate(&id));
    }
    #[test]
    fn test_invalid() {
        let mut p = ComputePool::new(PoolConfig::default());
        p.register_worker("w1", 5);
        let j = sample_compute_job();
        let id = p.submit_job(j);
        p.assign_next_job();
        let bad = ComputeJobResult {
            job_id: id.clone(),
            success: false,
            trace_root: String::new(),
            execution_steps: 0,
            execution_time_ms: 0,
            final_registers: vec![],
            proof: None,
            proof_verified: false,
            error: Some("x".into()),
        };
        assert!(!p.submit_result(&id, bad).unwrap());
    }
    #[test]
    fn test_timeout() {
        let mut p = ComputePool::new(PoolConfig {
            job_timeout_secs: 0,
            ..Default::default()
        });
        p.register_worker("w1", 5);
        let id = p.submit_job(sample_compute_job());
        p.assign_next_job();
        p.jobs.lock().unwrap().get_mut(&id).unwrap().started_at = Some(0);
        assert_eq!(p.detect_timeouts().len(), 1);
    }
    #[test]
    fn test_stats() {
        let mut p = ComputePool::new(PoolConfig::default());
        p.register_worker("w1", 5);
        p.submit_batch(vec![sample_compute_job(), sample_compute_job()]);
        let s = p.stats();
        assert_eq!(s.pending_jobs, 2);
    }
    #[test]
    fn test_deterministic() {
        let run = || {
            let mut p = ComputePool::new(PoolConfig::default());
            p.register_worker("w1", 10);
            p.submit_batch(vec![sample_compute_job()]);
            let mut w = String::new();
            while let Some((_, wid)) = p.assign_next_job() {
                w = wid;
            }
            w
        };
        assert_eq!(run(), "w1");
    }
    #[test]
    fn test_e2e() {
        let mut p = ComputePool::new(PoolConfig::default());
        p.register_worker("w1", 10);
        let jobs: Vec<ComputeJob> = (0..4).map(|_| sample_compute_job()).collect();
        p.submit_batch(jobs.clone());
        let w = WorkerNode::new("w1");
        let mut d = 0;
        while let Some((jid, _)) = p.assign_next_job() {
            let j = jobs.iter().find(|j| j.job_id == jid).unwrap();
            if p.submit_result(&jid, w.execute_job(j)).unwrap() {
                d += 1;
            }
        }
        assert_eq!(d, 4);
    }
}
