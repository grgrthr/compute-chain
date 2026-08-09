//! Proof Network Integration — Connects the STARK Proof Pipeline with the P2P network.
//!
//! Workers execute Compute VM workloads, generate STARK proofs,
//! and submit results with proofs to the network.
//! Receiving nodes verify proofs before accepting results.

use crate::p2p::message::{
    ProofSubmission, ProofVerification, WorkerHeartbeat, WorkloadAnnouncement, WorkloadAssignment,
    WorkloadRequest, WorkloadResult,
};
use crate::p2p::node::{NetworkMessage, P2PCommand, P2PEvent, P2PHandle};
use crate::stark::proof_manager::ProofManager;
use crate::stark::prover::StarkProof;
use crate::stark::trace::{build_trace_from_records, ExecutionTrace};
use crate::trace::trace_record::TraceRecord;
use crate::vm::cpu::Cpu;
use crate::vm::executor::Executor;
use crate::vm::instruction::Instruction;
use crate::vm::memory::Memory;
use crate::vm::program::Program;
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// A compute job that workers execute.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComputeJob {
    pub job_id: String,
    pub instructions: Vec<InstructionData>,
    pub input_registers: Vec<u64>,
    pub difficulty: u32,
}

/// Serializable instruction data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstructionData {
    pub opcode: String,
    pub params: Vec<u64>,
}

impl InstructionData {
    pub fn to_instruction(&self) -> Option<Instruction> {
        match self.opcode.as_str() {
            "MOV" => Some(Instruction::Mov {
                register: self.params.get(0).copied().unwrap_or(0) as usize,
                value: self.params.get(1).copied().unwrap_or(0),
            }),
            "ADD" => Some(Instruction::Add {
                destination: self.params.get(0).copied().unwrap_or(0) as usize,
                source: self.params.get(1).copied().unwrap_or(0) as usize,
            }),
            "SUB" => Some(Instruction::Sub {
                destination: self.params.get(0).copied().unwrap_or(0) as usize,
                source: self.params.get(1).copied().unwrap_or(0) as usize,
            }),
            "MUL" => Some(Instruction::Mul {
                destination: self.params.get(0).copied().unwrap_or(0) as usize,
                source: self.params.get(1).copied().unwrap_or(0) as usize,
            }),
            "DIV" => Some(Instruction::Div {
                destination: self.params.get(0).copied().unwrap_or(0) as usize,
                source: self.params.get(1).copied().unwrap_or(0) as usize,
            }),
            "HALT" => Some(Instruction::Halt),
            "PUSH" => Some(Instruction::Push {
                value: self.params.get(0).copied().unwrap_or(0),
            }),
            "POP" => Some(Instruction::Pop {
                register: self.params.get(0).copied().unwrap_or(0) as usize,
            }),
            _ => None,
        }
    }
}

/// Result of executing a compute job with proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeJobResult {
    pub job_id: String,
    pub success: bool,
    pub trace_root: String,
    pub execution_steps: usize,
    pub execution_time_ms: u64,
    pub final_registers: Vec<u64>,
    pub proof: Option<SerializableProof>,
    pub proof_verified: bool,
    pub error: Option<String>,
}

/// Lightweight serializable proof for network transport.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableProof {
    pub trace_root: String,
    pub trace_hash: Vec<u8>,
    pub trace_length: usize,
    pub final_registers: [u64; 8],
    pub proof_size_bytes: usize,
}

impl From<StarkProof> for SerializableProof {
    fn from(p: StarkProof) -> Self {
        let size = serde_json::to_vec(&p).map(|v| v.len()).unwrap_or(0);
        SerializableProof {
            trace_root: p.trace_root,
            trace_hash: p.trace_hash,
            trace_length: p.trace_length,
            final_registers: p.final_registers,
            proof_size_bytes: size,
        }
    }
}

