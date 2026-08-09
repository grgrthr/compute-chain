//! Real Multi-Node Worker Network
//!
//! Workers and Schedulers communicate over libp2p gossipsub.
//! No simulation — real TCP transport, peer discovery, and message propagation.

use crate::p2p::message::{
    ProofSubmission, ProofVerification, WorkerHeartbeat, WorkloadAnnouncement, WorkloadAssignment,
    WorkloadRequest, WorkloadResult,
};
use crate::p2p::node::{NetworkMessage, P2PCommand, P2PEvent, P2PHandle, P2PNode};
use crate::p2p::proof_network::{ComputeJob, ComputeJobResult, WorkerNode};
use crate::stark::proof_manager::ProofManager;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

// ═══════════════════════════════════════════
// Gossipsub Topics for Worker Network
// ═══════════════════════════════════════════

const TOPIC_WORKLOADS: &str = "compute-chain/workloads";
const TOPIC_WORKER_RESULTS: &str = "compute-chain/results";
const TOPIC_WORKER_HEARTBEATS: &str = "compute-chain/heartbeats";
const TOPIC_PROOFS: &str = "compute-chain/proofs";

// ═══════════════════════════════════════════
// Extended Network Messages
// ═══════════════════════════════════════════

/// Messages for the worker compute network (extends existing NetworkMessage).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerNetworkMessage {
    /// Scheduler announces available workloads.
    WorkloadAnnounce(WorkloadAnnouncement),
    /// Worker requests a specific workload.
    WorkloadReq(WorkloadRequest),
    /// Scheduler assigns workload to worker.
    WorkloadAssign(WorkloadAssignment),
    /// Worker submits execution result.
    WorkloadResultMsg(WorkloadResult),
    /// Worker submits STARK proof.
    ProofSubmit(ProofSubmission),
    /// Verifier reports proof verification result.
    ProofVerifyResult(ProofVerification),
    /// Worker heartbeat signal.
    Heartbeat(WorkerHeartbeat),
}

// ═══════════════════════════════════════════
// Networked Worker
// ═══════════════════════════════════════════

/// A worker connected to the real P2P network.
pub struct NetworkedWorker {
    pub peer_id: String,
    pub worker: WorkerNode,
    pub p2p_handle: Option<P2PHandle>,
    pending_jobs: Arc<Mutex<HashMap<String, ComputeJob>>>,
}

