//! Integration Runner — End-to-end execution flow.
//!
//! Connects every major component:
//!   User → Scheduler → Worker → VM → Trace → Merkle → Proof → Verify → Complete
//!
//! This is the first fully working version of Compute Chain.

use crate::merkle::hash::TraceHasher;
use crate::merkle::tree::MerkleTree;
use crate::scheduler::{Job, JobStatus, Scheduler, SchedulerConfig, Worker};
use crate::stark::proof_manager::ProofManager;
use crate::stark::prover::StarkProof;
use crate::trace::trace_record::TraceRecord;
use crate::vm::cpu::Cpu;
use crate::vm::executor::Executor;
use crate::vm::instruction::Instruction;
use crate::vm::memory::Memory;
use crate::vm::program::Program;
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// The result of a complete end-to-end run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationResult {
    /// Whether the job completed successfully.
    pub success: bool,
    /// Final VM register values.
    pub final_registers: Vec<u64>,
    /// Number of execution steps in the trace.
    pub trace_length: usize,
    /// Merkle root of the execution trace.
    pub merkle_root: String,
    /// Whether a STARK proof was generated.
    pub proof_generated: bool,
    /// Whether the proof was verified.
    pub proof_verified: bool,
    /// Final job status.
    pub job_status: String,
    /// Total execution time in milliseconds.
    pub execution_time_ms: u64,
    /// Error message if any step failed.
    pub error: Option<String>,
}

/// The IntegrationRunner orchestrates the full pipeline.
pub struct IntegrationRunner {
    scheduler: Scheduler,
    proof_manager: ProofManager,
}

impl IntegrationRunner {
    pub fn new() -> Self {
        IntegrationRunner {
            scheduler: Scheduler::new(SchedulerConfig::default()),
            proof_manager: ProofManager::new(),
        }
    }