/// A worker node that executes compute jobs and generates proofs.
pub struct WorkerNode {
    pub peer_id: String,
    pub proof_manager: ProofManager,
    pub completed_jobs: Arc<Mutex<HashMap<String, ComputeJobResult>>>,
    pub verified_proofs: Arc<Mutex<HashMap<String, bool>>>,
    pub uptime: Instant,
    tasks_completed: Cell<u64>,
    proofs_verified: Arc<Mutex<u64>>,
}

impl WorkerNode {
    pub fn new(peer_id: &str) -> Self {
        WorkerNode {
            peer_id: peer_id.to_string(),
            proof_manager: ProofManager::new(),
            completed_jobs: Arc::new(Mutex::new(HashMap::new())),
            verified_proofs: Arc::new(Mutex::new(HashMap::new())),
            uptime: Instant::now(),
            tasks_completed: Cell::new(0),
            proofs_verified: Arc::new(Mutex::new(0)),
        }
    }

    /// Execute a compute job: VM → Trace → Proof → Verify
    pub fn execute_job(&self, job: &ComputeJob) -> ComputeJobResult {
        let start = Instant::now();

        // 1. Build VM program
        let instructions: Vec<Instruction> = job
            .instructions
            .iter()
            .filter_map(|d| d.to_instruction())
            .collect();

        if instructions.is_empty() {
            return ComputeJobResult {
                job_id: job.job_id.clone(),
                success: false,
                trace_root: String::new(),
                execution_steps: 0,
                execution_time_ms: start.elapsed().as_millis() as u64,
                final_registers: vec![],
                proof: None,
                proof_verified: false,
                error: Some("No valid instructions".into()),
            };
        }

        let program = Program::new(instructions);
        let mut cpu = Cpu::new();
        let mut memory = Memory::new(65536);

        // Set input registers
        for (i, &val) in job.input_registers.iter().enumerate() {
            if i < 8 {
                cpu.registers[i] = val;
            }
        }

        // 2. Execute VM and record trace
        let mut records: Vec<TraceRecord> = Vec::new();
        let mut step = 0;

        while !cpu.halted {
            match Executor::step(&mut cpu, &mut memory, &program) {
                Some(r) => {
                    records.push(TraceRecord {
                        step,
                        pc: r.pc,
                        instruction: r.instruction,
                        registers_before: r.registers_before,
                        registers_after: r.registers_after,
                    });
                    step += 1;
                }
                None => break,
            }
        }

        let execution_time_ms = start.elapsed().as_millis() as u64;
        let final_registers = cpu.registers.to_vec();

        // 3. Build execution trace — convert TraceRecord to TraceStep directly
        let trace_steps: Vec<crate::stark::trace::TraceStep> = records
            .iter()
            .enumerate()
            .map(|(_i, r)| {
                let opcode = match &r.instruction {
                    Instruction::Mov { .. } => 0x01,
                    Instruction::Add { .. } => 0x02,
                    Instruction::Sub { .. } => 0x03,
                    Instruction::Mul { .. } => 0x04,
                    Instruction::Div { .. } => 0x05,
                    Instruction::Load { .. } => 0x10,
                    Instruction::Store { .. } => 0x11,
                    Instruction::Jump { .. } => 0x20,
                    Instruction::Cmp { .. } => 0x30,
                    Instruction::Call { .. } => 0x31,
                    Instruction::Push { .. } => 0x50,
                    Instruction::Pop { .. } => 0x51,
                    Instruction::Sha256 { .. } => 0x60,
                    Instruction::Ret => 0x32,
                    Instruction::CallData { .. } => 0x33,
                    Instruction::Log { .. } => 0x34,
                    Instruction::SelfBalance { .. } => 0x35,
                    Instruction::Halt => 0xFF,
                };
                crate::stark::trace::TraceStep {
                    pc: r.pc,
                    opcode,
                    reg_before: *r.registers_before.first().unwrap_or(&0),
                    reg_after: *r.registers_after.first().unwrap_or(&0),
                    mem_hash_before: vec![],
                    mem_hash_after: vec![],
                }
            })
            .collect();
        let trace = ExecutionTrace::new(trace_steps);
        let trace_root = {
            let values = trace.to_values();
            let leaves: Vec<String> = values
                .iter()
                .map(|v| crate::merkle::hash::TraceHasher::hash(&v.to_string()))
                .collect();
            crate::merkle::tree::MerkleTree::new(leaves).root_hash
        };

        // 4. Generate STARK proof
        let proof_result = self.proof_manager.generate_proof(&trace);

        match proof_result {
            Ok(result) => {
                let serializable: SerializableProof = result.proof.into();

                // 5. Verify proof locally
                let verified = result.verified;

                self.tasks_completed.set(self.tasks_completed.get() + 1);

                ComputeJobResult {
                    job_id: job.job_id.clone(),
                    success: true,
                    trace_root,
                    execution_steps: step,
                    execution_time_ms,
                    final_registers,
                    proof: Some(serializable),
                    proof_verified: verified,
                    error: None,
                }
            }
            Err(e) => ComputeJobResult {
                job_id: job.job_id.clone(),
                success: false,
                trace_root,
                execution_steps: step,
                execution_time_ms,
                final_registers,
                proof: None,
                proof_verified: false,
                error: Some(e),
            },
        }
    }

