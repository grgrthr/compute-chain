//! Network Seed — Deterministic seed shared by all nodes.
//!
//! Every node computes the same seed from round metadata.
//! No seed is transmitted — only metadata crosses the network.

use sha2::{Digest, Sha256};

/// Metadata published by the scheduler for each round.
#[derive(Debug, Clone, PartialEq)]
pub struct RoundMetadata {
    /// Previous seed (or genesis seed for round 0).
    pub previous_seed: String,
    /// Hash of the previous block.
    pub previous_block_hash: String,
    /// Current round number.
    pub current_round: u64,
    /// Hash of active worker set (sorted, comma-separated peer IDs).
    pub active_workers_hash: String,
    /// Difficulty level for this round.
    pub difficulty: u32,
}

/// The Network Seed — derived deterministically from RoundMetadata.
#[derive(Debug, Clone, PartialEq)]
pub struct NetworkSeed {
    /// The computed seed hash (hex string).
    pub seed: String,
    /// The round metadata used to derive this seed.
    pub metadata: RoundMetadata,
    /// The raw SHA-256 hash bytes.
    pub hash_bytes: Vec<u8>,
}

impl NetworkSeed {
    /// Genesis seed — used for round 0 before any blocks exist.
    pub const GENESIS: &str = "compute-chain-genesis-seed-v1";

    /// Compute the network seed from round metadata.
    /// Same metadata always produces the same seed.
    pub fn compute(metadata: &RoundMetadata) -> Self {
        let mut hasher = Sha256::new();

        // Concatenate all metadata fields in deterministic order
        hasher.update(metadata.previous_seed.as_bytes());
        hasher.update(b"||");
        hasher.update(metadata.previous_block_hash.as_bytes());
        hasher.update(b"||");
        hasher.update(metadata.current_round.to_le_bytes());
        hasher.update(b"||");
        hasher.update(metadata.active_workers_hash.as_bytes());
        hasher.update(b"||");
        hasher.update(metadata.difficulty.to_le_bytes());

        let hash_bytes = hasher.finalize().to_vec();
        let seed = hex::encode(&hash_bytes);

        NetworkSeed {
            seed,
            metadata: metadata.clone(),
            hash_bytes,
        }
    }

    /// Compute seed for the genesis round.
    pub fn genesis() -> Self {
        Self::compute(&RoundMetadata {
            previous_seed: Self::GENESIS.into(),
            previous_block_hash: String::new(),
            current_round: 0,
            active_workers_hash: String::new(),
            difficulty: 1,
        })
    }

    /// Compute the next round's seed from current state.
    pub fn next_round(
        previous_seed: &str,
        previous_block_hash: &str,
        current_round: u64,
        active_workers: &[String],
        difficulty: u32,
    ) -> Self {
        let workers_hash = Self::hash_worker_set(active_workers);
        Self::compute(&RoundMetadata {
            previous_seed: previous_seed.into(),
            previous_block_hash: previous_block_hash.into(),
            current_round,
            active_workers_hash: workers_hash,
            difficulty,
        })
    }

