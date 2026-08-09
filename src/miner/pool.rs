use crate::miner::gpu::{GpuEstimate, GpuManager, GpuWorkload};
use crate::miner::rewards::RewardSystem;
use crate::miner::types::{Miner, MinerReward, MinerStats};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct MinerPool {
    miners: Arc<Mutex<HashMap<String, Miner>>>,
    stats: Arc<Mutex<HashMap<String, MinerStats>>>,
    rewards: Arc<Mutex<RewardSystem>>,
    reward_history: Arc<Mutex<Vec<MinerReward>>>,
    gpu_manager: Arc<Mutex<GpuManager>>,
    total_network_rewards: Arc<Mutex<u64>>,
}

impl MinerPool {
    pub fn new() -> Self {
        Self {
            miners: Arc::new(Mutex::new(HashMap::new())),
            stats: Arc::new(Mutex::new(HashMap::new())),
            rewards: Arc::new(Mutex::new(RewardSystem::new())),
            reward_history: Arc::new(Mutex::new(Vec::new())),
            gpu_manager: Arc::new(Mutex::new(GpuManager::new())),
            total_network_rewards: Arc::new(Mutex::new(0)),
        }
    }

    /// تسجيل معدّن جديد
    pub fn register_miner(&self, id: String, address: String) -> String {
        let miner = Miner::new(id.clone(), address);
        let stats = MinerStats::new(id.clone());

        self.miners.lock().unwrap().insert(id.clone(), miner);
        self.stats.lock().unwrap().insert(id.clone(), stats);

        println!("Miner registered: {}", id);
        id
    }

    /// الحصول على معدّن
    pub fn get_miner(&self, id: &str) -> Option<Miner> {
        self.miners.lock().unwrap().get(id).cloned()
    }

    /// تنفيذ workload عبر GPU/CPU وتوزيع المكافأة
    pub fn execute_workload(
        &self,
        miner_id: &str,
        workload: &GpuWorkload,
        market_fee: u64,
    ) -> Result<(MinerReward, u64), String> {
        // تنفيذ العمل
        let gpu = self.gpu_manager.lock().unwrap();
        let result = gpu
            .execute(workload)
            .map_err(|e| format!("GPU execution failed: {}", e))?;

        let execution_time_ms = result.execution_time_us / 1000;
        let difficulty = workload.instructions.len() as u32;

        // احتساب المكافأة (أساسية + رسوم السوق)
        let reward = {
            let rewards = self.rewards.lock().unwrap();
            let base = rewards.calculate_reward(difficulty, execution_time_ms, true);
            base + market_fee
        };

        // تحديث إحصائيات المعدّن
        let workload_id = uuid::Uuid::new_v4().to_string();
        let miner_reward = MinerReward {
            miner_id: miner_id.to_string(),
            workload_id: workload_id.clone(),
            reward_amount: reward,
            timestamp: Self::current_time(),
            proof_hash: "pending".into(),
        };

        {
            let mut stats_map = self.stats.lock().unwrap();
            if let Some(stats) = stats_map.get_mut(miner_id) {
                stats.add_success(execution_time_ms, reward);
            }
        }

        {
            let mut miners = self.miners.lock().unwrap();
            if let Some(miner) = miners.get_mut(miner_id) {
                miner.update_stats(execution_time_ms, reward);
            }
        }

        // تسجيل في السجل
        {
            let mut history = self.reward_history.lock().unwrap();
            history.push(miner_reward.clone());
        }

        {
            let mut total = self.total_network_rewards.lock().unwrap();
            *total += reward;
        }

        println!(
            "Workload executed by {}: {} instructions, reward={}, time={}ms",
            miner_id,
            workload.instructions.len(),
            reward,
            execution_time_ms
        );

        Ok((miner_reward, result.output[0]))
    }

    /// تنفيذ مع إثبات (للتعدين الحقيقي)
    pub fn execute_with_proof(
        &self,
        miner_id: &str,
        workload: &GpuWorkload,
        proof_valid: bool,
        market_fee: u64,
    ) -> Result<MinerReward, String> {
        let gpu = self.gpu_manager.lock().unwrap();
        let result = gpu
            .execute(workload)
            .map_err(|e| format!("Execution failed: {}", e))?;

        let execution_time_ms = result.execution_time_us / 1000;
        let difficulty = workload.instructions.len() as u32;

        let reward = {
            let rewards = self.rewards.lock().unwrap();
            if proof_valid {
                let base = rewards.calculate_reward(difficulty, execution_time_ms, true);
                base + market_fee
            } else {
                0
            }
        };

        let workload_id = uuid::Uuid::new_v4().to_string();
        let miner_reward = MinerReward {
            miner_id: miner_id.to_string(),
            workload_id,
            reward_amount: reward,
            timestamp: Self::current_time(),
            proof_hash: "verified".into(),
        };

        {
            let mut stats_map = self.stats.lock().unwrap();
            if let Some(stats) = stats_map.get_mut(miner_id) {
                if proof_valid {
                    stats.add_success(execution_time_ms, reward);
                } else {
                    stats.add_failure();
                }
            }
        }

        {
            let mut history = self.reward_history.lock().unwrap();
            history.push(miner_reward.clone());
        }

        if reward > 0 {
            let mut total = self.total_network_rewards.lock().unwrap();
            *total += reward;
        }

        Ok(miner_reward)
    }

