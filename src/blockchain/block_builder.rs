//! Block Builder — Collects completed jobs from ComputePool into a Block.
//!
//! Deterministic: same inputs always produce the same block.

use crate::compute_pool::{ComputePool, PoolJob, PoolJobStatus};
use crate::merkle::hash::TraceHasher;
use crate::merkle::tree::MerkleTree;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Header of a compute block.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlockHeader {
    pub block_height: u64,
    pub previous_hash: String,
    pub round: u64,
    pub timestamp: u64,
    pub workload_seed: String,
    pub merkle_root_jobs: String,
    pub merkle_root_proofs: String,
    pub producer_id: String,
}

/// A block produced by the BlockBuilder.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComputeBlock {
    pub header: BlockHeader,
    pub accepted_jobs: Vec<PoolJob>,
    pub accepted_proofs: Vec<String>,
    pub block_hash: String,
}

/// Builds blocks from completed ComputePool jobs.
pub struct BlockBuilder {
    producer_id: String,
}

impl BlockBuilder {
    pub fn new(producer_id: &str) -> Self {
        BlockBuilder {
            producer_id: producer_id.to_string(),
        }
    }

    /// Build Merkle root from job IDs (deterministic).
    pub fn build_merkle_root_jobs(jobs: &[PoolJob]) -> String {
        if jobs.is_empty() {
            return TraceHasher::hash("empty_jobs");
        }
        let mut ids: Vec<String> = jobs.iter().map(|j| j.id.clone()).collect();
        ids.sort(); // Deterministic ordering
        let leaves: Vec<String> = ids.iter().map(|id| TraceHasher::hash(id)).collect();
        MerkleTree::new(leaves).root_hash
    }

    /// Build Merkle root from proof hashes (deterministic).
    pub fn build_merkle_root_proofs(proofs: &[String]) -> String {
        if proofs.is_empty() {
            return TraceHasher::hash("empty_proofs");
        }
        let mut sorted: Vec<String> = proofs.to_vec();
        sorted.sort();
        let leaves: Vec<String> = sorted.iter().map(|p| TraceHasher::hash(p)).collect();
        MerkleTree::new(leaves).root_hash
    }