    /// Hash a set of worker IDs deterministically.
    pub fn hash_worker_set(workers: &[String]) -> String {
        let mut sorted: Vec<String> = workers.to_vec();
        sorted.sort();
        let joined = sorted.join(",");
        let mut hasher = Sha256::new();
        hasher.update(joined.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Convert this seed into a WorkloadConfig for the WorkloadGenerator.
    pub fn to_workload_config(
        &self,
        block_height: u64,
    ) -> crate::workload::generator::WorkloadConfig {
        crate::workload::generator::WorkloadConfig {
            network_seed: self.seed.clone(),
            block_height,
            difficulty: self.metadata.difficulty,
        }
    }
}

impl std::fmt::Display for NetworkSeed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.seed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_metadata() -> RoundMetadata {
        RoundMetadata {
            previous_seed: "prev-seed-abc".into(),
            previous_block_hash: "block-hash-xyz".into(),
            current_round: 42,
            active_workers_hash: NetworkSeed::hash_worker_set(&[
                "worker-a".into(),
                "worker-b".into(),
                "worker-c".into(),
            ]),
            difficulty: 5,
        }
    }

    // ═══ SAME METADATA → SAME SEED ═══

    #[test]
    fn test_same_metadata_same_seed() {
        let m = test_metadata();
        let s1 = NetworkSeed::compute(&m);
        let s2 = NetworkSeed::compute(&m);
        assert_eq!(s1.seed, s2.seed);
        assert_eq!(s1.hash_bytes, s2.hash_bytes);
    }

    // ═══ DIFFERENT ROUND → DIFFERENT SEED ═══

    #[test]
    fn test_different_round_different_seed() {
        let mut m = test_metadata();
        let s1 = NetworkSeed::compute(&m);
        m.current_round = 100;
        let s2 = NetworkSeed::compute(&m);
        assert_ne!(s1.seed, s2.seed);
    }

    // ═══ DIFFERENT WORKER SET → DIFFERENT SEED ═══

    #[test]
    fn test_different_workers_different_seed() {
        let mut m = test_metadata();
        let s1 = NetworkSeed::compute(&m);
        m.active_workers_hash = NetworkSeed::hash_worker_set(&["only-one".into()]);
        let s2 = NetworkSeed::compute(&m);
        assert_ne!(s1.seed, s2.seed);
    }

    // ═══ DIFFERENT DIFFICULTY → DIFFERENT SEED ═══

    #[test]
    fn test_different_difficulty_different_seed() {
        let mut m = test_metadata();
        let s1 = NetworkSeed::compute(&m);
        m.difficulty = 10;
        let s2 = NetworkSeed::compute(&m);
        assert_ne!(s1.seed, s2.seed);
    }

    // ═══ GENESIS SEED ═══

    #[test]
    fn test_genesis_seed() {
        let g1 = NetworkSeed::genesis();
        let g2 = NetworkSeed::genesis();
        assert_eq!(g1.seed, g2.seed);
        assert_eq!(g1.metadata.current_round, 0);
    }

    // ═══ WORKER HASH DETERMINISTIC ═══

    #[test]
    fn test_worker_hash_deterministic() {
        let workers = vec!["c".into(), "a".into(), "b".into()];
        let h1 = NetworkSeed::hash_worker_set(&workers);
        let h2 = NetworkSeed::hash_worker_set(&workers);
        assert_eq!(h1, h2);

        // Order doesn't matter (sorted internally)
        let workers2 = vec!["a".into(), "b".into(), "c".into()];
        assert_eq!(h1, NetworkSeed::hash_worker_set(&workers2));
    }

    // ═══ NEXT ROUND ═══

    #[test]
    fn test_next_round() {
        let s1 = NetworkSeed::genesis();
        let s2 =
            NetworkSeed::next_round(&s1.seed, "block-1-hash", 1, &["w1".into(), "w2".into()], 3);
        assert_ne!(s1.seed, s2.seed);
        assert_eq!(s2.metadata.current_round, 1);
        assert_eq!(s2.metadata.difficulty, 3);
    }

    // ═══ CONVERT TO WORKLOAD CONFIG ═══

    #[test]
    fn test_to_workload_config() {
        let seed = NetworkSeed::compute(&test_metadata());
        let config = seed.to_workload_config(42);
        assert_eq!(config.network_seed, seed.seed);
        assert_eq!(config.block_height, 42);
        assert_eq!(config.difficulty, 5);
    }

    // ═══ THREE NODES DERIVE IDENTICAL WORKLOADS ═══

    #[test]
    fn test_three_nodes_derive_identical_workloads() {
        let metadata = test_metadata();

        // All three "nodes" compute the same seed
        let seed1 = NetworkSeed::compute(&metadata);
        let seed2 = NetworkSeed::compute(&metadata);
        let seed3 = NetworkSeed::compute(&metadata);

        assert_eq!(seed1.seed, seed2.seed);
        assert_eq!(seed2.seed, seed3.seed);

        // All three generate the same workload
        let config = seed1.to_workload_config(42);
        let wl1 = crate::workload::generator::WorkloadGenerator::generate(&config);
        let wl2 = crate::workload::generator::WorkloadGenerator::generate(&config);
        let wl3 = crate::workload::generator::WorkloadGenerator::generate(&config);

        assert_eq!(wl1.seed_hash, wl2.seed_hash);
        assert_eq!(wl2.seed_hash, wl3.seed_hash);
        assert_eq!(wl1.instruction_count, wl2.instruction_count);
        assert_eq!(wl2.instruction_count, wl3.instruction_count);
    }

    // ═══ GENERATED PROOFS REMAIN VALID ═══

    #[test]
    fn test_seed_generated_proof_valid() {
        let seed = NetworkSeed::compute(&test_metadata());
        let config = seed.to_workload_config(42);
        let wl = crate::workload::generator::WorkloadGenerator::generate(&config);

        let mut runner = crate::integration::runner::IntegrationRunner::new();
        let result = runner.run("seed_proof_test", wl.instructions);

        assert!(
            result.success,
            "Proof from seed-generated workload must succeed: {:?}",
            result.error
        );
        assert!(result.proof_generated);
        assert!(result.proof_verified);
    }
}
