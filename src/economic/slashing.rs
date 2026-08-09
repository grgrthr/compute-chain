use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// Types of slashable offenses
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SlashOffense {
    /// Validator signed two different blocks at the same height
    DoubleBlockProposal {
        height: u64,
        block1_hash: String,
        block2_hash: String,
    },
    /// Validator submitted an invalid proof
    InvalidProof { height: u64, reason: String },
    /// Validator submitted an invalid execution result
    InvalidExecution { height: u64, reason: String },
    /// Validator was offline for too long
    ExtendedDowntime { missed_blocks: u64 },
}

/// Record of a slashing event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashRecord {
    pub validator_id: String,
    pub offense: SlashOffense,
    pub slash_amount: u64,
    pub timestamp: u64,
    pub block_height: u64,
}

/// Slashing penalties configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashingConfig {
    /// Percentage of stake slashed for double block (0-100)
    pub double_block_penalty_percent: u8,
    /// Percentage of stake slashed for invalid proof
    pub invalid_proof_penalty_percent: u8,
    /// Percentage of stake slashed for invalid execution
    pub invalid_execution_penalty_percent: u8,
    /// Fixed amount slashed for downtime
    pub downtime_penalty_fixed: u64,
    /// Maximum slashable percentage of total stake
    pub max_slash_percent: u8,
}

impl Default for SlashingConfig {
    fn default() -> Self {
        Self {
            double_block_penalty_percent: 20,
            invalid_proof_penalty_percent: 10,
            invalid_execution_penalty_percent: 5,
            downtime_penalty_fixed: 10,
            max_slash_percent: 50,
        }
    }
}

/// Slashing engine
pub struct SlashingEngine {
    /// History of all slash events
    pub records: Arc<Mutex<Vec<SlashRecord>>>,
    /// Count of offenses per validator
    pub offense_counts: Arc<Mutex<BTreeMap<String, u64>>>,
    /// Configuration
    pub config: SlashingConfig,
}

impl SlashingEngine {
    pub fn new(config: SlashingConfig) -> Self {
        Self {
            records: Arc::new(Mutex::new(Vec::new())),
            offense_counts: Arc::new(Mutex::new(BTreeMap::new())),
            config,
        }
    }

    pub fn new_default() -> Self {
        Self::new(SlashingConfig::default())
    }

    /// Slash a validator for an offense
    /// Returns the slash amount
    pub fn slash(
        &self,
        validator_id: &str,
        stake: u64,
        offense: SlashOffense,
        block_height: u64,
    ) -> u64 {
        let slash_percent = match &offense {
            SlashOffense::DoubleBlockProposal { .. } => self.config.double_block_penalty_percent,
            SlashOffense::InvalidProof { .. } => self.config.invalid_proof_penalty_percent,
            SlashOffense::InvalidExecution { .. } => self.config.invalid_execution_penalty_percent,
            SlashOffense::ExtendedDowntime { .. } => 0,
        };

        let slash_amount = if let SlashOffense::ExtendedDowntime { .. } = &offense {
            self.config.downtime_penalty_fixed
        } else {
            (stake * slash_percent as u64) / 100
        };

        // Cap at max slashable
        let max_slash = (stake * self.config.max_slash_percent as u64) / 100;
        let actual_slash = slash_amount.min(max_slash);

        // Record the event
        let record = SlashRecord {
            validator_id: validator_id.to_string(),
            offense: offense.clone(),
            slash_amount: actual_slash,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            block_height,
        };

        let offense_type = format!("{:?}", std::mem::discriminant(&record.offense));

        {
            let mut records = self.records.lock().unwrap();
            records.push(record);
        }

        // Increment offense count
        {
            let mut counts = self.offense_counts.lock().unwrap();
            *counts.entry(validator_id.to_string()).or_insert(0) += 1;
        }

        tracing::warn!(
            "🗡️ Slashed validator {} for {}: {} tokens (offense #{})",
            validator_id,
            offense_type,
            actual_slash,
            self.get_offense_count(validator_id)
        );

        actual_slash
    }

    /// Get total offense count for a validator
    pub fn get_offense_count(&self, validator_id: &str) -> u64 {
        let counts = self.offense_counts.lock().unwrap();
        counts.get(validator_id).copied().unwrap_or(0)
    }

