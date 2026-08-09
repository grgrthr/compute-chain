use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Miner {
    pub id: String,
    pub address: String,
    pub hash_rate: f64,
    pub total_work: u64,
    pub total_rewards: u64,
    pub last_seen: u64,
    pub status: MinerStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MinerStatus {
    Idle,
    Working,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinerStats {
    pub miner_id: String,
    pub completed_workloads: u64,
    pub failed_workloads: u64,
    pub total_compute_time_ms: u64,
    pub avg_execution_time_ms: f64,
    pub total_rewards: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinerReward {
    pub miner_id: String,
    pub workload_id: String,
    pub reward_amount: u64,
    pub timestamp: u64,
    pub proof_hash: String,
}

impl Miner {
    pub fn new(id: String, address: String) -> Self {
        Self {
            id,
            address,
            hash_rate: 0.0,
            total_work: 0,
            total_rewards: 0,
            last_seen: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            status: MinerStatus::Idle,
        }
    }

    pub fn update_stats(&mut self, execution_time_ms: u64, reward: u64) {
        self.total_work += 1;
        self.total_rewards += reward;
        self.last_seen = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let ops_per_sec = 1000.0 / execution_time_ms as f64;
        self.hash_rate = (self.hash_rate * 0.7 + ops_per_sec * 0.3).min(10000.0);
    }
}

impl MinerStats {
    pub fn new(miner_id: String) -> Self {
        Self {
            miner_id,
            completed_workloads: 0,
            failed_workloads: 0,
            total_compute_time_ms: 0,
            avg_execution_time_ms: 0.0,
            total_rewards: 0,
        }
    }

    pub fn add_success(&mut self, execution_time_ms: u64, reward: u64) {
        self.completed_workloads += 1;
        self.total_compute_time_ms += execution_time_ms;
        self.total_rewards += reward;
        self.avg_execution_time_ms =
            self.total_compute_time_ms as f64 / self.completed_workloads as f64;
    }

    pub fn add_failure(&mut self) {
        self.failed_workloads += 1;
    }
}
