use crate::consensus::types::Transaction;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mempool {
    pub transactions: BTreeMap<String, Transaction>,
    pub max_size: usize,
}

impl Mempool {
    pub fn new(max_size: usize) -> Self {
        Self {
            transactions: BTreeMap::new(),
            max_size,
        }
    }

    /// Add a transaction to the mempool (prevents duplicates, checks nonce)
    pub fn add(&mut self, tx: Transaction, current_nonce: u64) -> Result<(), String> {
        if self.transactions.contains_key(&tx.id) {
            return Err("Transaction already in mempool".into());
        }
        if self.transactions.len() >= self.max_size {
            return Err("Mempool is full".into());
        }
        if tx.nonce < current_nonce {
            return Err(format!("Nonce too low: {} < {}", tx.nonce, current_nonce));
        }
        self.transactions.insert(tx.id.clone(), tx);
        Ok(())
    }

    /// Remove a transaction by id
    pub fn remove(&mut self, tx_id: &str) {
        self.transactions.remove(tx_id);
    }

    /// Get top N transactions ordered by fee (highest first), then by id (deterministic tie-breaker)
    pub fn get_top(&self, limit: usize) -> Vec<Transaction> {
        let mut txs: Vec<Transaction> = self.transactions.values().cloned().collect();
        txs.sort_by(|a, b| b.fee.cmp(&a.fee).then_with(|| a.id.cmp(&b.id)));
        txs.truncate(limit);
        txs
    }

    /// Remove multiple transactions by ids
    pub fn remove_batch(&mut self, tx_ids: &[String]) {
        for id in tx_ids {
            self.transactions.remove(id);
        }
    }

    /// Get current mempool size
    pub fn len(&self) -> usize {
        self.transactions.len()
    }

    /// Get all transactions in deterministic order (by id)
    pub fn get_all(&self) -> Vec<Transaction> {
        self.transactions.values().cloned().collect()
    }

    /// Check if transaction exists
    pub fn contains(&self, tx_id: &str) -> bool {
        self.transactions.contains_key(tx_id)
    }

    /// Save mempool to disk (sorted by key for determinism)
    pub fn save_to_disk(&self, path: &str) -> Result<(), String> {
        let txs: Vec<&Transaction> = self.transactions.values().collect();
        let json = serde_json::to_string_pretty(&txs).map_err(|e| e.to_string())?;
        let dir = format!("{}/mempool", path);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        std::fs::write(format!("{}/mempool.json", dir), json).map_err(|e| e.to_string())?;
        tracing::info!("💾 Mempool saved: {} transactions", txs.len());
        Ok(())
    }

    /// Load mempool from disk
    pub fn load_from_disk(path: &str, max_size: usize) -> Self {
        let file = format!("{}/mempool/mempool.json", path);
        if !std::path::Path::new(&file).exists() {
            return Self::new(max_size);
        }
        match std::fs::read_to_string(&file) {
            Ok(json) => match serde_json::from_str::<Vec<Transaction>>(&json) {
                Ok(txs) => {
                    let mut mempool = Self::new(max_size);
                    for tx in txs {
                        let _ = mempool.add(tx, 0);
                    }
                    tracing::info!("📂 Mempool loaded: {} transactions", mempool.len());
                    mempool
                }
                Err(_) => Self::new(max_size),
            },
            Err(_) => Self::new(max_size),
        }
    }
}

/// Thread-safe mempool wrapper using std::sync::RwLock
/// - read() for queries (get_top, get_all, contains, len, save)
/// - write() for mutations (add, remove, remove_batch)
pub struct SharedMempool {
    pub inner: Arc<RwLock<Mempool>>,
}

impl SharedMempool {
    pub fn new(max_size: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Mempool::new(max_size))),
        }
    }

    pub fn load_from_disk(path: &str, max_size: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Mempool::load_from_disk(path, max_size))),
        }
    }
}

// Strategy: Dependency Inversion for consensus (Core)
// Review and adjust before applying.