    /// Run a complete end-to-end flow.
    ///
    /// Steps:
    /// 1. Register a worker
    /// 2. Submit the job
    /// 3. Assign job to worker
    /// 4. Execute VM program
    /// 5. Build execution trace
    /// 6. Build Merkle tree
    /// 7. Generate STARK proof
    /// 8. Verify proof
    /// 9. Mark job completed
    pub fn run(&mut self, job_id: &str, instructions: Vec<Instruction>) -> IntegrationResult {
        let start = Instant::now();

        // ═══ STEP 1: Register worker ═══
        let worker_id = "worker_1";
        self.scheduler.register_worker(Worker::new(worker_id, 10));

        // ═══ STEP 2: Submit job ═══
        let job = Job::new(job_id, 1);
        self.scheduler.enqueue(job);

        // ═══ STEP 3: Assign job ═══
        let (assigned_job_id, _assigned_worker) = match self.scheduler.assign_job() {
            Some(assignment) => assignment,
            None => {
                return IntegrationResult {
                    success: false,
                    final_registers: vec![],
                    trace_length: 0,
                    merkle_root: String::new(),
                    proof_generated: false,
                    proof_verified: false,
                    job_status: "Failed".into(),
                    execution_time_ms: start.elapsed().as_millis() as u64,
                    error: Some("No worker available".into()),
                };
            }
        };

        // ═══ STEP 4: Execute VM ═══
        let program = Program::new(instructions);
        let mut cpu = Cpu::new();
        let mut memory = Memory::new(65536);
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

        let final_registers = cpu.registers.to_vec();

        // ═══ STEP 5: Build execution trace ═══
        let trace_steps: Vec<crate::stark::trace::TraceStep> = records
            .iter()
            .map(|r| {
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

        let trace = crate::stark::trace::ExecutionTrace::new(trace_steps);

        // ═══ STEP 6: Build Merkle tree ═══
        let values = trace.to_values();
        let leaves: Vec<String> = values
            .iter()
            .map(|v| TraceHasher::hash(&v.to_string()))
            .collect();
        let merkle_tree = MerkleTree::new(leaves);
        let merkle_root = merkle_tree.root_hash.clone();

        // ═══ STEP 7: Generate STARK proof ═══
        // Debug: check trace before proof
        eprintln!(
            "DEBUG: trace len={}, values={}",
            trace.len(),
            trace.to_values().len()
        );
        eprintln!(
            "DEBUG: first step pc={}, last step opcode={:x}",
            trace.steps.first().map(|s| s.pc).unwrap_or(999),
            trace.steps.last().map(|s| s.opcode).unwrap_or(999)
        );

        let proof_result = self.proof_manager.generate_proof(&trace);
        let (proof_generated, _proof_verified) = match proof_result {
            Ok(result) => (true, result.verified),
            Err(_) => (false, false),
        };

        // ═══ STEP 8: Verify proof ═══
        let verified = if proof_generated {
            self.proof_manager.verify_proof(
                &self.proof_manager.generate_proof(&trace).unwrap().proof,
                &trace.to_values(),
            )
        } else {
            false
        };

        // ═══ STEP 9: Mark job completed ═══
        let overall_success = proof_generated && verified && !cpu.halted == false;
        let status = if overall_success {
            self.scheduler
                .complete_job(&assigned_job_id, true, Some(&merkle_root))
                .ok();
            "Completed".to_string()
        } else {
            self.scheduler
                .complete_job(&assigned_job_id, false, None)
                .ok();
            "Failed".to_string()
        };

        IntegrationResult {
            success: overall_success,
            final_registers,
            trace_length: trace.len(),
            merkle_root,
            proof_generated,
            proof_verified: verified,
            job_status: status,
            execution_time_ms: start.elapsed().as_millis() as u64,
            error: if overall_success {
                None
            } else {
                Some("Proof generation or verification failed".into())
            },
        }
    }
}

impl Default for IntegrationRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_program() -> Vec<Instruction> {
        vec![
            Instruction::Mov {
                register: 0,
                value: 5,
            },
            Instruction::Mov {
                register: 1,
                value: 10,
            },
            Instruction::Add {
                destination: 0,
                source: 1,
            },
            Instruction::Halt,
        ]
    }

    fn mul_program() -> Vec<Instruction> {
        vec![
            Instruction::Mov {
                register: 0,
                value: 42,
            },
            Instruction::Mov {
                register: 1,
                value: 7,
            },
            Instruction::Mul {
                destination: 0,
                source: 1,
            },
            Instruction::Halt,
        ]
    }

    // ═══ END-TO-END TESTS ═══

    #[test]
    fn test_end_to_end_add() {
        let mut runner = IntegrationRunner::new();
        let result = runner.run("e2e_add", add_program());

        assert!(result.success, "E2E should succeed: {:?}", result.error);
        assert_eq!(result.final_registers[0], 15, "5 + 10 = 15");
        assert!(result.trace_length > 0, "Trace must not be empty");
        assert!(!result.merkle_root.is_empty(), "Merkle root must exist");
        assert!(result.proof_generated, "Proof must be generated");
        assert!(result.proof_verified, "Proof must be verified");
        assert_eq!(result.job_status, "Completed");
    }

    #[test]
    fn test_end_to_end_mul() {
        let mut runner = IntegrationRunner::new();
        let result = runner.run("e2e_mul", mul_program());

        assert!(result.success, "E2E mul should succeed: {:?}", result.error);
        assert_eq!(result.final_registers[0], 294, "42 * 7 = 294");
        assert!(result.trace_length > 0);
        assert!(!result.merkle_root.is_empty());
        assert!(result.proof_generated);
        assert!(result.proof_verified);
        assert_eq!(result.job_status, "Completed");
    }

    #[test]
    fn test_end_to_end_deterministic() {
        let r1 = IntegrationRunner::new().run("e2e_det", add_program());
        let r2 = IntegrationRunner::new().run("e2e_det", add_program());

        assert_eq!(r1.final_registers, r2.final_registers);
        assert_eq!(r1.merkle_root, r2.merkle_root);
        assert_eq!(r1.trace_length, r2.trace_length);
        assert_eq!(r1.success, r2.success);
    }
}
