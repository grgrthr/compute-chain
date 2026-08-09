use crate::stark::prover::{StarkProof, StarkProver};
use crate::stark::trace::ExecutionTrace;
use crate::stark::verifier::StarkVerifier;
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofMetadata {
    pub trace_root: String,
    pub execution_steps: usize,
    pub proof_size_bytes: usize,
    pub generation_time_ms: u64,
    pub verifier_version: String,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofResult {
    pub proof: StarkProof,
    pub metadata: ProofMetadata,
    pub verified: bool,
}

pub struct ProofManager {
    prover: StarkProver,
    verifier: StarkVerifier,
    verifier_version: String,
}

impl ProofManager {
    pub fn new() -> Self {
        ProofManager {
            prover: StarkProver::new(),
            verifier: StarkVerifier::new(),
            verifier_version: "1.0.0".into(),
        }
    }

    pub fn generate_proof(&self, trace: &ExecutionTrace) -> Result<ProofResult, String> {
        let start = Instant::now();
        let proof = match self.prover.prove(trace) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("DEBUG PROOF: prover.prove failed: {}", e);
                return Err(e);
            }
        };
        let generation_time_ms = start.elapsed().as_millis() as u64;
        let trace_values = trace.to_values();
        let verified = self.verifier.verify(&proof, &trace_values, &[]);
        let proof_size_bytes = serde_json::to_vec(&proof).map(|v| v.len()).unwrap_or(0);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Ok(ProofResult {
            proof: proof.clone(),
            metadata: ProofMetadata {
                trace_root: proof.trace_root.clone(),
                execution_steps: proof.trace_length,
                proof_size_bytes,
                generation_time_ms,
                verifier_version: self.verifier_version.clone(),
                generated_at: format!("{}", now),
            },
            verified,
        })
    }

    pub fn verify_proof(&self, proof: &StarkProof, trace_values: &[u64]) -> bool {
        self.verifier.verify(proof, trace_values, &[])
    }

    pub fn quick_verify_proof(&self, proof: &StarkProof) -> bool {
        self.verifier.quick_verify(proof)
    }

    pub fn version(&self) -> &str {
        &self.verifier_version
    }
}

impl Default for ProofManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stark::trace::TraceStep;

    fn valid() -> ExecutionTrace {
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

    fn invalid() -> ExecutionTrace {
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
                pc: 5,
                opcode: 0xFF,
                reg_before: 5,
                reg_after: 5,
                mem_hash_before: vec![],
                mem_hash_after: vec![],
            },
        ])
    }

    #[test]
    fn test_valid_pipeline() {
        let m = ProofManager::new();
        let r = m.generate_proof(&valid()).unwrap();
        assert!(r.verified);
        assert_eq!(r.metadata.execution_steps, 3);
        assert!(!r.metadata.trace_root.is_empty());
        assert!(r.metadata.proof_size_bytes > 0);
    }
    #[test]
    fn test_invalid_rejected() {
        assert!(ProofManager::new().generate_proof(&invalid()).is_err());
    }
    #[test]
    fn test_modified_rejected() {
        let m = ProofManager::new();
        let t = valid();
        let r = m.generate_proof(&t).unwrap();
        let mut p = r.proof.clone();
        p.trace_root = "x".into();
        assert!(!m.verify_proof(&p, &t.to_values()));
    }
    #[test]
    fn test_deterministic() {
        let m = ProofManager::new();
        let t = valid();
        let a = m.generate_proof(&t).unwrap();
        let b = m.generate_proof(&t).unwrap();
        assert_eq!(a.proof.trace_root, b.proof.trace_root);
        assert_eq!(a.metadata.trace_root, b.metadata.trace_root);
    }
    #[test]
    fn test_repeated() {
        let m = ProofManager::new();
        let t = valid();
        let r = m.generate_proof(&t).unwrap();
        for _ in 0..10 {
            assert!(m.verify_proof(&r.proof, &t.to_values()));
        }
    }
    #[test]
    fn test_quick() {
        let m = ProofManager::new();
        assert!(m.quick_verify_proof(&m.generate_proof(&valid()).unwrap().proof));
    }
    #[test]
    fn test_empty() {
        assert!(ProofManager::new()
            .generate_proof(&ExecutionTrace::new(vec![]))
            .is_err());
    }
    #[test]
    fn test_metadata() {
        let m = ProofManager::new();
        let r = m.generate_proof(&valid()).unwrap();
        let d = r.metadata;
        assert!(!d.trace_root.is_empty());
        assert_eq!(d.execution_steps, 3);
        assert!(d.proof_size_bytes > 0);
        assert_eq!(d.verifier_version, "1.0.0");
    }
    #[test]
    fn test_version() {
        assert_eq!(ProofManager::new().version(), "1.0.0");
    }
}
