use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleProof {
    pub commitments: Vec<Vec<u8>>,
    pub trace_hash: Vec<u8>,
    pub trace_length: usize,
    pub final_state: Vec<u64>,
    pub memory_root: Vec<u8>,
    pub fri_layers: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchProof {
    pub proofs: Vec<SimpleProof>,
    pub aggregated_hash: Vec<u8>,
    pub batch_size: usize,
}

pub struct SimpleStark;

impl SimpleStark {
    pub fn prove(trace: &[u64]) -> SimpleProof {
        let mut commitments = Vec::new();
        let mut fri_layers = Vec::new();

        let level0 = Self::hash_trace(trace);
        commitments.push(level0.clone());
        fri_layers.push(level0.clone());

        let level1 = Self::fri_fold(&level0);
        commitments.push(level1.clone());
        fri_layers.push(level1.clone());

        let level2 = Self::hash_pair(&level1, &level1);
        commitments.push(level2.clone());

        let mem_root = Self::hash_trace(&[trace.len() as u64]);

        SimpleProof {
            trace_hash: level2,
            commitments,
            trace_length: trace.len(),
            final_state: if trace.is_empty() {
                vec![]
            } else {
                vec![trace[trace.len() - 1]]
            },
            memory_root: mem_root,
            fri_layers,
        }
    }

    /// 🆕 Batch proof - إثبات مجموعة traces دفعة واحدة
    pub fn prove_batch(traces: &[Vec<u64>]) -> BatchProof {
        let proofs: Vec<SimpleProof> = traces.iter().map(|t| Self::prove(t)).collect();

        // تجميع الهاشات
        let mut hasher = Sha256::new();
        for proof in &proofs {
            hasher.update(&proof.trace_hash);
        }
        let aggregated_hash = hasher.finalize().to_vec();

        BatchProof {
            proofs,
            aggregated_hash,
            batch_size: traces.len(),
        }
    }

    /// 🆕 Verify batch proof
    pub fn verify_batch(batch: &BatchProof) -> bool {
        let mut hasher = Sha256::new();
        for proof in &batch.proofs {
            if !Self::verify(proof, &proof.trace_hash) {
                return false;
            }
            hasher.update(&proof.trace_hash);
        }
        hasher.finalize().to_vec() == batch.aggregated_hash
    }

    pub fn verify(proof: &SimpleProof, expected_hash: &[u8]) -> bool {
        if proof.commitments.is_empty() {
            return false;
        }
        let root = &proof.commitments.last().unwrap();
        *root == expected_hash
    }

    pub fn quick_verify(trace: &[u64], proof: &SimpleProof) -> bool {
        let recomputed = Self::prove(trace);
        recomputed.trace_hash == proof.trace_hash
    }

    /// 🆕 Compressed proof - إثبات مضغوط
    pub fn compress(proof: &SimpleProof) -> Vec<u8> {
        let mut compressed = Vec::new();
        // تخزين root hash فقط + trace_length + final_state
        compressed.extend_from_slice(&proof.trace_hash);
        compressed.extend_from_slice(&proof.trace_length.to_le_bytes());
        for &val in &proof.final_state {
            compressed.extend_from_slice(&val.to_le_bytes());
        }
        compressed
    }

    /// 🆕 Verify compressed proof
    pub fn verify_compressed(compressed: &[u8], trace: &[u64]) -> bool {
        if compressed.len() < 40 {
            return false;
        }

        let expected_hash = &compressed[..32];
        let stored_len = u64::from_le_bytes(compressed[32..40].try_into().unwrap_or([0; 8]));

        if stored_len != trace.len() as u64 {
            return false;
        }

        let proof = Self::prove(trace);
        proof.trace_hash == expected_hash
    }

    fn fri_fold(hash: &[u8]) -> Vec<u8> {
        let mut hasher = Sha256::new();
        let mid = hash.len() / 2;
        for i in 0..mid {
            hasher.update(&[hash[i] ^ hash[i + mid]]);
        }
        hasher.finalize().to_vec()
    }

    fn hash_trace(trace: &[u64]) -> Vec<u8> {
        let mut hasher = Sha256::new();
        for &value in trace {
            hasher.update(&value.to_le_bytes());
        }
        hasher.finalize().to_vec()
    }

    fn hash_pair(a: &[u8], b: &[u8]) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(a);
        hasher.update(b);
        hasher.finalize().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prove_verify() {
        let trace = vec![1, 2, 3, 4, 5];
        let proof = SimpleStark::prove(&trace);
        assert!(SimpleStark::verify(&proof, &proof.trace_hash));
    }

    #[test]
    fn test_batch_proof() {
        let traces = vec![
            vec![1, 2, 3, 4, 5],
            vec![10, 20, 30, 40, 50],
            vec![100, 200, 300, 400, 500],
        ];
        let batch = SimpleStark::prove_batch(&traces);
        assert_eq!(batch.batch_size, 3);
        assert!(SimpleStark::verify_batch(&batch));
    }

    #[test]
    fn test_compressed_proof() {
        let trace = vec![1, 2, 3, 4, 5];
        let proof = SimpleStark::prove(&trace);
        let compressed = SimpleStark::compress(&proof);
        assert!(SimpleStark::verify_compressed(&compressed, &trace));
    }

    #[test]
    fn test_quick_verify() {
        let trace = vec![10, 20, 30, 40, 50];
        let proof = SimpleStark::prove(&trace);
        assert!(SimpleStark::quick_verify(&trace, &proof));
    }

    #[test]
    fn test_wrong_trace_fails() {
        let trace1 = vec![1, 2, 3];
        let trace2 = vec![1, 2, 4];
        let proof = SimpleStark::prove(&trace1);
        assert!(!SimpleStark::quick_verify(&trace2, &proof));
    }

    #[test]
    fn test_memory_root() {
        let trace = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let proof = SimpleStark::prove(&trace);
        assert_eq!(proof.memory_root.len(), 32);
    }

    #[test]
    fn test_fri_layers() {
        let trace = vec![1, 2, 3, 4, 5];
        let proof = SimpleStark::prove(&trace);
        assert_eq!(proof.fri_layers.len(), 2);
    }
}

// Strategy: Move Dependencies Down for stark (Infrastructure)
// Review and adjust before applying.
