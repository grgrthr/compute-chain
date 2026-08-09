use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stake {
    pub address: String,
    pub amount: u64,
    pub locked_until: u64,
    pub apr: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakingPool {
    pub total_staked: u64,
    pub total_rewards: u64,
    pub stakers: HashMap<String, Stake>,
}

pub struct StakingSystem {
    pools: Arc<Mutex<HashMap<u32, StakingPool>>>,
}

impl StakingSystem {
    pub fn new() -> Self {
        let mut pools = HashMap::new();

        pools.insert(
            1,
            StakingPool {
                total_staked: 0,
                total_rewards: 0,
                stakers: HashMap::new(),
            },
        );
        pools.insert(
            2,
            StakingPool {
                total_staked: 0,
                total_rewards: 0,
                stakers: HashMap::new(),
            },
        );
        pools.insert(
            3,
            StakingPool {
                total_staked: 0,
                total_rewards: 0,
                stakers: HashMap::new(),
            },
        );

        Self {
            pools: Arc::new(Mutex::new(pools)),
        }
    }

    pub fn stake(
        &self,
        pool_id: u32,
        address: &str,
        amount: u64,
        lock_days: u64,
    ) -> Result<(), String> {
        let mut pools = self.pools.lock().unwrap();
        let pool = pools.get_mut(&pool_id).ok_or("Pool not found")?;

        let lock_until = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + (lock_days * 86400);

        let apr = self.get_apr(pool_id);

        let stake = Stake {
            address: address.to_string(),
            amount,
            locked_until: lock_until,
            apr,
        };

        pool.stakers.insert(address.to_string(), stake);
        pool.total_staked += amount;

        Ok(())
    }

    pub fn unstake(&self, pool_id: u32, address: &str) -> Result<u64, String> {
        let mut pools = self.pools.lock().unwrap();
        let pool = pools.get_mut(&pool_id).ok_or("Pool not found")?;

        let stake = pool.stakers.remove(address).ok_or("No stake found")?;

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if current_time < stake.locked_until {
            let penalty = stake.amount / 10;
            let returned = stake.amount - penalty;
            pool.total_staked -= stake.amount;
            Ok(returned)
        } else {
            let lock_duration =
                stake.locked_until - (stake.locked_until - (stake.locked_until - current_time));
            let reward =
                (stake.amount as f64 * stake.apr * lock_duration as f64 / 365.0 / 86400.0) as u64;
            pool.total_staked -= stake.amount;
            pool.total_rewards += reward;
            Ok(stake.amount + reward)
        }
    }

    /// Slashing: معاقبة validator مخالف
    pub fn slash(&self, pool_id: u32, address: &str, slash_percent: u64) -> Result<u64, String> {
        let mut pools = self.pools.lock().unwrap();
        let pool = pools.get_mut(&pool_id).ok_or("Pool not found")?;

        let stake = pool.stakers.get_mut(address).ok_or("No stake found")?;
        let slash_amount = stake.amount * slash_percent / 100;
        stake.amount -= slash_amount;
        pool.total_staked -= slash_amount;

        println!(
            "🗡️ Slashed {}% from {}: {} tokens",
            slash_percent, address, slash_amount
        );
        Ok(slash_amount)
    }

    /// Validator reward for mining a block
    pub fn reward_validator(
        &self,
        pool_id: u32,
        address: &str,
        block_reward: u64,
    ) -> Result<(), String> {
        let mut pools = self.pools.lock().unwrap();
        let pool = pools.get_mut(&pool_id).ok_or("Pool not found")?;

        if let Some(stake) = pool.stakers.get_mut(address) {
            let reward =
                (block_reward as f64 * stake.amount as f64 / pool.total_staked as f64) as u64;
            stake.amount += reward;
            pool.total_rewards += reward;
            println!("🏆 Reward {}: {} tokens", address, reward);
        }
        Ok(())
    }

    pub fn get_stake(&self, pool_id: u32, address: &str) -> Option<Stake> {
        let pools = self.pools.lock().unwrap();
        pools.get(&pool_id)?.stakers.get(address).cloned()
    }

    pub fn get_pool_stats(&self, pool_id: u32) -> Option<(u64, u64, usize)> {
        let pools = self.pools.lock().unwrap();
        let pool = pools.get(&pool_id)?;
        Some((pool.total_staked, pool.total_rewards, pool.stakers.len()))
    }

    fn get_apr(&self, pool_id: u32) -> f64 {
        match pool_id {
            1 => 0.05,
            2 => 0.10,
            3 => 0.15,
            _ => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stake_and_unstake() {
        let staking = StakingSystem::new();
        staking.stake(1, "alice", 1000, 30).unwrap();
        let stake = staking.get_stake(1, "alice").unwrap();
        assert_eq!(stake.amount, 1000);
        let returned = staking.unstake(1, "alice").unwrap();
        assert!(returned > 0);
    }

    #[test]
    fn test_slash() {
        let staking = StakingSystem::new();
        staking.stake(1, "alice", 1000, 30).unwrap();
        let slashed = staking.slash(1, "alice", 10).unwrap();
        assert_eq!(slashed, 100);
        let stake = staking.get_stake(1, "alice").unwrap();
        assert_eq!(stake.amount, 900);
    }
}

// Strategy: Dependency Inversion for economic (Core)
// Review and adjust before applying.