    /// معدل الهاش للشبكة
    pub fn get_network_hash_rate(&self) -> f64 {
        let miners = self.miners.lock().unwrap();
        miners.values().map(|m| m.hash_rate).sum()
    }

    /// إحصائيات معدّن
    pub fn get_miner_stats(&self, miner_id: &str) -> Option<MinerStats> {
        self.stats.lock().unwrap().get(miner_id).cloned()
    }

    /// قائمة المعدّنين
    pub fn list_miners(&self) -> Vec<Miner> {
        self.miners.lock().unwrap().values().cloned().collect()
    }

    /// سجل المكافآت
    pub fn get_reward_history(&self) -> Vec<MinerReward> {
        self.reward_history.lock().unwrap().clone()
    }

    /// سجل مكافآت معدّن محدد
    pub fn get_miner_rewards(&self, miner_id: &str) -> Vec<MinerReward> {
        self.reward_history
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.miner_id == miner_id)
            .cloned()
            .collect()
    }

    /// إجمالي مكافآت الشبكة
    pub fn get_total_network_rewards(&self) -> u64 {
        *self.total_network_rewards.lock().unwrap()
    }

    /// إحصائيات الـ pool كاملة
    pub fn get_pool_stats(&self) -> PoolStats {
        let miners = self.miners.lock().unwrap();
        let history = self.reward_history.lock().unwrap();
        let total_rewards = *self.total_network_rewards.lock().unwrap();

        PoolStats {
            total_miners: miners.len() as u64,
            active_miners: miners
                .values()
                .filter(|m| m.status == crate::miner::types::MinerStatus::Working)
                .count() as u64,
            network_hash_rate: miners.values().map(|m| m.hash_rate).sum(),
            total_rewards_distributed: total_rewards,
            total_workloads_completed: history.len() as u64,
            gpu_count: self.gpu_manager.lock().unwrap().device_count() as u64,
        }
    }

    /// تقدير أداء GPU لـ workload
    pub fn estimate_gpu_performance(&self, instruction_count: usize) -> GpuEstimate {
        self.gpu_manager
            .lock()
            .unwrap()
            .estimate_performance(instruction_count)
    }

    /// تعديل صعوبة الشبكة
    pub fn adjust_global_difficulty(&self) {
        let hash_rate = self.get_network_hash_rate();
        self.rewards
            .lock()
            .unwrap()
            .adjust_difficulty_multiplier(hash_rate);
    }

    /// تحديث إحصائيات معدّن (متوافق مع API القديم)
    pub fn update_miner_stats(
        &self,
        miner_id: &str,
        execution_time_ms: u64,
        difficulty: u32,
        proof_valid: bool,
    ) -> u64 {
        let reward = {
            let mut stats_map = self.stats.lock().unwrap();
            if let Some(stats) = stats_map.get_mut(miner_id) {
                self.rewards.lock().unwrap().distribute_reward(
                    stats,
                    difficulty,
                    execution_time_ms,
                    proof_valid,
                )
            } else {
                0
            }
        };

        if reward > 0 {
            let mut miners = self.miners.lock().unwrap();
            if let Some(miner) = miners.get_mut(miner_id) {
                miner.update_stats(execution_time_ms, reward);
            }
        }

        reward
    }

    fn current_time() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStats {
    pub total_miners: u64,
    pub active_miners: u64,
    pub network_hash_rate: f64,
    pub total_rewards_distributed: u64,
    pub total_workloads_completed: u64,
    pub gpu_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::miner::gpu::GpuInstruction;

    #[test]
    fn test_register_miner() {
        let pool = MinerPool::new();
        let id = pool.register_miner("miner1".to_string(), "0x123".to_string());
        assert_eq!(id, "miner1");
        assert!(pool.get_miner("miner1").is_some());
    }

    #[test]
    fn test_execute_workload() {
        let pool = MinerPool::new();
        pool.register_miner("miner1".to_string(), "0x123".to_string());

        let workload = GpuWorkload {
            instructions: vec![
                GpuInstruction {
                    opcode: 0,
                    src1: 0,
                    src2: 1,
                    dst: 2,
                },
                GpuInstruction {
                    opcode: 1,
                    src1: 2,
                    src2: 0,
                    dst: 3,
                },
            ],
            input_data: vec![10, 5, 0, 0],
            expected_output_size: 4,
        };

        let result = pool.execute_workload("miner1", &workload, 10);
        assert!(result.is_ok());

        let stats = pool.get_miner_stats("miner1").unwrap();
        assert_eq!(stats.completed_workloads, 1);
        assert!(stats.total_rewards > 0);
    }

    #[test]
    fn test_pool_stats() {
        let pool = MinerPool::new();
        pool.register_miner("m1".to_string(), "0x1".to_string());
        pool.register_miner("m2".to_string(), "0x2".to_string());

        let stats = pool.get_pool_stats();
        assert_eq!(stats.total_miners, 2);
    }
}