    /// Get all slash records for a validator
    pub fn get_records(&self, validator_id: &str) -> Vec<SlashRecord> {
        let records = self.records.lock().unwrap();
        records
            .iter()
            .filter(|r| r.validator_id == validator_id)
            .cloned()
            .collect()
    }

    /// Get total slash history
    pub fn get_all_records(&self) -> Vec<SlashRecord> {
        self.records.lock().unwrap().clone()
    }

    /// Check if a block is a double proposal
    /// Returns previous block hash if this validator already proposed at this height
    pub fn check_double_block(
        &self,
        validator_id: &str,
        height: u64,
        _hash: &str,
    ) -> Option<String> {
        let records = self.records.lock().unwrap();
        for record in records.iter() {
            if let SlashOffense::DoubleBlockProposal {
                height: h,
                block1_hash: ref h1,
                ..
            } = record.offense
            {
                if h == height && record.validator_id == validator_id {
                    return Some(h1.clone());
                }
            }
        }
        None
    }

    /// Save slashing state to disk
    pub fn save_to_disk(&self, path: &str) -> Result<(), String> {
        let records = self.records.lock().unwrap();
        let counts = self.offense_counts.lock().unwrap();

        let dir = format!("{}/slashing", path);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

        let data = serde_json::json!({
            "records": &*records,
            "offense_counts": &*counts,
            "config": self.config,
            "saved_at": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });

        let json = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
        std::fs::write(format!("{}/slashing.json", dir), json).map_err(|e| e.to_string())?;
        tracing::info!("💾 Slashing state saved: {} records", records.len());
        Ok(())
    }

    /// Load slashing state from disk
    pub fn load_from_disk(path: &str) -> Self {
        let file = format!("{}/slashing/slashing.json", path);
        if !std::path::Path::new(&file).exists() {
            return Self::new_default();
        }

        match std::fs::read_to_string(&file) {
            Ok(json) => match serde_json::from_str::<serde_json::Value>(&json) {
                Ok(data) => {
                    let records: Vec<SlashRecord> =
                        serde_json::from_value(data["records"].clone()).unwrap_or_default();
                    let offense_counts: BTreeMap<String, u64> =
                        serde_json::from_value(data["offense_counts"].clone()).unwrap_or_default();
                    let config: SlashingConfig =
                        serde_json::from_value(data["config"].clone()).unwrap_or_default();

                    tracing::info!("📂 Slashing state loaded: {} records", records.len());
                    Self {
                        records: Arc::new(Mutex::new(records)),
                        offense_counts: Arc::new(Mutex::new(offense_counts)),
                        config,
                    }
                }
                Err(_) => Self::new_default(),
            },
            Err(_) => Self::new_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slash_double_block() {
        let engine = SlashingEngine::new_default();

        let amount = engine.slash(
            "validator1",
            1000,
            SlashOffense::DoubleBlockProposal {
                height: 5,
                block1_hash: "hash1".into(),
                block2_hash: "hash2".into(),
            },
            5,
        );

        assert!(amount > 0);
        assert_eq!(engine.get_offense_count("validator1"), 1);
        assert_eq!(engine.get_records("validator1").len(), 1);
    }

    #[test]
    fn test_slash_capped_at_max() {
        let mut config = SlashingConfig::default();
        config.double_block_penalty_percent = 80;
        let engine = SlashingEngine::new(config);

        let amount = engine.slash(
            "validator1",
            1000,
            SlashOffense::DoubleBlockProposal {
                height: 5,
                block1_hash: "h1".into(),
                block2_hash: "h2".into(),
            },
            5,
        );

        assert_eq!(amount, 500);
    }

    #[test]
    fn test_save_and_load() {
        let engine = SlashingEngine::new_default();
        engine.slash(
            "v1",
            1000,
            SlashOffense::InvalidProof {
                height: 3,
                reason: "Bad proof".into(),
            },
            3,
        );

        let path = "/tmp/test_slashing";
        let _ = std::fs::remove_dir_all(path);
        engine.save_to_disk(path).unwrap();

        let loaded = SlashingEngine::load_from_disk(path);
        assert_eq!(loaded.get_offense_count("v1"), 1);
        assert_eq!(loaded.get_records("v1").len(), 1);

        let _ = std::fs::remove_dir_all(path);
    }
}

// Strategy: Dependency Inversion for economic (Core)
// Review and adjust before applying.
