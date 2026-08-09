use crate::consensus::types::Validator;
use rand::Rng;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct StakeInfo {
    pub validator_id: String,
    pub amount: u64,
    pub since: u64,
}

pub struct ProofOfStake {
    pub stakes: Arc<Mutex<BTreeMap<String, StakeInfo>>>,
    pub validators: Arc<Mutex<Vec<String>>>,
}

impl ProofOfStake {
    pub fn new() -> Self {
        Self {
            stakes: Arc::new(Mutex::new(BTreeMap::new())),
            validators: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn add_stake(&self, validator_id: String, amount: u64) {
        let mut stakes = self.stakes.lock().unwrap();
        let stake = StakeInfo {
            validator_id: validator_id.clone(),
            amount,
            since: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        stakes.insert(validator_id.clone(), stake);

        let mut validators = self.validators.lock().unwrap();
        if !validators.contains(&validator_id) {
            validators.push(validator_id);
        }
    }

    pub fn remove_stake(&self, validator_id: &str) {
        let mut stakes = self.stakes.lock().unwrap();
        stakes.remove(validator_id);

        let mut validators = self.validators.lock().unwrap();
        validators.retain(|v| v != validator_id);
    }

    pub fn get_stake(&self, validator_id: &str) -> Option<StakeInfo> {
        let stakes = self.stakes.lock().unwrap();
        stakes.get(validator_id).cloned()
    }

    pub fn get_total_stake(&self) -> u64 {
        let stakes = self.stakes.lock().unwrap();
        stakes.values().map(|s| s.amount).sum()
    }

    pub fn get_all_stakes(&self) -> Vec<StakeInfo> {
        let stakes = self.stakes.lock().unwrap();
        stakes.values().cloned().collect()
    }

    pub fn select_validator(&self) -> Option<String> {
        let stakes = self.stakes.lock().unwrap();
        if stakes.is_empty() {
            return None;
        }

        let total: u64 = stakes.values().map(|s| s.amount).sum();
        let mut rng = rand::thread_rng();
        let mut target = rng.gen_range(0..total);

        for (id, stake) in stakes.iter() {
            if target < stake.amount {
                return Some(id.clone());
            }
            target -= stake.amount;
        }

        stakes.keys().next().cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_stake() {
        let pos = ProofOfStake::new();
        pos.add_stake("v1".to_string(), 1000);
        assert_eq!(pos.get_total_stake(), 1000);
    }

    #[test]
    fn test_select_validator() {
        let pos = ProofOfStake::new();
        pos.add_stake("v1".to_string(), 1000);
        pos.add_stake("v2".to_string(), 2000);

        let selected = pos.select_validator();
        assert!(selected.is_some());
    }
}

// Strategy: Dependency Inversion for consensus (Core)
// Review and adjust before applying.
