//! Blockchain Manager — Maintains the canonical chain of Compute Blocks.
//!
//! Deterministic, re-validatable chain of ComputeBlock entries.

use crate::blockchain::block_builder::{BlockBuilder, ComputeBlock};
use sha2::{Digest, Sha256};

/// The canonical blockchain of Compute Blocks.
pub struct ComputeChain {
    chain: Vec<ComputeBlock>,
}

impl ComputeChain {
    /// Create a new chain with only the Genesis block.
    pub fn new() -> Self {
        ComputeChain {
            chain: vec![Self::genesis()],
        }
    }

    /// Create the deterministic Genesis block (block 0).
    pub fn genesis() -> ComputeBlock {
        let builder = BlockBuilder::new("genesis");
        builder.build_block(
            0,
            "0",
            0,
            "compute-chain-genesis-seed-v1",
            vec![],
            vec![],
            0,
        )
    }

    /// Get the current chain height (number of blocks - 1).
    pub fn height(&self) -> u64 {
        (self.chain.len() as u64).saturating_sub(1)
    }

    /// Get a reference to the latest block.
    pub fn latest_block(&self) -> &ComputeBlock {
        self.chain.last().expect("Chain must have at least genesis")
    }

    /// Get the hash of the latest block.
    pub fn latest_hash(&self) -> String {
        self.latest_block().block_hash.clone()
    }

    /// Get a block by height.
    pub fn find_block(&self, height: u64) -> Option<&ComputeBlock> {
        self.chain.get(height as usize)
    }

    /// Check if a block exists at the given height.
    pub fn block_exists(&self, height: u64) -> bool {
        height <= self.height()
    }

    /// Get the total number of blocks.
    pub fn len(&self) -> usize {
        self.chain.len()
    }

    /// Check if the chain is empty (should never be — always has genesis).
    pub fn is_empty(&self) -> bool {
        self.chain.is_empty()
    }

    // ═══ APPEND ═══

    /// Append a block to the chain if valid.
    pub fn append_block(&mut self, block: ComputeBlock) -> Result<(), String> {
        self.validate_block(&block)?;
        self.chain.push(block);
        Ok(())
    }

    // ═══ VALIDATION ═══

    /// Validate a single block against the current chain state.
    pub fn validate_block(&self, block: &ComputeBlock) -> Result<(), String> {
        let latest = self.latest_block();

        // Rule 1: Previous hash must match latest block
        if block.header.previous_hash != latest.block_hash {
            return Err(format!(
                "Invalid previous_hash: expected {}, got {}",
                latest.block_hash, block.header.previous_hash
            ));
        }

        // Rule 2: Block height must increment by 1
        if block.header.block_height != latest.header.block_height + 1 {
            return Err(format!(
                "Invalid height: expected {}, got {}",
                latest.header.block_height + 1,
                block.header.block_height
            ));
        }

        // Rule 3: Block hash must be correct
        let computed_hash = BlockBuilder::calculate_block_hash(&block.header);
        if computed_hash != block.block_hash {
            return Err(format!(
                "Invalid block_hash: expected {}, got {}",
                computed_hash, block.block_hash
            ));
        }

        // Rule 4: Merkle root for jobs must match
        let jobs_root = BlockBuilder::build_merkle_root_jobs(&block.accepted_jobs);
        if jobs_root != block.header.merkle_root_jobs {
            return Err("Invalid merkle_root_jobs".into());
        }

        // Rule 5: Merkle root for proofs must match
        let proofs_root = BlockBuilder::build_merkle_root_proofs(&block.accepted_proofs);
        if proofs_root != block.header.merkle_root_proofs {
            return Err("Invalid merkle_root_proofs".into());
        }

        Ok(())
    }

    /// Validate the entire chain from Genesis to tip.
    pub fn validate_chain(&self) -> Result<(), String> {
        if self.chain.is_empty() {
            return Err("Empty chain".into());
        }

        // Validate genesis
        let genesis = &self.chain[0];
        let expected_genesis = Self::genesis();
        if genesis.block_hash != expected_genesis.block_hash {
            return Err("Genesis block is invalid".into());
        }

        // Validate each subsequent block
        for i in 1..self.chain.len() {
            let prev = &self.chain[i - 1];
            let current = &self.chain[i];

            if current.header.previous_hash != prev.block_hash {
                return Err(format!(
                    "Chain broken at height {}",
                    current.header.block_height
                ));
            }
            if current.header.block_height != prev.header.block_height + 1 {
                return Err(format!("Height gap at {}", current.header.block_height));
            }
            let computed = BlockBuilder::calculate_block_hash(&current.header);
            if computed != current.block_hash {
                return Err(format!(
                    "Invalid hash at height {}",
                    current.header.block_height
                ));
            }
        }

        Ok(())
    }

    // ═══ ROLLBACK ═══

    /// Rollback the chain to a specific height.
    /// Returns the removed blocks.
    pub fn rollback(&mut self, target_height: u64) -> Result<Vec<ComputeBlock>, String> {
        if target_height > self.height() {
            return Err(format!(
                "Cannot rollback to height {} (current: {})",
                target_height,
                self.height()
            ));
        }

        let keep = (target_height + 1) as usize;
        let removed: Vec<ComputeBlock> = self.chain.drain(keep..).collect();
        Ok(removed)
    }

    // ═══ FORK DETECTION ═══

