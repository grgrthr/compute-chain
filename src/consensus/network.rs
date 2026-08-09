use crate::consensus::block::Blockchain;
use crate::consensus::p_bft::PracticalBFT;
use crate::consensus::pos::ProofOfStake;
use crate::consensus::types::{Block, ConsensusState, Transaction};
use crate::consensus::validator::ValidatorSet;
use std::sync::{Arc, Mutex};

pub struct ConsensusNetwork {
    pub validators: Arc<ValidatorSet>,
    pub blockchain: Arc<Blockchain>,
    pos: Arc<ProofOfStake>,
    pub pbft: Arc<PracticalBFT>,
    state: Arc<Mutex<ConsensusState>>,
    storage_path: String,
}

impl ConsensusNetwork {
    pub fn new() -> Self {
        let validators = Arc::new(ValidatorSet::new());
        let blockchain = Arc::new(
            Blockchain::load_from_disk("./chain_data").unwrap_or_else(|_| Blockchain::new()),
        );
        let pos = Arc::new(ProofOfStake::new());

        // Register 3 default validators
        validators.register_validator("node1".into(), "0xNode1".into(), 1000, 0.05);
        validators.register_validator("node2".into(), "0xNode2".into(), 1000, 0.05);
        validators.register_validator("node3".into(), "0xNode3".into(), 1000, 0.05);

        let validators_list = vec![
            "node1".to_string(),
            "node2".to_string(),
            "node3".to_string(),
        ];

        let pbft = Arc::new(PracticalBFT::new(validators_list));

        Self {
            validators,
            blockchain,
            pos,
            pbft,
            state: Arc::new(Mutex::new(ConsensusState::Initializing)),
            storage_path: "./chain_data".into(),
        }
    }

    pub fn commit_block(&self, block: Block) -> Result<(), String> {
        // Check PBFT consensus (f=0 for 3 validators means 1 vote is enough)
        if !self.pbft.is_committed(&block.hash) {
            return Err("PBFT consensus not reached".into());
        }

        self.blockchain.add_block_validated(block.clone())?;
        let _ = self.save_to_disk();
        Ok(())
    }

    /// Fork Resolution: longer valid chain wins
    pub fn resolve_fork(&self, incoming_height: u64, incoming_hash: &str) -> bool {
        let current_height = self.get_blockchain_height();
        let current_block = self.get_last_block();

        if incoming_height > current_height {
            println!(
                "🔄 Fork resolved: longer chain ({} > {})",
                incoming_height, current_height
            );
            return true;
        }

        if incoming_height == current_height && incoming_hash != current_block.hash {
            println!("⚠️ Fork at height {} - comparing hashes", current_height);
            return incoming_hash.as_bytes() < current_block.hash.as_bytes();
        }

        false
    }

    pub fn get_last_block(&self) -> Block {
        self.blockchain.get_last_block()
    }

    pub fn get_blockchain_height(&self) -> u64 {
        self.blockchain.get_height()
    }

    pub fn get_validator_set(&self) -> Vec<String> {
        self.validators
            .list_validators()
            .iter()
            .map(|v| v.id.clone())
            .collect()
    }

    pub fn get_all_blocks(&self) -> Vec<Block> {
        let chain = self.blockchain.chain.lock().unwrap();
        chain.clone()
    }

    pub fn get_block_by_height(&self, height: u64) -> Option<Block> {
        self.blockchain.get_block(height)
    }

    pub fn add_transaction(
        &self,
        from: String,
        to: String,
        amount: u64,
        _fee: u64,
    ) -> Result<(), String> {
        let tx = Transaction::new(from, to, amount, _fee);
        self.blockchain.add_transaction(tx)
    }

    pub fn save_to_disk(&self) -> Result<(), String> {
        self.blockchain.save_to_disk(&self.storage_path)
    }
}

// Strategy: Dependency Inversion for consensus (Core)
// Review and adjust before applying.
