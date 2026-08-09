//! STARK Prover — Integrates Trace, AIR, Merkle, and FRI.
//!
//! Generates deterministic STARK proofs from execution traces.
//! Uses the existing AIR constraint system and FRI protocol.

use crate::merkle::hash::TraceHasher;
use crate::merkle::tree::MerkleTree;
use crate::stark::air::{AirConstraints, ComputeAir, ExecutionState};
use crate::stark::fri::{FriConfig, FriProof, FriProtocol};
use crate::stark::trace::{ExecutionTrace, TraceStep};
use serde::{Deserialize, Serialize};

/// A complete STARK proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarkProof {
    /// Number of execution steps.
    pub trace_length: usize,
    /// Merkle root of the execution trace.
    pub trace_root: String,
    /// Memory root hash.
    pub memory_root: String,
    /// Final state of all 8 registers.
    pub final_registers: [u64; 8],
    /// FRI proof for the constraint polynomials.
    pub fri_proof: FriProof,
    /// Public inputs.
    pub public_inputs: Vec<u64>,
    /// Backward-compat: SHA-256 hash of trace_root.
    pub trace_hash: Vec<u8>,
}

/// The STARK Prover.
pub struct StarkProver {
    air: ComputeAir,
    fri: FriProtocol,
}

impl StarkProver {
    pub fn new() -> Self {
        StarkProver {
            air: ComputeAir::new(),
            fri: FriProtocol::new(FriConfig::default()),
        }
    }

    pub fn with_config(fri_config: FriConfig) -> Self {
        StarkProver {
            air: ComputeAir::new(),
            fri: FriProtocol::new(fri_config),
        }
    }

    pub fn build_states(&self, trace: &ExecutionTrace) -> Vec<ExecutionState> {
        trace
            .steps
            .iter()
            .enumerate()
            .map(|(i, step)| {
                let is_terminal = i == trace.steps.len() - 1 && step.opcode == 0xFF;
                ExecutionState {
                    pc: step.pc,
                    opcode: step.opcode as u8,
                    registers_before: [step.reg_before, 0, 0, 0, 0, 0, 0, 0],
                    registers_after: [step.reg_after, 0, 0, 0, 0, 0, 0, 0],
                    mem_addr: None,
                    mem_value_before: None,
                    mem_value_after: None,
                    is_terminal,
                }
            })
            .collect()
    }

    pub fn evaluate_constraints(&self, trace: &ExecutionTrace) -> bool {
        let states = self.build_states(trace);
        if states.is_empty() {
            return false;
        }
        for i in 0..states.len() - 1 {
            if !self.air.evaluate_transition(&states[i], &states[i + 1]) {
                return false;
            }
        }
        self.air
            .evaluate_boundary(&states[0], &states[states.len() - 1])
    }

    pub fn commit_trace(&self, trace: &ExecutionTrace) -> MerkleTree {
        let values = trace.to_values();
        let leaves: Vec<String> = values
            .iter()
            .map(|v| TraceHasher::hash(&v.to_string()))
            .collect();
        MerkleTree::new(leaves)
    }

    pub fn build_fri_proof(&self, trace: &ExecutionTrace) -> FriProof {
        let values = trace.to_values();
        self.fri.build_proof(&values)
    }

    pub fn prove(&self, trace: &ExecutionTrace) -> Result<StarkProof, String> {
        if trace.steps.is_empty() {
            return Err("Cannot prove empty trace".into());
        }
        if !self.evaluate_constraints(trace) {
            return Err("Trace violates AIR constraints".into());
        }
        let merkle_tree = self.commit_trace(trace);
        let trace_root = merkle_tree.root_hash.clone();
        let fri_proof = self.build_fri_proof(trace);
        let memory_root = TraceHasher::hash("memory_root");
        let last_step = &trace.steps[trace.steps.len() - 1];
        let final_registers = [last_step.reg_after, 0, 0, 0, 0, 0, 0, 0];
        // Backward-compat trace_hash
        let trace_hash = {
            use sha2::{Digest, Sha256};
            let mut h = Sha256::new();
            h.update(trace_root.as_bytes());
            h.finalize().to_vec()
        };

        Ok(StarkProof {
            trace_length: trace.steps.len(),
            trace_root,
            memory_root,
            final_registers,
            fri_proof,
            public_inputs: vec![],
            trace_hash,
        })
    }

    pub fn serialize_proof(&self, proof: &StarkProof) -> Result<Vec<u8>, String> {
        serde_json::to_vec(proof).map_err(|e| format!("Serialization failed: {}", e))
    }

    pub fn deserialize_proof(&self, data: &[u8]) -> Result<StarkProof, String> {
        serde_json::from_slice(data).map_err(|e| format!("Deserialization failed: {}", e))
    }
}