    /// Find the fork point between this chain and another.
    /// Returns the height of the last common block.
    pub fn fork_point(&self, other: &ComputeChain) -> u64 {
        let min_len = self.chain.len().min(other.chain.len());
        for i in 0..min_len {
            if self.chain[i].block_hash != other.chain[i].block_hash {
                return i.saturating_sub(1) as u64;
            }
        }
        (min_len.saturating_sub(1)) as u64
    }

    /// Get all blocks in the chain (clone).
    pub fn get_all_blocks(&self) -> Vec<ComputeBlock> {
        self.chain.clone()
    }

    /// Get a range of blocks.
    pub fn get_blocks_range(&self, from: u64, to: u64) -> Vec<&ComputeBlock> {
        let from = from as usize;
        let to = (to as usize).min(self.chain.len());
        if from >= self.chain.len() {
            return vec![];
        }
        self.chain[from..to].iter().collect()
    }
}

impl Default for ComputeChain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::block_builder::BlockBuilder;

    fn make_test_block(builder: &BlockBuilder, height: u64, prev_hash: &str) -> ComputeBlock {
        builder.build_block(
            height,
            prev_hash,
            height,
            &format!("seed_{}", height),
            vec![],
            vec![],
            1700000000 + height,
        )
    }

    #[test]
    fn test_genesis_chain() {
        let chain = ComputeChain::new();
        assert_eq!(chain.height(), 0);
        assert!(!chain.latest_hash().is_empty());
        assert!(chain.block_exists(0));
        assert!(!chain.block_exists(1));
    }

    #[test]
    fn test_append_block() {
        let mut chain = ComputeChain::new();
        let builder = BlockBuilder::new("test");
        let block = make_test_block(&builder, 1, &chain.latest_hash());

        chain.append_block(block).unwrap();
        assert_eq!(chain.height(), 1);
        assert!(chain.block_exists(1));
    }

    #[test]
    fn test_invalid_previous_hash() {
        let mut chain = ComputeChain::new();
        let builder = BlockBuilder::new("test");
        let block = make_test_block(&builder, 1, "wrong_hash");

        assert!(chain.append_block(block).is_err());
    }

    #[test]
    fn test_invalid_height() {
        let mut chain = ComputeChain::new();
        let builder = BlockBuilder::new("test");
        // Height 5 when we expect 1
        let block = builder.build_block(
            5,
            &chain.latest_hash(),
            5,
            "seed",
            vec![],
            vec![],
            1700000000,
        );

        assert!(chain.append_block(block).is_err());
    }

    #[test]
    fn test_invalid_block_hash() {
        let mut chain = ComputeChain::new();
        let builder = BlockBuilder::new("test");
        let mut block = make_test_block(&builder, 1, &chain.latest_hash());
        block.block_hash = "tampered".into();

        assert!(chain.append_block(block).is_err());
    }

    #[test]
    fn test_validate_entire_chain() {
        let mut chain = ComputeChain::new();
        let builder = BlockBuilder::new("test");

        for i in 1..=5 {
            let block = make_test_block(&builder, i, &chain.latest_hash());
            chain.append_block(block).unwrap();
        }

        assert_eq!(chain.height(), 5);
        chain.validate_chain().unwrap();
    }

    #[test]
    fn test_find_block() {
        let mut chain = ComputeChain::new();
        let builder = BlockBuilder::new("test");

        for i in 1..=3 {
            chain
                .append_block(make_test_block(&builder, i, &chain.latest_hash()))
                .unwrap();
        }

        assert!(chain.find_block(0).is_some());
        assert!(chain.find_block(2).is_some());
        assert!(chain.find_block(99).is_none());
    }

    #[test]
    fn test_rollback() {
        let mut chain = ComputeChain::new();
        let builder = BlockBuilder::new("test");

        for i in 1..=5 {
            chain
                .append_block(make_test_block(&builder, i, &chain.latest_hash()))
                .unwrap();
        }
        assert_eq!(chain.height(), 5);

        let removed = chain.rollback(2).unwrap();
        assert_eq!(chain.height(), 2);
        assert_eq!(removed.len(), 3); // Blocks 3, 4, 5
    }

    #[test]
    fn test_fork_detection() {
        let mut chain_a = ComputeChain::new();
        let mut chain_b = ComputeChain::new();
        let builder = BlockBuilder::new("test");

        // Both get blocks 1 and 2
        for i in 1..=2 {
            let block = make_test_block(&builder, i, &chain_a.latest_hash());
            chain_a.append_block(block.clone()).unwrap();
            chain_b.append_block(block).unwrap();
        }

        // Chain A gets block 3a, chain B gets block 3b (divergent)
        let block_3a = make_test_block(&builder, 3, &chain_a.latest_hash());
        chain_a.append_block(block_3a).unwrap();

        let block_3b = builder.build_block(
            3,
            &chain_b.latest_hash(),
            3,
            "seed_3b",
            vec![],
            vec![],
            1700000003,
        );
        chain_b.append_block(block_3b).unwrap();

        let fork = chain_a.fork_point(&chain_b);
        assert_eq!(fork, 2);
    }

    #[test]
    fn test_large_chain() {
        let mut chain = ComputeChain::new();
        let builder = BlockBuilder::new("test");

        for i in 1..=100 {
            let block = make_test_block(&builder, i, &chain.latest_hash());
            chain.append_block(block).unwrap();
        }

        assert_eq!(chain.height(), 100);
        chain.validate_chain().unwrap();
        assert!(chain.block_exists(50));
        assert!(chain.block_exists(100));
    }
}
