use crate::blockchain::block_builder::ComputeBlock;
use crate::blockchain::chain::ComputeChain;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub enum SubmissionResult {
    Accepted,
    Duplicate,
    Invalid(String),
    ForkResolved { new_height: u64 },
}

pub struct ConsensusEngine {
    chain: ComputeChain,
    seen_blocks: HashSet<String>,
    pending_blocks: Vec<ComputeBlock>,
}

impl ConsensusEngine {
    pub fn new() -> Self {
        ConsensusEngine {
            chain: ComputeChain::new(),
            seen_blocks: HashSet::new(),
            pending_blocks: Vec::new(),
        }
    }

    pub fn height(&self) -> u64 {
        self.chain.height()
    }
    pub fn canonical_tip(&self) -> &ComputeBlock {
        self.chain.latest_block()
    }
    pub fn chain(&self) -> &ComputeChain {
        &self.chain
    }

    pub fn submit_block(&mut self, block: ComputeBlock) -> SubmissionResult {
        if self.seen_blocks.contains(&block.block_hash) {
            return SubmissionResult::Duplicate;
        }
        self.seen_blocks.insert(block.block_hash.clone());
        match self.chain.append_block(block.clone()) {
            Ok(()) => SubmissionResult::Accepted,
            Err(e) => {
                self.pending_blocks.push(block);
                SubmissionResult::Invalid(e)
            }
        }
    }

    pub fn receive_block(&mut self, block: ComputeBlock) -> SubmissionResult {
        self.submit_block(block)
    }

    pub fn resolve_fork(&mut self) -> bool {
        let mut to_keep = Vec::new();
        let mut found = false;
        let height = self.height();
        let tip_hash = self.chain.latest_hash();

        for i in 0..self.pending_blocks.len() {
            let block = &self.pending_blocks[i];
            if block.header.block_height == height + 1 && block.header.previous_hash == tip_hash {
                if self.chain.append_block(block.clone()).is_ok() {
                    found = true;
                    continue;
                }
            }
            to_keep.push(self.pending_blocks[i].clone());
        }
        self.pending_blocks = to_keep;
        found
    }

    pub fn verify_consensus(&self) -> Result<(), String> {
        self.chain.validate_chain()
    }

    pub fn sync_peer(&mut self, other: &ComputeChain) -> Result<usize, String> {
        let fork = self.chain.fork_point(other);
        let mut added = 0;
        for height in (fork + 1)..=other.height() {
            if let Some(block) = other.find_block(height) {
                if self.chain.height() < height {
                    if block.header.previous_hash == self.chain.latest_hash() {
                        self.chain.append_block(block.clone())?;
                        self.seen_blocks.insert(block.block_hash.clone());
                        added += 1;
                    }
                }
            }
        }
        Ok(added)
    }

    pub fn tiebreak(&self, hash_a: &str, hash_b: &str) -> std::cmp::Ordering {
        hash_a.cmp(hash_b)
    }

    pub fn is_canonical(&self, other: &ComputeChain) -> bool {
        if other.len() > self.chain.len() {
            false
        } else if other.len() < self.chain.len() {
            true
        } else {
            self.chain.latest_hash() <= other.latest_hash()
        }
    }
}

impl Default for ConsensusEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::block_builder::BlockBuilder;

    fn mb(b: &BlockBuilder, h: u64, ph: &str, s: &str) -> ComputeBlock {
        b.build_block(h, ph, h, s, vec![], vec![], 1700000000 + h)
    }

    #[test]
    fn test_accept() {
        let mut e = ConsensusEngine::new();
        let b = BlockBuilder::new("t");
        assert_eq!(
            e.submit_block(mb(&b, 1, &e.chain().latest_hash(), "s")),
            SubmissionResult::Accepted
        );
        assert_eq!(e.height(), 1);
    }
    #[test]
    fn test_reject() {
        let mut e = ConsensusEngine::new();
        let b = BlockBuilder::new("t");
        assert!(matches!(
            e.submit_block(mb(&b, 5, "w", "s")),
            SubmissionResult::Invalid(_)
        ));
    }
    #[test]
    fn test_duplicate() {
        let mut e = ConsensusEngine::new();
        let b = BlockBuilder::new("t");
        let blk = mb(&b, 1, &e.chain().latest_hash(), "s");
        e.submit_block(blk.clone());
        assert_eq!(e.submit_block(blk), SubmissionResult::Duplicate);
    }
    #[test]
    fn test_longest() {
        let mut a = ConsensusEngine::new();
        let b = BlockBuilder::new("t");
        for i in 1..=3 {
            a.submit_block(mb(&b, i, &a.chain().latest_hash(), &format!("s{}", i)));
        }
        assert_eq!(a.height(), 3);
        let c = ConsensusEngine::new();
        assert!(a.is_canonical(c.chain()));
    }
    #[test]
    fn test_tiebreak() {
        let e = ConsensusEngine::new();
        assert!(e.tiebreak("aaa", "bbb") == std::cmp::Ordering::Less);
    }
    #[test]
    fn test_sync() {
        let mut a = ConsensusEngine::new();
        let mut c = ConsensusEngine::new();
        let b = BlockBuilder::new("t");
        for i in 1..=3 {
            a.submit_block(mb(&b, i, &a.chain().latest_hash(), &format!("s{}", i)));
        }
        assert_eq!(c.sync_peer(a.chain()).unwrap(), 3);
        assert_eq!(c.height(), 3);
    }
    #[test]
    fn test_three_nodes() {
        let mut na = ConsensusEngine::new();
        let mut nb = ConsensusEngine::new();
        let mut nc = ConsensusEngine::new();
        let b = BlockBuilder::new("t");
        for i in 1..=4 {
            let blk = mb(&b, i, &na.chain().latest_hash(), &format!("s{}", i));
            na.submit_block(blk.clone());
            nb.submit_block(blk.clone());
            nc.submit_block(blk);
        }
        assert_eq!(na.height(), 4);
        assert_eq!(na.chain().latest_hash(), nb.chain().latest_hash());
        assert_eq!(nb.chain().latest_hash(), nc.chain().latest_hash());
    }
    #[test]
    fn test_fork() {
        let mut e = ConsensusEngine::new();
        let b = BlockBuilder::new("t");
        for i in 1..=2 {
            e.submit_block(mb(&b, i, &e.chain().latest_hash(), &format!("s{}", i)));
        }
        e.submit_block(mb(&b, 1, "diff", "fs"));
        e.resolve_fork();
        assert_eq!(e.height(), 2);
    }
    #[test]
    fn test_partition() {
        let mut a = ConsensusEngine::new();
        let mut c = ConsensusEngine::new();
        let b = BlockBuilder::new("t");
        for i in 1..=5 {
            a.submit_block(mb(&b, i, &a.chain().latest_hash(), &format!("s{}", i)));
        }
        c.sync_peer(a.chain()).unwrap();
        assert_eq!(c.height(), 5);
    }
    #[test]
    fn test_deterministic() {
        let run = || {
            let mut e = ConsensusEngine::new();
            let b = BlockBuilder::new("t");
            for i in 1..=3 {
                e.submit_block(mb(&b, i, &e.chain().latest_hash(), &format!("s{}", i)));
            }
            (e.height(), e.chain().latest_hash())
        };
        let r1 = run();
        let r2 = run();
        assert_eq!(r1, r2);
    }
}