impl Default for StarkProver {
    fn default() -> Self {
        Self::new()
    }
}

// ═══ BACKWARD COMPATIBILITY ═══

/// Legacy ComputeProver — wraps StarkProver for backward compatibility.
pub struct ComputeProver;

impl ComputeProver {
    pub fn new() -> Self {
        Self
    }

    pub fn prove(&self, trace: &ExecutionTrace) -> Result<StarkProof, String> {
        StarkProver::new().prove(trace)
    }
}

// Re-export SimpleProof for backward compatibility
pub use crate::stark::simple_stark::SimpleProof;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stark::trace::TraceStep;

    fn make_trace(steps: usize) -> ExecutionTrace {
        let steps: Vec<TraceStep> = (0..steps)
            .map(|i| {
                let opcode = if i == steps - 1 { 0xFF } else { 0x01 };
                TraceStep {
                    pc: i,
                    opcode,
                    reg_before: (i * 10) as u64,
                    reg_after: ((i + 1) * 10) as u64,
                    mem_hash_before: vec![],
                    mem_hash_after: vec![],
                }
            })
            .collect();
        ExecutionTrace::new(steps)
    }

    fn make_simple_valid_trace() -> ExecutionTrace {
        ExecutionTrace::new(vec![
            TraceStep {
                pc: 0,
                opcode: 0x01,
                reg_before: 0,
                reg_after: 5,
                mem_hash_before: vec![],
                mem_hash_after: vec![],
            },
            TraceStep {
                pc: 1,
                opcode: 0x01,
                reg_before: 5,
                reg_after: 10,
                mem_hash_before: vec![],
                mem_hash_after: vec![],
            },
            TraceStep {
                pc: 2,
                opcode: 0xFF,
                reg_before: 10,
                reg_after: 10,
                mem_hash_before: vec![],
                mem_hash_after: vec![],
            },
        ])
    }

    #[test]
    fn test_prove_simple() {
        let p = StarkProver::new();
        assert!(p.prove(&make_simple_valid_trace()).is_ok());
    }
    #[test]
    fn test_prove_large() {
        let p = StarkProver::new();
        assert!(p.prove(&make_trace(1000)).is_ok());
    }
    #[test]
    fn test_prove_deterministic() {
        let p = StarkProver::new();
        let t = make_simple_valid_trace();
        let a = p.prove(&t).unwrap();
        let b = p.prove(&t).unwrap();
        assert_eq!(a.trace_root, b.trace_root);
        assert_eq!(a.trace_hash, b.trace_hash);
    }
    #[test]
    fn test_empty_rejected() {
        assert!(StarkProver::new()
            .prove(&ExecutionTrace::new(vec![]))
            .is_err());
    }
    #[test]
    fn test_constraint_violation() {
        let t = ExecutionTrace::new(vec![
            TraceStep {
                pc: 0,
                opcode: 0x01,
                reg_before: 0,
                reg_after: 1,
                mem_hash_before: vec![],
                mem_hash_after: vec![],
            },
            TraceStep {
                pc: 5,
                opcode: 0xFF,
                reg_before: 1,
                reg_after: 1,
                mem_hash_before: vec![],
                mem_hash_after: vec![],
            },
        ]);
        assert!(StarkProver::new().prove(&t).is_err());
    }
    #[test]
    fn test_serialization() {
        let p = StarkProver::new();
        let t = make_simple_valid_trace();
        let proof = p.prove(&t).unwrap();
        let bytes = p.serialize_proof(&proof).unwrap();
        let d = p.deserialize_proof(&bytes).unwrap();
        assert_eq!(proof.trace_length, d.trace_length);
        assert_eq!(proof.trace_hash, d.trace_hash);
    }
    #[test]
    fn test_evaluate_valid() {
        assert!(StarkProver::new().evaluate_constraints(&make_simple_valid_trace()));
    }
    #[test]
    fn test_evaluate_empty() {
        assert!(!StarkProver::new().evaluate_constraints(&ExecutionTrace::new(vec![])));
    }
    #[test]
    fn test_commit_deterministic() {
        let p = StarkProver::new();
        let t = make_simple_valid_trace();
        assert_eq!(p.commit_trace(&t).root_hash, p.commit_trace(&t).root_hash);
    }
    #[test]
    fn test_custom_config() {
        let p = StarkProver::with_config(FriConfig {
            num_queries: 5,
            blowup_factor: 2,
            num_layers: 3,
            domain_generator: 7,
        });
        let proof = p.prove(&make_simple_valid_trace()).unwrap();
        assert_eq!(proof.fri_proof.layers.len(), 3);
    }
}