    /// Verify a submitted proof from another worker.
    pub fn verify_submitted_proof(
        &self,
        submission: &ProofSubmission,
        trace_values: &[u64],
        proof: &StarkProof,
    ) -> bool {
        let result = self.proof_manager.verify_proof(proof, trace_values);
        if result {
            *self.proofs_verified.lock().unwrap() += 1;
        }
        self.verified_proofs
            .lock()
            .unwrap()
            .insert(submission.submission_id.clone(), result);
        result
    }

    /// Generate a heartbeat status.
    pub fn heartbeat(&self) -> WorkerHeartbeat {
        WorkerHeartbeat {
            peer_id: self.peer_id.clone(),
            uptime_seconds: self.uptime.elapsed().as_secs(),
            tasks_completed: self.tasks_completed.get(),
            proofs_verified: *self.proofs_verified.lock().unwrap(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    /// Get all completed jobs.
    pub fn get_completed_jobs(&self) -> HashMap<String, ComputeJobResult> {
        self.completed_jobs.lock().unwrap().clone()
    }
}

/// Create a sample compute job for testing.
pub fn sample_compute_job() -> ComputeJob {
    ComputeJob {
        job_id: format!("job_{}", uuid::Uuid::new_v4()),
        instructions: vec![
            InstructionData {
                opcode: "MOV".into(),
                params: vec![0, 5],
            },
            InstructionData {
                opcode: "MOV".into(),
                params: vec![1, 10],
            },
            InstructionData {
                opcode: "ADD".into(),
                params: vec![0, 1],
            },
            InstructionData {
                opcode: "HALT".into(),
                params: vec![],
            },
        ],
        input_registers: vec![0, 0, 0, 0, 0, 0, 0, 0],
        difficulty: 1,
    }
}

/// Create a more complex compute job.
pub fn complex_compute_job() -> ComputeJob {
    ComputeJob {
        job_id: format!("job_{}", uuid::Uuid::new_v4()),
        instructions: vec![
            InstructionData {
                opcode: "MOV".into(),
                params: vec![0, 42],
            },
            InstructionData {
                opcode: "MOV".into(),
                params: vec![1, 7],
            },
            InstructionData {
                opcode: "MUL".into(),
                params: vec![0, 1],
            },
            InstructionData {
                opcode: "PUSH".into(),
                params: vec![100],
            },
            InstructionData {
                opcode: "POP".into(),
                params: vec![2],
            },
            InstructionData {
                opcode: "ADD".into(),
                params: vec![0, 2],
            },
            InstructionData {
                opcode: "HALT".into(),
                params: vec![],
            },
        ],
        input_registers: vec![0; 8],
        difficulty: 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_execute_simple_job() {
        let worker = WorkerNode::new("test_peer");
        let job = sample_compute_job();
        let result = worker.execute_job(&job);

        assert!(result.success, "Job should succeed: {:?}", result.error);
        assert_eq!(result.execution_steps, 4);
        assert!(!result.trace_root.is_empty());
        assert!(result.proof.is_some(), "Proof should be generated");
        assert!(result.proof_verified, "Proof should be locally verified");
        // 5 + 10 = 15
        assert_eq!(result.final_registers[0], 15);
    }

    #[test]
    fn test_worker_execute_complex_job() {
        let worker = WorkerNode::new("test_peer");
        let job = complex_compute_job();
        let result = worker.execute_job(&job);

        assert!(
            result.success,
            "Complex job should succeed: {:?}",
            result.error
        );
        assert!(result.execution_steps > 0);
        // 42 * 7 = 294, then push 100, pop to reg 2, add 100 to 294 = 394
        assert_eq!(result.final_registers[0], 394);
        assert!(result.proof_verified);
    }

    #[test]
    fn test_worker_heartbeat() {
        let worker = WorkerNode::new("test_peer");
        let job = sample_compute_job();
        worker.execute_job(&job);

        let hb = worker.heartbeat();
        assert_eq!(hb.peer_id, "test_peer");
        assert_eq!(hb.tasks_completed, 1);
        assert!(hb.uptime_seconds < 5);
    }

    #[test]
    fn test_deterministic_execution() {
        let worker = WorkerNode::new("peer_a");
        let job = sample_compute_job();

        let r1 = worker.execute_job(&job);
        let r2 = worker.execute_job(&job);

        assert_eq!(
            r1.trace_root, r2.trace_root,
            "Same job must produce same trace root"
        );
        assert_eq!(r1.final_registers, r2.final_registers);
    }

    #[test]
    fn test_invalid_job_rejected() {
        let worker = WorkerNode::new("test_peer");
        let job = ComputeJob {
            job_id: "invalid".into(),
            instructions: vec![],
            input_registers: vec![],
            difficulty: 0,
        };
        let result = worker.execute_job(&job);
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_proof_verification_by_another_worker() {
        let worker_a = WorkerNode::new("peer_a");
        let worker_b = WorkerNode::new("peer_b");

        // Worker A executes job
        let job = sample_compute_job();
        let result_a = worker_a.execute_job(&job);
        assert!(result_a.success);

        // Worker B verifies by re-executing and comparing trace roots
        let result_b = worker_b.execute_job(&job);
        assert_eq!(
            result_a.trace_root, result_b.trace_root,
            "Different workers must produce identical trace roots"
        );
        assert_eq!(result_a.final_registers, result_b.final_registers);
    }

    #[test]
    fn test_multiple_jobs_tracking() {
        let worker = WorkerNode::new("multi_peer");

        for _ in 0..5 {
            let job = sample_compute_job();
            worker.execute_job(&job);
        }

        let hb = worker.heartbeat();
        assert_eq!(hb.tasks_completed, 5);
    }

    #[test]
    fn test_job_result_fields() {
        let worker = WorkerNode::new("test_peer");
        let job = sample_compute_job();
        let result = worker.execute_job(&job);

        assert!(!result.job_id.is_empty());
        // execution_time_ms may be 0 for very fast execution — accept >= 0
        assert!(!result.final_registers.is_empty());
        if let Some(proof) = &result.proof {
            assert!(!proof.trace_root.is_empty());
            assert!(!proof.trace_hash.is_empty());
            assert_eq!(proof.trace_length, 4);
            assert!(proof.proof_size_bytes > 0);
        }
    }
}
