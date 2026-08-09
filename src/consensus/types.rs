use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::SystemTime;

/// Chain ID - must be identical across all nodes for consensus
pub const CHAIN_ID: &str = "compute-chain-mainnet-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConsensusState {
    Initializing,
    Syncing,
    Validating,
    Committing,
    Active,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Validator {
    pub id: String,
    pub address: String,
    pub stake: u64,
    pub commission: f64,
    pub active: bool,
    pub blocks_validated: u64,
    pub rewards_earned: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub height: u64,
    pub timestamp: u64,
    pub previous_hash: String,
    pub hash: String,
    pub validator_id: String,
    pub transactions: Vec<Transaction>,
    pub signature: String,
    pub proof: Vec<u8>,
    pub compute_proof: Option<crate::stark::simple_stark::SimpleProof>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub from: String,
    pub to: String,
    pub amount: u64,
    pub fee: u64,
    pub timestamp: u64,
    pub public_key: String,
    pub signature: String,
    pub nonce: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vote {
    pub validator_id: String,
    pub block_hash: String,
    pub vote_type: VoteType,
    pub signature: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VoteType {
    Prepare,
    Commit,
}

impl Validator {
    pub fn new(id: String, address: String, stake: u64, commission: f64) -> Self {
        Self {
            id,
            address,
            stake,
            commission,
            active: true,
            blocks_validated: 0,
            rewards_earned: 0,
        }
    }
}

impl Block {
    pub fn new_genesis() -> Self {
        Self {
            height: 0,
            timestamp: 0,
            previous_hash: "0".to_string(),
            hash: "0000000000000000genesis_block_compute_chain".to_string(),
            validator_id: "genesis".to_string(),
            transactions: vec![],
            signature: String::new(),
            proof: Vec::new(),
            compute_proof: None,
        }
    }

    pub fn new(
        height: u64,
        previous_hash: String,
        validator_id: String,
        transactions: Vec<Transaction>,
    ) -> Self {
        let timestamp = Self::current_time();
        let hash = Self::calculate_hash(
            height,
            timestamp,
            &previous_hash,
            &validator_id,
            &transactions,
        );

        Self {
            height,
            timestamp,
            previous_hash,
            hash,
            validator_id,
            transactions,
            signature: String::new(),
            proof: Vec::new(),
            compute_proof: None,
        }
    }

    pub fn verify_compute_proof(&self) -> bool {
        self.compute_proof.is_some()
    }

    /// Calculate block hash deterministically
    fn calculate_hash(
        height: u64,
        timestamp: u64,
        previous_hash: &str,
        validator_id: &str,
        transactions: &[Transaction],
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(CHAIN_ID.as_bytes());
        hasher.update(height.to_le_bytes());
        hasher.update(timestamp.to_le_bytes());
        hasher.update(previous_hash.as_bytes());
        hasher.update(validator_id.as_bytes());

        // Process transactions in order (Vec is ordered, so this is deterministic)
        for tx in transactions {
            hasher.update(tx.id.as_bytes());
            hasher.update(tx.from.as_bytes());
            hasher.update(tx.to.as_bytes());
            hasher.update(tx.amount.to_le_bytes());
            hasher.update(tx.fee.to_le_bytes());
            hasher.update(tx.nonce.to_le_bytes());
        }

        hex::encode(hasher.finalize())
    }

    fn current_time() -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

impl Transaction {
    pub fn new(from: String, to: String, amount: u64, fee: u64) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let id = format!("{}-{}-{}", from, to, timestamp);

        Self {
            id,
            from,
            to,
            amount,
            fee,
            timestamp,
            public_key: String::new(),
            signature: String::new(),
            nonce: 0,
        }
    }

    /// Create the message bytes for signing.
    /// Includes Chain ID to prevent cross-chain replay attacks.
    /// Includes all fields that affect execution: from, to, amount, fee, timestamp, nonce.
    pub fn signing_message(&self) -> Vec<u8> {
        let mut msg = Vec::new();
        msg.extend_from_slice(CHAIN_ID.as_bytes());
        msg.extend_from_slice(self.from.as_bytes());
        msg.extend_from_slice(self.to.as_bytes());
        msg.extend_from_slice(&self.amount.to_le_bytes());
        msg.extend_from_slice(&self.fee.to_le_bytes());
        msg.extend_from_slice(&self.timestamp.to_le_bytes());
        msg.extend_from_slice(&self.nonce.to_le_bytes());
        msg
    }

    /// Verify this transaction's signature
    pub fn verify_signature(&self) -> bool {
        if self.public_key.is_empty() || self.signature.is_empty() {
            return false;
        }
        let msg = self.signing_message();
        crate::crypto::signer::verify(&self.public_key, &msg, &self.signature)
    }
}

// Strategy: Dependency Inversion for consensus (Core)
// Review and adjust before applying.
