use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeConfig {
    pub base_fee: u64,
    pub compute_fee_per_unit: u64,
    pub memory_fee_per_kb: u64,
    pub priority_fee_multiplier: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeEstimate {
    pub total: u64,
    pub base: u64,
    pub compute: u64,
    pub memory: u64,
    pub priority: u64,
}

pub struct FeeManager {
    config: Arc<Mutex<FeeConfig>>,
    collected_fees: Arc<Mutex<u64>>,
    fee_history: Arc<Mutex<Vec<(u64, u64)>>>,
}

impl FeeManager {
    pub fn new() -> Self {
        Self {
            config: Arc::new(Mutex::new(FeeConfig {
                base_fee: 10,
                compute_fee_per_unit: 1,
                memory_fee_per_kb: 5,
                priority_fee_multiplier: 1.0,
            })),
            collected_fees: Arc::new(Mutex::new(0)),
            fee_history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn calculate_fee(&self, compute_units: u64, memory_kb: u64, priority: u32) -> FeeEstimate {
        let config = self.config.lock().unwrap();

        let base = config.base_fee;
        let compute = compute_units * config.compute_fee_per_unit;
        let memory = (memory_kb / 1024) * config.memory_fee_per_kb;
        let priority_multiplier = 1.0 + (priority as f64 / 100.0);

        let priority_fee = ((base + compute + memory) as f64 * (priority_multiplier - 1.0)) as u64;
        let total = ((base + compute + memory) as f64 * priority_multiplier) as u64;

        FeeEstimate {
            total,
            base,
            compute,
            memory,
            priority: priority_fee,
        }
    }

    pub fn collect_fee(&self, amount: u64) {
        let mut collected = self.collected_fees.lock().unwrap();
        *collected += amount;

        let mut history = self.fee_history.lock().unwrap();
        history.push((Self::current_time(), amount));

        if history.len() > 100 {
            history.remove(0);
        }
    }

    pub fn get_collected_fees(&self) -> u64 {
        *self.collected_fees.lock().unwrap()
    }

    pub fn adjust_fees(&self, network_load: f64) {
        let mut config = self.config.lock().unwrap();

        if network_load > 0.8 {
            config.base_fee = (config.base_fee as f64 * 1.2) as u64;
            config.priority_fee_multiplier *= 1.1;
        } else if network_load < 0.3 {
            config.base_fee = (config.base_fee as f64 * 0.9) as u64;
            config.priority_fee_multiplier *= 0.95;
        }

        config.base_fee = config.base_fee.max(1).min(1000);
        config.priority_fee_multiplier = config.priority_fee_multiplier.max(1.0).min(10.0);
    }

    pub fn get_current_config(&self) -> FeeConfig {
        self.config.lock().unwrap().clone()
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
    fn test_calculate_fee() {
        let fees = FeeManager::new();
        let estimate = fees.calculate_fee(1000, 1024, 10);
        assert!(estimate.total > 0);
    }

    #[test]
    fn test_collect_fee() {
        let fees = FeeManager::new();
        fees.collect_fee(100);
        assert_eq!(fees.get_collected_fees(), 100);
    }
}

// Strategy: Dependency Inversion for economic (Core)
// Review and adjust before applying.
