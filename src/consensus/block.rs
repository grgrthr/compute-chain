use crate::consensus::types::{Block, Transaction};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct Blockchain {
    pub chain: Arc<Mutex<Vec<Block>>>,
    pub pending_transactions: Arc<Mutex<VecDeque<Transaction>>>,
}

impl Blockchain {
    pub fn new() -> Self {
        let genesis_block = Block::new_genesis();
        println!(
            "🌍 Genesis block created: height=0, hash={}",
            genesis_block.hash
        );

        Self {
            chain: Arc::new(Mutex::new(vec![genesis_block])),
            pending_transactions: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn add_block(&self, block: Block) -> Result<(), String> {
        let mut chain = self.chain.lock().unwrap();
        let last_block = chain.last().unwrap();

        if block.height != last_block.height + 1 {
            return Err(format!(
                "Invalid height: expected {}, got {}",
                last_block.height + 1,
                block.height
            ));
        }

        if block.previous_hash != last_block.hash {
            return Err(format!(
                "Invalid previous hash: expected {}, got {}",
                last_block.hash, block.previous_hash
            ));
        }

        chain.push(block);
        Ok(())
    }

    pub fn add_block_validated(&self, block: Block) -> Result<(), String> {
        let mut chain = self.chain.lock().unwrap();
        let last_block = chain.last().unwrap();

        if block.height != last_block.height + 1 {
            return Err(format!(
                "Invalid height: expected {}, got {}",
                last_block.height + 1,
                block.height
            ));
        }

        if block.previous_hash != last_block.hash {
            return Err(format!(
                "Invalid previous hash: expected {}, got {}",
                last_block.hash, block.previous_hash
            ));
        }

        if !block.verify_compute_proof() {
            return Err("Block has no valid compute proof".into());
        }

        chain.push(block);
        Ok(())
    }

    pub fn get_last_block(&self) -> Block {
        let chain = self.chain.lock().unwrap();
        chain.last().unwrap().clone()
    }

    pub fn get_height(&self) -> u64 {
        let chain = self.chain.lock().unwrap();
        chain.last().unwrap().height
    }

    pub fn get_block(&self, height: u64) -> Option<Block> {
        let chain = self.chain.lock().unwrap();
        chain.get(height as usize).cloned()
    }

    pub fn get_all_blocks(&self) -> Vec<Block> {
        let chain = self.chain.lock().unwrap();
        chain.clone()
    }

    pub fn get_recent_blocks(&self, count: usize) -> Vec<Block> {
        let chain = self.chain.lock().unwrap();
        let start = chain.len().saturating_sub(count);
        chain[start..].to_vec()
    }

    pub fn add_transaction(&self, tx: Transaction) -> Result<(), String> {
        let mut pending = self.pending_transactions.lock().unwrap();
        pending.push_back(tx);
        Ok(())
    }

    pub fn remove_pending_transactions(&self, tx_ids: &[String]) {
        let mut pending = self.pending_transactions.lock().unwrap();
        pending.retain(|tx| !tx_ids.contains(&tx.id));
    }

    /// Save all blocks to disk - uses snapshot to minimize lock time
    pub fn save_to_disk(&self, path: &str) -> Result<(), String> {
        let snapshot = {
            let chain = self.chain.lock().unwrap();
            chain.clone()
        }; // Lock released here - very fast

        let dir = format!("{}/blocks", path);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

        for block in snapshot.iter() {
            let json = serde_json::to_string_pretty(block).map_err(|e| e.to_string())?;
            let filename = format!("{}/block_{:08}.json", dir, block.height);
            std::fs::write(&filename, json).map_err(|e| e.to_string())?;
        }

        println!("💾 Chain saved: {} blocks to {}", snapshot.len(), dir);
        Ok(())
    }

    /// Load all blocks from disk
    pub fn load_from_disk(path: &str) -> Result<Self, String> {
        let dir = format!("{}/blocks", path);
        if !std::path::Path::new(&dir).exists() {
            return Err("No blocks directory found".into());
        }

        let mut blocks = Vec::new();
        let mut entries: Vec<_> = std::fs::read_dir(&dir)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok())
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let contents = std::fs::read_to_string(entry.path()).map_err(|e| e.to_string())?;
            let block: Block = serde_json::from_str(&contents).map_err(|e| e.to_string())?;
            blocks.push(block);
        }

        if blocks.is_empty() {
            return Err("No blocks found".into());
        }

        println!("💾 Chain loaded: {} blocks from disk", blocks.len());

        Ok(Self {
            chain: Arc::new(Mutex::new(blocks)),
            pending_transactions: Arc::new(Mutex::new(VecDeque::new())),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_block() {
        let blockchain = Blockchain::new();
        let genesis = blockchain.get_last_block();
        assert_eq!(genesis.height, 0);
        assert_eq!(genesis.previous_hash, "0");
    }
}

// Strategy: Dependency Inversion for consensus (Core)
// Review and adjust before applying.