    /// Calculate block hash from header.
    pub fn calculate_block_hash(header: &BlockHeader) -> String {
        let mut hasher = Sha256::new();
        hasher.update(header.block_height.to_le_bytes());
        hasher.update(header.previous_hash.as_bytes());
        hasher.update(header.round.to_le_bytes());
        hasher.update(header.timestamp.to_le_bytes());
        hasher.update(header.workload_seed.as_bytes());
        hasher.update(header.merkle_root_jobs.as_bytes());
        hasher.update(header.merkle_root_proofs.as_bytes());
        hasher.update(header.producer_id.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Build a complete block from completed jobs and proofs.
    pub fn build_block(
        &self,
        block_height: u64,
        previous_hash: &str,
        round: u64,
        workload_seed: &str,
        jobs: Vec<PoolJob>,
        proofs: Vec<String>,
        timestamp: u64,
    ) -> ComputeBlock {
        let merkle_root_jobs = Self::build_merkle_root_jobs(&jobs);
        let merkle_root_proofs = Self::build_merkle_root_proofs(&proofs);

        let header = BlockHeader {
            block_height,
            previous_hash: previous_hash.to_string(),
            round,
            timestamp,
            workload_seed: workload_seed.to_string(),
            merkle_root_jobs,
            merkle_root_proofs,
            producer_id: self.producer_id.clone(),
        };

        let block_hash = Self::calculate_block_hash(&header);

        ComputeBlock {
            header,
            accepted_jobs: jobs,
            accepted_proofs: proofs,
            block_hash,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p::proof_network::{sample_compute_job, WorkerNode};

    fn create_completed_jobs(count: usize) -> Vec<PoolJob> {
        let worker = WorkerNode::new("test_worker");
        let mut jobs = Vec::new();
        for i in 0..count {
            let compute_job = sample_compute_job();
            let result = worker.execute_job(&compute_job);
            let job = PoolJob {
                id: format!("job_{}", i),
                compute_job,
                status: PoolJobStatus::Completed,
                assigned_worker: Some("test_worker".into()),
                submitted_at: 1700000000,
                started_at: Some(1700000001),
                completed_at: Some(1700000002),
                result_hash: Some(result.trace_root),
                retry_count: 0,
                max_retries: 3,
            };
            jobs.push(job);
        }
        jobs
    }

    #[test]
    fn test_empty_block() {
        let builder = BlockBuilder::new("producer_1");
        let block = builder.build_block(0, "genesis", 0, "seed_0", vec![], vec![], 1700000000);

        assert_eq!(block.header.block_height, 0);
        assert_eq!(block.header.previous_hash, "genesis");
        assert!(!block.block_hash.is_empty());
        assert!(block.accepted_jobs.is_empty());
    }

    #[test]
    fn test_single_job_block() {
        let builder = BlockBuilder::new("producer_1");
        let jobs = create_completed_jobs(1);
        let proofs = vec!["proof_hash_1".into()];

        let block = builder.build_block(
            1,
            "prev_hash",
            1,
            "seed_1",
            jobs.clone(),
            proofs,
            1700000000,
        );

        assert_eq!(block.accepted_jobs.len(), 1);
        assert_eq!(block.accepted_proofs.len(), 1);
        assert!(!block.header.merkle_root_jobs.is_empty());
        assert!(!block.header.merkle_root_proofs.is_empty());
        assert!(!block.block_hash.is_empty());
    }

    #[test]
    fn test_multiple_jobs_block() {
        let builder = BlockBuilder::new("producer_1");
        let jobs = create_completed_jobs(5);
        let proofs: Vec<String> = (0..5).map(|i| format!("proof_{}", i)).collect();

        let block = builder.build_block(2, "prev", 2, "seed_2", jobs, proofs, 1700000000);

        assert_eq!(block.accepted_jobs.len(), 5);
        assert_eq!(block.accepted_proofs.len(), 5);
        assert!(!block.block_hash.is_empty());
    }

    #[test]
    fn test_same_inputs_same_block() {
        let builder = BlockBuilder::new("producer_1");
        let jobs = create_completed_jobs(3);
        let proofs: Vec<String> = (0..3).map(|i| format!("proof_{}", i)).collect();

        let block1 = builder.build_block(
            1,
            "prev",
            1,
            "seed",
            jobs.clone(),
            proofs.clone(),
            1700000000,
        );
        let block2 = builder.build_block(
            1,
            "prev",
            1,
            "seed",
            jobs.clone(),
            proofs.clone(),
            1700000000,
        );

        assert_eq!(block1.block_hash, block2.block_hash);
        assert_eq!(
            block1.header.merkle_root_jobs,
            block2.header.merkle_root_jobs
        );
        assert_eq!(
            block1.header.merkle_root_proofs,
            block2.header.merkle_root_proofs
        );
    }

    #[test]
    fn test_different_jobs_different_hash() {
        let builder = BlockBuilder::new("producer_1");
        let jobs1 = create_completed_jobs(2);
        let jobs2 = create_completed_jobs(3);
        let proofs = vec!["p1".into()];

        let block1 = builder.build_block(1, "prev", 1, "seed", jobs1, proofs.clone(), 1700000000);
        let block2 = builder.build_block(1, "prev", 1, "seed", jobs2, proofs, 1700000000);

        assert_ne!(block1.block_hash, block2.block_hash);
    }

    #[test]
    fn test_block_hash_deterministic() {
        let builder = BlockBuilder::new("producer_x");
        let jobs = create_completed_jobs(2);
        let proofs = vec!["proof_a".into(), "proof_b".into()];

        let hash1 = builder
            .build_block(5, "ph", 5, "ws", jobs.clone(), proofs.clone(), 99)
            .block_hash;
        let hash2 = builder
            .build_block(5, "ph", 5, "ws", jobs.clone(), proofs.clone(), 99)
            .block_hash;

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_merkle_roots_present() {
        let builder = BlockBuilder::new("p1");
        let jobs = create_completed_jobs(2);
        let proofs = vec!["p1".into(), "p2".into()];

        let block = builder.build_block(1, "prev", 1, "seed", jobs, proofs, 1700000000);

        assert!(!block.header.merkle_root_jobs.is_empty());
        assert!(!block.header.merkle_root_proofs.is_empty());
        assert_ne!(
            block.header.merkle_root_jobs,
            block.header.merkle_root_proofs
        );
    }

    #[test]
    fn test_block_contains_valid_proofs() {
        let worker = WorkerNode::new("test_worker");
        let compute_job = sample_compute_job();
        let result = worker.execute_job(&compute_job);

        let job = PoolJob {
            id: "job_real".into(),
            compute_job,
            status: PoolJobStatus::Completed,
            assigned_worker: Some("test_worker".into()),
            submitted_at: 1700000000,
            started_at: Some(1700000001),
            completed_at: Some(1700000002),
            result_hash: Some(result.trace_root.clone()),
            retry_count: 0,
            max_retries: 3,
        };

        let builder = BlockBuilder::new("producer_1");
        let block = builder.build_block(
            1,
            "prev",
            1,
            "seed_real",
            vec![job],
            vec![result.trace_root],
            1700000000,
        );

        assert_eq!(block.accepted_jobs.len(), 1);
        assert!(!block.accepted_proofs[0].is_empty());
        assert!(!block.block_hash.is_empty());
    }

    #[test]
    fn test_large_block() {
        let builder = BlockBuilder::new("producer_large");
        let jobs = create_completed_jobs(100);
        let proofs: Vec<String> = (0..100).map(|i| format!("proof_{}", i)).collect();

        let block =
            builder.build_block(10, "prev_large", 10, "seed_large", jobs, proofs, 1700000000);

        assert_eq!(block.accepted_jobs.len(), 100);
        assert!(block.block_hash.len() == 64); // SHA-256 hex
        assert!(!block.header.merkle_root_jobs.is_empty());
    }
}
