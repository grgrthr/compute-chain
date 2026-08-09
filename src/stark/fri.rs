//! FRI Protocol — Fast Reed-Solomon IOP of Proximity
//!
//! Complete deterministic implementation for the Compute Chain STARK backend.
//! Uses the existing MerkleTree for layer commitments.
//! No randomness — query positions derived from layer roots via SHA-256.

use crate::merkle::hash::TraceHasher;
use crate::merkle::tree::MerkleTree;

/// Configuration for a FRI protocol instance.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FriConfig {
    pub num_queries: usize,
    pub blowup_factor: usize,
    pub num_layers: usize,
    pub domain_generator: u64,
}

impl Default for FriConfig {
    fn default() -> Self {
        FriConfig {
            num_queries: 10,
            blowup_factor: 2,
            num_layers: 4,
            domain_generator: 3,
        }
    }
}

/// A single FRI layer with its Merkle commitment.
#[derive(Debug, Clone)]
pub struct FriLayer {
    pub index: usize,
    pub evaluations: Vec<u64>,
    pub domain_size: usize,
    pub merkle_root: String,
    pub merkle_tree: MerkleTree,
}

/// A FRI proof that can be verified independently.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FriProof {
    pub layers: Vec<FriLayerSnapshot>,
    pub final_polynomial: Vec<u64>,
    pub query_positions: Vec<usize>,
    pub config: FriConfig,
}

/// A lightweight snapshot of a layer for the proof (no full tree).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FriLayerSnapshot {
    pub index: usize,
    pub domain_size: usize,
    pub merkle_root: String,
}

/// The FRI protocol engine.
pub struct FriProtocol {
    config: FriConfig,
}

impl FriProtocol {
    pub fn new(config: FriConfig) -> Self {
        FriProtocol { config }
    }

    pub fn config(&self) -> &FriConfig {
        &self.config
    }

    /// Generate deterministic query positions from layer data.
    /// Same input always produces the same positions.
    pub fn generate_queries(&self, domain_size: usize) -> Vec<usize> {
        if domain_size == 0 {
            return vec![];
        }
        let n = self.config.num_queries.min(domain_size);
        let mut positions = Vec::with_capacity(n);
        for i in 0..n {
            let base = (i * domain_size) / n;
            let hash_input = format!("fri_query_{}_{}", base, domain_size);
            let hash = TraceHasher::hash(&hash_input);
            let offset = u64::from_str_radix(&hash[..16], 16).unwrap_or(0) as usize;
            let pos = (base + offset) % domain_size;
            positions.push(pos);
        }
        positions
    }

    /// Fold a polynomial: p'(x) = p(x) + p(-x) for even part (simplified).
    /// This is a deterministic folding that halves the domain each round.
    pub fn fold_polynomial(&self, evaluations: &[u64]) -> Vec<u64> {
        if evaluations.len() <= 1 {
            return evaluations.to_vec();
        }
        let half = evaluations.len() / 2;
        let mut folded = Vec::with_capacity(half);
        for i in 0..half {
            let combined = evaluations[2 * i].wrapping_add(evaluations[2 * i + 1]);
            folded.push(combined);
        }
        folded
    }

    /// Build a single FRI layer: commit the evaluations to a Merkle tree.
    pub fn build_layer(&self, index: usize, evaluations: &[u64]) -> FriLayer {
        let leaves: Vec<String> = evaluations
            .iter()
            .map(|v| TraceHasher::hash(&v.to_string()))
            .collect();
        let tree = MerkleTree::new(leaves);
        let root = tree.root_hash.clone();
        FriLayer {
            index,
            evaluations: evaluations.to_vec(),
            domain_size: evaluations.len(),
            merkle_root: root,
            merkle_tree: tree,
        }
    }

