use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════
// Worker Network Messages — Phase 6
// ═══════════════════════════════════════════

/// Announcement of available compute workload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadAnnouncement {
    pub workload_id: String,
    pub program_hash: String,
    pub difficulty: u32,
    pub reward: u64,
    pub deadline_ms: u64,
    pub issuer_peer: String,
    pub timestamp: u64,
}

/// Request to receive a specific workload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadRequest {
    pub request_id: String,
    pub workload_id: String,
    pub requester_peer: String,
    pub timestamp: u64,
}

/// Assignment of a workload to a worker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadAssignment {
    pub assignment_id: String,
    pub workload_id: String,
    pub program: Vec<crate::p2p::proof_network::InstructionData>,
    pub assigned_peer: String,
    pub issuer_peer: String,
    pub timestamp: u64,
}

/// Result of workload execution by a worker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadResult {
    pub result_id: String,
    pub workload_id: String,
    pub worker_peer: String,
    pub trace_root: String,
    pub execution_steps: usize,
    pub execution_time_ms: u64,
    pub success: bool,
    pub timestamp: u64,
}

/// Submission of a STARK proof by a worker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProofSubmission {
    pub submission_id: String,
    pub workload_id: String,
    pub worker_peer: String,
    pub proof_hash: Vec<u8>,
    pub proof_size_bytes: usize,
    pub generation_time_ms: u64,
    pub timestamp: u64,
}

/// Verification result of a submitted proof.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProofVerification {
    pub verification_id: String,
    pub submission_id: String,
    pub workload_id: String,
    pub verifier_peer: String,
    pub verified: bool,
    pub verification_time_ms: u64,
    pub timestamp: u64,
}

/// Heartbeat signal between worker nodes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkerHeartbeat {
    pub peer_id: String,
    pub uptime_seconds: u64,
    pub tasks_completed: u64,
    pub proofs_verified: u64,
    pub timestamp: u64,
}

/// Peer discovery announcement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PeerAnnouncement {
    pub peer_id: String,
    pub addresses: Vec<String>,
    pub capabilities: Vec<String>,
    pub timestamp: u64,
}

// ═══════════════════════════════════════════
// Legacy Message Types (preserved)
// ═══════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeRequest {
    pub workload_id: String,
    pub instructions: Vec<String>,
    pub difficulty: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeResponse {
    pub workload_id: String,
    pub result: Vec<u64>,
    pub proof_valid: bool,
    pub execution_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionMessage {
    pub tx_id: String,
    pub from: String,
    pub to: String,
    pub amount: u64,
    pub fee: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockMessage {
    pub height: u64,
    pub hash: String,
    pub previous_hash: String,
    pub validator_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfoMessage {
    pub peer_id: String,
    pub address: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> u64 {
        1700000000
    }

    #[test]
    fn test_workload_announcement_serialization() {
        let msg = WorkloadAnnouncement {
            workload_id: "wl_001".into(),
            program_hash: "abc123".into(),
            difficulty: 5,
            reward: 1000,
            deadline_ms: 60000,
            issuer_peer: "peer_a".into(),
            timestamp: now(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: WorkloadAnnouncement = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.workload_id, "wl_001");
        assert_eq!(decoded.difficulty, 5);
    }

    #[test]
    fn test_proof_submission_serialization() {
        let msg = ProofSubmission {
            submission_id: "sub_001".into(),
            workload_id: "wl_001".into(),
            worker_peer: "peer_b".into(),
            proof_hash: vec![1, 2, 3, 4],
            proof_size_bytes: 1024,
            generation_time_ms: 500,
            timestamp: now(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: ProofSubmission = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.proof_hash, vec![1, 2, 3, 4]);
        assert_eq!(decoded.proof_size_bytes, 1024);
    }

    #[test]
    fn test_worker_heartbeat_serialization() {
        let msg = WorkerHeartbeat {
            peer_id: "peer_c".into(),
            uptime_seconds: 3600,
            tasks_completed: 42,
            proofs_verified: 10,
            timestamp: now(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: WorkerHeartbeat = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.tasks_completed, 42);
    }

    #[test]
    fn test_deterministic_encoding() {
        let msg1 = WorkloadResult {
            result_id: "r1".into(),
            workload_id: "w1".into(),
            worker_peer: "p1".into(),
            trace_root: "root".into(),
            execution_steps: 100,
            execution_time_ms: 200,
            success: true,
            timestamp: now(),
        };
        let json1 = serde_json::to_string(&msg1).unwrap();
        let json2 = serde_json::to_string(&msg1).unwrap();
        assert_eq!(json1, json2, "Same message must encode identically");
    }

    #[test]
    fn test_all_message_types() {
        // Verify all new types serialize/deserialize
        let types: Vec<String> = vec![
            serde_json::to_string(&WorkloadAnnouncement {
                workload_id: "w".into(),
                program_hash: "h".into(),
                difficulty: 1,
                reward: 10,
                deadline_ms: 1000,
                issuer_peer: "p".into(),
                timestamp: now(),
            })
            .unwrap(),
            serde_json::to_string(&WorkloadRequest {
                request_id: "r".into(),
                workload_id: "w".into(),
                requester_peer: "p".into(),
                timestamp: now(),
            })
            .unwrap(),
            serde_json::to_string(&WorkloadAssignment {
                assignment_id: "a".into(),
                workload_id: "w".into(),
                program: vec![crate::p2p::proof_network::InstructionData { opcode: "MOV".into(), params: vec![0, 42] }],
                assigned_peer: "p".into(),
                issuer_peer: "i".into(),
                timestamp: now(),
            })
            .unwrap(),
            serde_json::to_string(&WorkloadResult {
                result_id: "r".into(),
                workload_id: "w".into(),
                worker_peer: "p".into(),
                trace_root: "t".into(),
                execution_steps: 1,
                execution_time_ms: 1,
                success: true,
                timestamp: now(),
            })
            .unwrap(),
            serde_json::to_string(&ProofSubmission {
                submission_id: "s".into(),
                workload_id: "w".into(),
                worker_peer: "p".into(),
                proof_hash: vec![],
                proof_size_bytes: 0,
                generation_time_ms: 0,
                timestamp: now(),
            })
            .unwrap(),
            serde_json::to_string(&ProofVerification {
                verification_id: "v".into(),
                submission_id: "s".into(),
                workload_id: "w".into(),
                verifier_peer: "p".into(),
                verified: true,
                verification_time_ms: 1,
                timestamp: now(),
            })
            .unwrap(),
            serde_json::to_string(&WorkerHeartbeat {
                peer_id: "p".into(),
                uptime_seconds: 0,
                tasks_completed: 0,
                proofs_verified: 0,
                timestamp: now(),
            })
            .unwrap(),
            serde_json::to_string(&PeerAnnouncement {
                peer_id: "p".into(),
                addresses: vec!["addr".into()],
                capabilities: vec!["compute".into()],
                timestamp: now(),
            })
            .unwrap(),
        ];
        assert_eq!(types.len(), 8, "All 8 new message types must serialize");
    }

    #[test]
    fn test_legacy_types_preserved() {
        let req = ComputeRequest {
            workload_id: "w".into(),
            instructions: vec!["MOV".into()],
            difficulty: 1,
        };
        let json = serde_json::to_string(&req).unwrap();
        let decoded: ComputeRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.workload_id, "w");

        let resp = ComputeResponse {
            workload_id: "w".into(),
            result: vec![42],
            proof_valid: true,
            execution_time_ms: 100,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let decoded: ComputeResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.result, vec![42]);
    }
}
