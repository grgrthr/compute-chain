use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBalance {
    pub address: String,
    pub balance: u64,
    pub locked_balance: u64,
    pub last_update: u64,
    pub nonce: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenTransfer {
    pub from: String,
    pub to: String,
    pub amount: u64,
    pub timestamp: u64,
    pub tx_hash: String,
}

pub struct TokenEngine {
    balances: Arc<RwLock<BTreeMap<String, TokenBalance>>>,
    transfers: Arc<RwLock<Vec<TokenTransfer>>>,
    total_supply: Arc<RwLock<u64>>,
}

impl TokenEngine {
    pub fn new() -> Self {
        let mut balances = BTreeMap::new();

        balances.insert(
            "genesis".to_string(),
            TokenBalance {
                address: "genesis".to_string(),
                balance: 1_000_000_000,
                locked_balance: 0,
                last_update: Self::current_time(),
                nonce: 0,
            },
        );

        println!("🔧 TokenEngine::new() - created with genesis account");

        Self {
            balances: Arc::new(RwLock::new(balances)),
            transfers: Arc::new(RwLock::new(Vec::new())),
            total_supply: Arc::new(RwLock::new(1_000_000_000)),
        }
    }

    /// Read-only: get balance
    pub fn get_balance(&self, address: &str) -> u64 {
        let balances = self.balances.read().unwrap();
        balances.get(address).map(|b| b.balance).unwrap_or(0)
    }

    /// Read-only: get nonce
    pub fn get_nonce(&self, address: &str) -> u64 {
        let balances = self.balances.read().unwrap();
        balances.get(address).map(|b| b.nonce).unwrap_or(0)
    }

    /// Write: increment nonce (called after successful signed transfer)
    pub fn increment_nonce(&self, address: &str) {
        let mut balances = self.balances.write().unwrap();
        if let Some(balance) = balances.get_mut(address) {
            balance.nonce += 1;
        } else {
            balances.insert(
                address.to_string(),
                TokenBalance {
                    address: address.to_string(),
                    balance: 0,
                    locked_balance: 0,
                    last_update: Self::current_time(),
                    nonce: 1,
                },
            );
        }
    }

    /// 🔐 Signed transfer - uses write() lock from the start
    pub fn transfer_signed(
        &self,
        tx: &crate::consensus::types::Transaction,
    ) -> Result<String, String> {
        // 1. Validate amount
        if tx.amount == 0 {
            return Err("Amount must be greater than zero".to_string());
        }

        // 2. Verify signature (includes Chain ID, from, to, amount, fee, timestamp, nonce)
        if !tx.verify_signature() {
            return Err("Invalid transaction signature".to_string());
        }

        // 3. Verify nonce (replay protection) - read-only
        let current_nonce = self.get_nonce(&tx.from);
        if tx.nonce < current_nonce {
            return Err(format!(
                "Nonce too low: {} < {} (possible replay attack)",
                tx.nonce, current_nonce
            ));
        }
        if tx.nonce > current_nonce {
            return Err(format!(
                "Nonce too high: {} > {} (transactions must be sequential)",
                tx.nonce, current_nonce
            ));
        }

        // 4. Execute transfer - write lock
        {
            let mut balances = self.balances.write().unwrap();

            // Check sender exists and has enough balance (amount + fee)
            let from_balance = balances.get(&tx.from).ok_or("Sender not found")?;
            if from_balance.balance < tx.amount + tx.fee {
                return Err("Insufficient balance (amount + fee)".to_string());
            }

            // Deduct from sender
            let from_balance = balances.get_mut(&tx.from).unwrap();
            from_balance.balance -= tx.amount + tx.fee;

            // Credit to recipient
            let to_balance = balances.entry(tx.to.clone()).or_insert(TokenBalance {
                address: tx.to.clone(),
                balance: 0,
                locked_balance: 0,
                last_update: Self::current_time(),
                nonce: 0,
            });
            to_balance.balance += tx.amount;

            // Increment sender's nonce AFTER successful transfer
            let from_balance = balances.get_mut(&tx.from).unwrap();
            from_balance.nonce += 1;
            from_balance.last_update = Self::current_time();
        }

        // 5. Record transfer - write lock
        {
            let mut transfers = self.transfers.write().unwrap();
            transfers.push(TokenTransfer {
                from: tx.from.clone(),
                to: tx.to.clone(),
                amount: tx.amount,
                timestamp: Self::current_time(),
                tx_hash: tx.id.clone(),
            });
        }

        tracing::info!(
            "💸 Signed transfer: {} -> {} : {} (nonce: {}, fee: {})",
            &tx.from[..12],
            &tx.to[..12],
            tx.amount,
            tx.nonce,
            tx.fee
        );

        Ok(tx.id.clone())
    }

    /// Legacy transfer without signature
    pub fn transfer(&self, from: &str, to: &str, amount: u64) -> Result<String, String> {
        tracing::warn!("⚠️ Using unsigned transfer - migrate to transfer_signed");

        if amount == 0 {
            return Err("Amount must be greater than zero".to_string());
        }

        let mut balances = self.balances.write().unwrap();

        let from_balance = balances.get_mut(from).ok_or("Sender not found")?;
        if from_balance.balance < amount {
            return Err("Insufficient balance".to_string());
        }

        from_balance.balance -= amount;

        let to_balance = balances.entry(to.to_string()).or_insert(TokenBalance {
            address: to.to_string(),
            balance: 0,
            locked_balance: 0,
            last_update: Self::current_time(),
            nonce: 0,
        });
        to_balance.balance += amount;

        let tx_hash = format!("{}-{}-{}", from, to, Self::current_time());

        let mut transfers = self.transfers.write().unwrap();
        transfers.push(TokenTransfer {
            from: from.to_string(),
            to: to.to_string(),
            amount,
            timestamp: Self::current_time(),
            tx_hash: tx_hash.clone(),
        });

        Ok(tx_hash)
    }

    /// Write: mint new tokens
    pub fn mint(&self, address: &str, amount: u64) {
        let mut balances = self.balances.write().unwrap();
        let balance = balances.entry(address.to_string()).or_insert(TokenBalance {
            address: address.to_string(),
            balance: 0,
            locked_balance: 0,
            last_update: Self::current_time(),
            nonce: 0,
        });
        balance.balance += amount;

        let mut total_supply = self.total_supply.write().unwrap();
        *total_supply += amount;
    }

    /// Write: burn tokens
    pub fn burn(&self, address: &str, amount: u64) -> Result<(), String> {
        let mut balances = self.balances.write().unwrap();
        let balance = balances.get_mut(address).ok_or("Address not found")?;

        if balance.balance < amount {
            return Err("Insufficient balance to burn".to_string());
        }

        balance.balance -= amount;

        let mut total_supply = self.total_supply.write().unwrap();
        *total_supply -= amount;
        Ok(())
    }

    /// Write: lock tokens
    pub fn lock_tokens(&self, address: &str, amount: u64) -> Result<(), String> {
        let mut balances = self.balances.write().unwrap();
        let balance = balances.get_mut(address).ok_or("Address not found")?;

        if balance.balance < amount {
            return Err("Insufficient balance to lock".to_string());
        }

        balance.balance -= amount;
        balance.locked_balance += amount;
        Ok(())
    }

    /// Write: unlock tokens
    pub fn unlock_tokens(&self, address: &str, amount: u64) -> Result<(), String> {
        let mut balances = self.balances.write().unwrap();
        let balance = balances.get_mut(address).ok_or("Address not found")?;

        if balance.locked_balance < amount {
            return Err("Insufficient locked balance".to_string());
        }

        balance.locked_balance -= amount;
        balance.balance += amount;
        Ok(())
    }

    /// Read-only: total supply
    pub fn get_total_supply(&self) -> u64 {
        *self.total_supply.read().unwrap()
    }

    /// Read-only: get recent transfers
    pub fn get_transfers(&self, limit: usize) -> Vec<TokenTransfer> {
        let transfers = self.transfers.read().unwrap();
        transfers.iter().rev().take(limit).cloned().collect()
    }

    /// Read-only: save state to disk
    pub fn save_to_disk(&self, path: &str) -> Result<(), String> {
        let balances = self.balances.read().unwrap();
        let total_supply = *self.total_supply.read().unwrap();

        let dir = format!("{}/tokens", path);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

        let data = serde_json::json!({
            "balances": &*balances,
            "total_supply": total_supply,
            "saved_at": Self::current_time(),
        });

        let json = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
        std::fs::write(format!("{}/balances.json", dir), json).map_err(|e| e.to_string())?;

        println!("💾 Token state saved to {}", dir);
        Ok(())
    }

    /// Load state from disk
    pub fn load_from_disk(path: &str) -> Result<Self, String> {
        let file = format!("{}/tokens/balances.json", path);

        if !std::path::Path::new(&file).exists() {
            println!("💾 load_from_disk: no file found");
            return Err("No token state found".into());
        }

        let json = std::fs::read_to_string(&file).map_err(|e| e.to_string())?;
        let data: serde_json::Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;

        let balances: BTreeMap<String, TokenBalance> =
            serde_json::from_value(data["balances"].clone()).unwrap_or_else(|_| {
                let old_balances: HashMap<String, serde_json::Value> =
                    serde_json::from_value(data["balances"].clone()).unwrap_or_default();
                old_balances
                    .into_iter()
                    .map(|(k, v)| {
                        (
                            k,
                            TokenBalance {
                                address: v["address"].as_str().unwrap_or("").to_string(),
                                balance: v["balance"].as_u64().unwrap_or(0),
                                locked_balance: v["locked_balance"].as_u64().unwrap_or(0),
                                last_update: v["last_update"].as_u64().unwrap_or(0),
                                nonce: v["nonce"].as_u64().unwrap_or(0),
                            },
                        )
                    })
                    .collect()
            });

        let total_supply: u64 = data["total_supply"]
            .as_u64()
            .unwrap_or_else(|| balances.values().map(|b| b.balance).sum());

        println!(
            "💾 Token state loaded: {} accounts, supply: {}",
            balances.len(),
            total_supply
        );

        Ok(Self {
            balances: Arc::new(RwLock::new(balances)),
            transfers: Arc::new(RwLock::new(Vec::new())),
            total_supply: Arc::new(RwLock::new(total_supply)),
        })
    }

    fn current_time() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

// Need HashMap for fallback loading
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transfer() {
        let token = TokenEngine::new();
        token.mint("alice", 1000);
        let result = token.transfer("alice", "bob", 100);
        assert!(result.is_ok());
        assert_eq!(token.get_balance("alice"), 900);
        assert_eq!(token.get_balance("bob"), 100);
    }

    #[test]
    fn test_nonce() {
        let token = TokenEngine::new();
        assert_eq!(token.get_nonce("alice"), 0);
        token.increment_nonce("alice");
        assert_eq!(token.get_nonce("alice"), 1);
        token.increment_nonce("alice");
        assert_eq!(token.get_nonce("alice"), 2);
    }

    #[test]
    fn test_nonce_persists_in_balance() {
        let token = TokenEngine::new();
        token.increment_nonce("alice");
        token.increment_nonce("alice");
        let balances = token.balances.read().unwrap();
        let alice = balances.get("alice").unwrap();
        assert_eq!(alice.nonce, 2);
    }

    #[test]
    fn test_lock_unlock() {
        let token = TokenEngine::new();
        token.mint("alice", 1000);
        token.lock_tokens("alice", 200).unwrap();
        assert_eq!(token.get_balance("alice"), 800);
        token.unlock_tokens("alice", 200).unwrap();
        assert_eq!(token.get_balance("alice"), 1000);
    }

    #[test]
    fn test_save_and_load() {
        let token = TokenEngine::new();
        token.mint("alice", 500);
        token.mint("bob", 300);
        token.increment_nonce("alice");
        token.increment_nonce("alice");

        let path = "/tmp/test_tokens";
        let _ = std::fs::remove_dir_all(path);

        token.save_to_disk(path).unwrap();
        let loaded = TokenEngine::load_from_disk(path).unwrap();

        assert_eq!(loaded.get_balance("alice"), 500);
        assert_eq!(loaded.get_balance("bob"), 300);
        assert_eq!(loaded.get_nonce("alice"), 2);

        let _ = std::fs::remove_dir_all(path);
    }
}

// Strategy: Dependency Inversion for economic (Core)
// Review and adjust before applying.