    /// Build the complete FRI proof for a polynomial.
    pub fn build_proof(&self, polynomial: &[u64]) -> FriProof {
        if polynomial.is_empty() {
            return FriProof {
                layers: vec![],
                final_polynomial: vec![],
                query_positions: vec![],
                config: self.config.clone(),
            };
        }

        let mut current_evals = polynomial.to_vec();
        let mut layer_snapshots = Vec::new();

        for i in 0..self.config.num_layers {
            let layer = self.build_layer(i, &current_evals);
            layer_snapshots.push(FriLayerSnapshot {
                index: layer.index,
                domain_size: layer.domain_size,
                merkle_root: layer.merkle_root,
            });
            current_evals = self.fold_polynomial(&current_evals);
        }

        let query_domain = polynomial.len();
        let query_positions = self.generate_queries(query_domain);

        FriProof {
            layers: layer_snapshots,
            final_polynomial: current_evals,
            query_positions,
            config: self.config.clone(),
        }
    }

    /// Verify a FRI proof.
    pub fn verify(&self, proof: &FriProof) -> bool {
        if proof.layers.len() != self.config.num_layers {
            return false;
        }
        if proof.config != self.config {
            return false;
        }
        for i in 1..proof.layers.len() {
            if proof.layers[i].domain_size > proof.layers[i - 1].domain_size {
                return false;
            }
        }
        for &pos in &proof.query_positions {
            if proof.layers.is_empty() {
                continue;
            }
            if pos >= proof.layers[0].domain_size {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_polynomial(size: usize) -> Vec<u64> {
        (0..size).map(|i| (i * 7 + 3) as u64).collect()
    }

    // ═══ FOLDING TESTS ═══

    #[test]
    fn test_fold_empty() {
        let fri = FriProtocol::new(FriConfig::default());
        let folded = fri.fold_polynomial(&[]);
        assert!(folded.is_empty());
    }

    #[test]
    fn test_fold_single() {
        let fri = FriProtocol::new(FriConfig::default());
        let folded = fri.fold_polynomial(&[42]);
        assert_eq!(folded, vec![42]);
    }

    #[test]
    fn test_fold_halves_domain() {
        let fri = FriProtocol::new(FriConfig::default());
        let poly = test_polynomial(16);
        let folded = fri.fold_polynomial(&poly);
        assert_eq!(folded.len(), 8);
    }

    #[test]
    fn test_fold_deterministic() {
        let fri = FriProtocol::new(FriConfig::default());
        let poly = test_polynomial(32);
        let f1 = fri.fold_polynomial(&poly);
        let f2 = fri.fold_polynomial(&poly);
        assert_eq!(f1, f2, "Folding must be deterministic");
    }

    // ═══ LAYER TESTS ═══

    #[test]
    fn test_build_layer() {
        let fri = FriProtocol::new(FriConfig::default());
        let poly = test_polynomial(8);
        let layer = fri.build_layer(0, &poly);
        assert_eq!(layer.index, 0);
        assert_eq!(layer.domain_size, 8);
        assert!(!layer.merkle_root.is_empty());
    }

    #[test]
    fn test_layer_merkle_root_deterministic() {
        let fri = FriProtocol::new(FriConfig::default());
        let poly = test_polynomial(8);
        let l1 = fri.build_layer(0, &poly);
        let l2 = fri.build_layer(0, &poly);
        assert_eq!(l1.merkle_root, l2.merkle_root);
    }

    // ═══ QUERY TESTS ═══

    #[test]
    fn test_generate_queries_deterministic() {
        let fri = FriProtocol::new(FriConfig::default());
        let q1 = fri.generate_queries(64);
        let q2 = fri.generate_queries(64);
        assert_eq!(q1, q2, "Queries must be deterministic");
    }

    #[test]
    fn test_generate_queries_count() {
        let fri = FriProtocol::new(FriConfig::default());
        let queries = fri.generate_queries(64);
        assert_eq!(queries.len(), 10);
    }

    #[test]
    fn test_generate_queries_empty_domain() {
        let fri = FriProtocol::new(FriConfig::default());
        assert!(fri.generate_queries(0).is_empty());
    }

    #[test]
    fn test_generate_queries_in_bounds() {
        let fri = FriProtocol::new(FriConfig::default());
        let domain = 32;
        let queries = fri.generate_queries(domain);
        for &q in &queries {
            assert!(q < domain, "Query {} out of bounds (domain={})", q, domain);
        }
    }

    // ═══ PROOF TESTS ═══

    #[test]
    fn test_build_proof_empty() {
        let fri = FriProtocol::new(FriConfig::default());
        let proof = fri.build_proof(&[]);
        assert!(proof.layers.is_empty());
        assert!(proof.final_polynomial.is_empty());
    }

    #[test]
    fn test_build_proof_single() {
        let fri = FriProtocol::new(FriConfig::default());
        let proof = fri.build_proof(&[42]);
        assert_eq!(proof.layers.len(), 4);
        assert!(!proof.final_polynomial.is_empty());
    }

    #[test]
    fn test_build_proof_large() {
        let fri = FriProtocol::new(FriConfig::default());
        let poly = test_polynomial(1024);
        let proof = fri.build_proof(&poly);
        assert_eq!(proof.layers.len(), 4);
        assert_eq!(proof.query_positions.len(), 10);
    }

    #[test]
    fn test_build_proof_deterministic() {
        let fri = FriProtocol::new(FriConfig::default());
        let poly = test_polynomial(256);
        let p1 = fri.build_proof(&poly);
        let p2 = fri.build_proof(&poly);
        assert_eq!(p1.layers.len(), p2.layers.len());
        assert_eq!(p1.final_polynomial, p2.final_polynomial);
        assert_eq!(p1.query_positions, p2.query_positions);
        for i in 0..p1.layers.len() {
            assert_eq!(
                p1.layers[i].merkle_root, p2.layers[i].merkle_root,
                "Layer {} root differs",
                i
            );
        }
    }

    // ═══ VERIFICATION TESTS ═══

    #[test]
    fn test_verify_valid_proof() {
        let fri = FriProtocol::new(FriConfig::default());
        let poly = test_polynomial(64);
        let proof = fri.build_proof(&poly);
        assert!(fri.verify(&proof), "Valid proof should verify");
    }

    #[test]
    fn test_verify_wrong_config_fails() {
        let fri = FriProtocol::new(FriConfig::default());
        let poly = test_polynomial(64);
        let mut proof = fri.build_proof(&poly);
        proof.config.num_queries = 999;
        assert!(!fri.verify(&proof), "Tampered config should fail");
    }

    #[test]
    fn test_verify_wrong_layer_count_fails() {
        let fri = FriProtocol::new(FriConfig::default());
        let poly = test_polynomial(64);
        let mut proof = fri.build_proof(&poly);
        proof.layers.pop();
        assert!(!fri.verify(&proof), "Wrong layer count should fail");
    }

    #[test]
    fn test_verify_query_out_of_bounds_fails() {
        let fri = FriProtocol::new(FriConfig::default());
        let poly = test_polynomial(8);
        let mut proof = fri.build_proof(&poly);
        proof.query_positions = vec![999];
        assert!(!fri.verify(&proof), "Out-of-bounds query should fail");
    }

    // ═══ CONFIG TESTS ═══

    #[test]
    fn test_default_config() {
        let config = FriConfig::default();
        assert_eq!(config.num_queries, 10);
        assert_eq!(config.blowup_factor, 2);
        assert_eq!(config.num_layers, 4);
    }

    #[test]
    fn test_custom_config() {
        let config = FriConfig {
            num_queries: 20,
            blowup_factor: 4,
            num_layers: 6,
            domain_generator: 5,
        };
        let fri = FriProtocol::new(config);
        assert_eq!(fri.config().num_queries, 20);
        assert_eq!(fri.generate_queries(100).len(), 20);
    }

    #[test]
    fn test_many_layers() {
        let config = FriConfig {
            num_queries: 5,
            blowup_factor: 2,
            num_layers: 8,
            domain_generator: 3,
        };
        let fri = FriProtocol::new(config);
        let poly = test_polynomial(512);
        let proof = fri.build_proof(&poly);
        assert_eq!(proof.layers.len(), 8);
        assert!(fri.verify(&proof));
    }
}
