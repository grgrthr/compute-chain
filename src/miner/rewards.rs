use crate::miner::types::MinerStats;

pub struct RewardSystem {
    base_reward: u64,
    difficulty_multiplier: f64,
}

impl RewardSystem {
    pub fn new() -> Self {
        Self {
            base_reward: 100,
            difficulty_multiplier: 1.0,
        }
    }

    pub fn calculate_reward(
        &self,
        difficulty_level: u32,
        execution_time_ms: u64,
        proof_valid: bool,
    ) -> u64 {
        if !proof_valid {
            return 0;
        }

        let target_time = 100u64;
        let time_factor = if execution_time_ms < target_time {
            (target_time as f64 / execution_time_ms as f64).min(2.0)
        } else {
            (target_time as f64 / execution_time_ms as f64).max(0.1)
        };

        let reward = self.base_reward as f64
            * difficulty_level as f64
            * time_factor
            * self.difficulty_multiplier;

        reward as u64
    }

    pub fn distribute_reward(
        &self,
        stats: &mut MinerStats,
        difficulty: u32,
        exec_time: u64,
        proof_valid: bool,
    ) -> u64 {
        let reward = self.calculate_reward(difficulty, exec_time, proof_valid);
        if reward > 0 {
            stats.add_success(exec_time, reward);
        } else if !proof_valid {
            stats.add_failure();
        }
        reward
    }

    pub fn adjust_difficulty_multiplier(&mut self, network_hash_rate: f64) {
        let target_hash_rate = 1000.0;
        if network_hash_rate > target_hash_rate {
            self.difficulty_multiplier *= 1.1;
            self.difficulty_multiplier = self.difficulty_multiplier.min(10.0);
        } else if network_hash_rate < target_hash_rate * 0.5 {
            self.difficulty_multiplier *= 0.9;
            self.difficulty_multiplier = self.difficulty_multiplier.max(0.1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::miner::types::MinerStats;

    #[test]
    fn test_reward_calculation() {
        let rewards = RewardSystem::new();

        let reward_valid = rewards.calculate_reward(5, 100, true);
        assert!(reward_valid > 0);

        let reward_invalid = rewards.calculate_reward(5, 100, false);
        assert_eq!(reward_invalid, 0);
    }

    #[test]
    fn test_reward_distribution() {
        let rewards = RewardSystem::new();
        let mut stats = MinerStats::new("miner1".to_string());

        let reward = rewards.distribute_reward(&mut stats, 5, 100, true);
        assert!(reward > 0);
        assert_eq!(stats.completed_workloads, 1);
        assert_eq!(stats.total_rewards, reward);
    }
}