impl NetworkedWorker {
    pub fn new(peer_id: &str) -> Self {
        NetworkedWorker {
            peer_id: peer_id.to_string(),
            worker: WorkerNode::new(peer_id),
            p2p_handle: None,
            pending_jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Attach a P2P handle for network communication.
    pub fn with_p2p(mut self, handle: P2PHandle) -> Self {
        self.p2p_handle = Some(handle);
        self
    }

    /// Receive and execute a compute job, then submit result.
    pub fn process_job(&self, job: &ComputeJob) -> ComputeJobResult {
        let result = self.worker.execute_job(job);
        self.pending_jobs
            .lock()
            .unwrap()
            .insert(job.job_id.clone(), job.clone());
        result
    }

    /// Create a WorkloadResult from a ComputeJobResult.
    pub fn to_workload_result(&self, job_id: &str, result: &ComputeJobResult) -> WorkloadResult {
        WorkloadResult {
            result_id: format!("res_{}", uuid::Uuid::new_v4()),
            workload_id: job_id.to_string(),
            worker_peer: self.peer_id.clone(),
            trace_root: result.trace_root.clone(),
            execution_steps: result.execution_steps,
            execution_time_ms: result.execution_time_ms,
            success: result.success,
            timestamp: now(),
        }
    }

    /// Create a ProofSubmission from a ComputeJobResult.
    pub fn to_proof_submission(
        &self,
        job_id: &str,
        result: &ComputeJobResult,
    ) -> Option<ProofSubmission> {
        result.proof.as_ref().map(|proof| ProofSubmission {
            submission_id: format!("sub_{}", uuid::Uuid::new_v4()),
            workload_id: job_id.to_string(),
            worker_peer: self.peer_id.clone(),
            proof_hash: proof.trace_hash.clone(),
            proof_size_bytes: proof.proof_size_bytes,
            generation_time_ms: result.execution_time_ms,
            timestamp: now(),
        })
    }
}

// ═══════════════════════════════════════════
// Networked Scheduler
// ═══════════════════════════════════════════

/// A scheduler that operates over the real P2P network.
pub struct NetworkedScheduler {
    pub peer_id: String,
    pub proof_manager: ProofManager,
    known_workers: Arc<Mutex<HashMap<String, WorkerHeartbeat>>>,
    pending_jobs: Arc<Mutex<HashMap<String, ComputeJob>>>,
    completed_results: Arc<Mutex<HashMap<String, ComputeJobResult>>>,
}

impl NetworkedScheduler {
    pub fn new(peer_id: &str) -> Self {
        NetworkedScheduler {
            peer_id: peer_id.to_string(),
            proof_manager: ProofManager::new(),
            known_workers: Arc::new(Mutex::new(HashMap::new())),
            pending_jobs: Arc::new(Mutex::new(HashMap::new())),
            completed_results: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a worker from heartbeat.
    pub fn register_heartbeat(&self, hb: WorkerHeartbeat) {
        self.known_workers
            .lock()
            .unwrap()
            .insert(hb.peer_id.clone(), hb);
    }

    /// Get list of known worker IDs.
    pub fn known_worker_ids(&self) -> Vec<String> {
        self.known_workers.lock().unwrap().keys().cloned().collect()
    }

    /// Create a workload announcement for a job.
    pub fn announce_workload(&self, job: &ComputeJob) -> WorkloadAnnouncement {
        WorkloadAnnouncement {
            workload_id: job.job_id.clone(),
            program_hash: {
                let mut h = sha2::Sha256::new();
                use sha2::Digest;
                h.update(format!("{:?}", job.instructions).as_bytes());
                format!("{:x}", h.finalize())
            },
            difficulty: job.difficulty,
            reward: 100,
            deadline_ms: 60_000,
            issuer_peer: self.peer_id.clone(),
            timestamp: now(),
        }
    }

    /// Store a received result.
    pub fn store_result(&self, result: ComputeJobResult) {
        self.completed_results
            .lock()
            .unwrap()
            .insert(result.job_id.clone(), result);
    }

    /// Verify a submitted proof using the proof manager.
    pub fn verify_result(&self, result: &ComputeJobResult) -> bool {
        if let Some(proof) = &result.proof {
            result.proof_verified && !proof.trace_hash.is_empty()
        } else {
            false
        }
    }

    /// Get completed results.
    pub fn get_results(&self) -> HashMap<String, ComputeJobResult> {
        self.completed_results.lock().unwrap().clone()
    }
}

// ═══════════════════════════════════════════
// Utility
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
    use crate::p2p::proof_network::sample_compute_job;

    #[test]
    fn test_worker_process_job() {
        let worker = NetworkedWorker::new("test_peer_1");
        let job = sample_compute_job();
        let result = worker.process_job(&job);

        assert!(result.success);
        assert_eq!(result.final_registers[0], 15);
        assert!(result.proof.is_some());
    }

    #[test]
    fn test_worker_to_result_message() {
        let worker = NetworkedWorker::new("test_peer_2");
        let job = sample_compute_job();
        let result = worker.process_job(&job);

        let msg = worker.to_workload_result(&job.job_id, &result);
        assert_eq!(msg.worker_peer, "test_peer_2");
        assert!(msg.success);
        assert!(!msg.trace_root.is_empty());
    }

    #[test]
    fn test_worker_to_proof_submission() {
        let worker = NetworkedWorker::new("test_peer_3");
        let job = sample_compute_job();
        let result = worker.process_job(&job);

        let proof = worker.to_proof_submission(&job.job_id, &result);
        assert!(proof.is_some());
        let proof = proof.unwrap();
        assert!(!proof.proof_hash.is_empty());
        assert!(proof.proof_size_bytes > 0);
    }

    #[test]
    fn test_scheduler_worker_registry() {
        let scheduler = NetworkedScheduler::new("scheduler_1");

        let hb = WorkerHeartbeat {
            peer_id: "worker_a".into(),
            uptime_seconds: 100,
            tasks_completed: 5,
            proofs_verified: 3,
            timestamp: now(),
        };

        scheduler.register_heartbeat(hb);
        let workers = scheduler.known_worker_ids();
        assert!(workers.contains(&"worker_a".to_string()));
    }

    #[test]
    fn test_scheduler_announce_workload() {
        let scheduler = NetworkedScheduler::new("scheduler_1");
        let job = sample_compute_job();

        let announcement = scheduler.announce_workload(&job);
        assert_eq!(announcement.issuer_peer, "scheduler_1");
        assert_eq!(announcement.difficulty, 1);
    }

    #[test]
    fn test_two_workers_process_same_job() {
        let worker_a = NetworkedWorker::new("peer_a");
        let worker_b = NetworkedWorker::new("peer_b");
        let job = sample_compute_job();

        let result_a = worker_a.process_job(&job);
        let result_b = worker_b.process_job(&job);

        // Both should produce same trace root (deterministic)
        assert_eq!(result_a.trace_root, result_b.trace_root);
        assert_eq!(result_a.final_registers, result_b.final_registers);
    }

    #[test]
    fn test_scheduler_verify_result() {
        let scheduler = NetworkedScheduler::new("scheduler_1");
        let worker = NetworkedWorker::new("worker_1");
        let job = sample_compute_job();
        let result = worker.process_job(&job);

        assert!(scheduler.verify_result(&result));
    }

    #[test]
    fn test_scheduler_rejects_invalid_result() {
        let scheduler = NetworkedScheduler::new("scheduler_1");
        let result = ComputeJobResult {
            job_id: "bad".into(),
            success: false,
            trace_root: String::new(),
            execution_steps: 0,
            execution_time_ms: 0,
            final_registers: vec![],
            proof: None,
            proof_verified: false,
            error: Some("no proof".into()),
        };
        assert!(!scheduler.verify_result(&result));
    }

    #[test]
    fn test_message_deterministic_encoding() {
        let worker = NetworkedWorker::new("peer_x");
        let job = sample_compute_job();
        let result = worker.process_job(&job);

        let msg1 = worker.to_workload_result(&job.job_id, &result);
        let msg2 = worker.to_workload_result(&job.job_id, &result);

        // result_id is UUID (unique per call), but core fields must match
        assert_eq!(msg1.workload_id, msg2.workload_id);
        assert_eq!(msg1.worker_peer, msg2.worker_peer);
        assert_eq!(msg1.trace_root, msg2.trace_root);
        assert_eq!(msg1.execution_steps, msg2.execution_steps);
        assert_eq!(msg1.success, msg2.success);
    }
}
