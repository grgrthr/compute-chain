use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Incentive {
    pub id: String,
    pub miner_id: String,
    pub workload_id: String,
    pub reward_amount: u64,
    pub reputation_bonus: f64,
    pub timestamp: u64,
}

pub struct IncentiveManager {
    incentives: Arc<Mutex<Vec<Incentive>>>,
    total_distributed: Arc<Mutex<u64>>,
}

impl IncentiveManager {
    pub fn new() -> Self {
        Self {
            incentives: Arc::new(Mutex::new(Vec::new())),
            total_distributed: Arc::new(Mutex::new(0)),
        }
    }

    pub fn calculate_incentive(
        &self,
        difficulty: u32,
        execution_time_ms: u64,
        proof_valid: bool,
        reputation_score: f64,
    ) -> u64 {
        if !proof_valid {
            return 0;
        }

        let base_reward = 100 * difficulty as u64;
        let time_bonus = if execution_time_ms < 50 {
            base_reward / 2
        } else if execution_time_ms < 100 {
            base_reward / 4
        } else {
            0
        };

        let reputation_multiplier = 0.5 + (reputation_score / 1000.0);

        let total = ((base_reward + time_bonus) as f64 * reputation_multiplier) as u64;
        total.max(1)
    }

    pub fn distribute_incentive(
        &self,
        miner_id: &str,
        workload_id: &str,
        amount: u64,
    ) -> Incentive {
        let incentive = Incentive {
            id: uuid::Uuid::new_v4().to_string(),
            miner_id: miner_id.to_string(),
            workload_id: workload_id.to_string(),
            reward_amount: amount,
            reputation_bonus: 0.0,
            timestamp: Self::current_time(),
        };

        let mut incentives = self.incentives.lock().unwrap();
        incentives.push(incentive.clone());

        let mut total = self.total_distributed.lock().unwrap();
        *total += amount;

        incentive
    }

    pub fn get_miner_incentives(&self, miner_id: &str) -> Vec<Incentive> {
        let incentives = self.incentives.lock().unwrap();
        incentives
            .iter()
            .filter(|i| i.miner_id == miner_id)
            .cloned()
            .collect()
    }

    pub fn get_total_distributed(&self) -> u64 {
        *self.total_distributed.lock().unwrap()
    }

    fn current_time() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_incentive() {
        let incentives = IncentiveManager::new();

        let reward = incentives.calculate_incentive(5, 50, true, 500.0);
        assert!(reward > 0);

        let invalid_reward = incentives.calculate_incentive(5, 50, false, 500.0);
        assert_eq!(invalid_reward, 0);
    }
}

// Strategy: Dependency Inversion for economic (Core)
// Review and adjust before applying.
