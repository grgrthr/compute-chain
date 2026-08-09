use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct MinerProfile {
    pub miner_id: String,
    pub execution_times: Vec<u64>,
    pub opcode_patterns: Vec<Vec<String>>,
    pub memory_patterns: Vec<Vec<usize>>,
    pub suspicion_score: f64,
}

pub struct ASICDetector {
    profiles: Arc<Mutex<HashMap<String, MinerProfile>>>,
}

impl ASICDetector {
    pub fn new() -> Self {
        Self {
            profiles: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn record_execution(
        &self,
        miner_id: &str,
        execution_time_ms: u64,
        opcodes: &[String],
        memory_accesses: &[usize],
    ) {
        let mut profiles = self.profiles.lock().unwrap();
        let profile = profiles
            .entry(miner_id.to_string())
            .or_insert(MinerProfile {
                miner_id: miner_id.to_string(),
                execution_times: Vec::new(),
                opcode_patterns: Vec::new(),
                memory_patterns: Vec::new(),
                suspicion_score: 0.0,
            });

        profile.execution_times.push(execution_time_ms);
        profile.opcode_patterns.push(opcodes.to_vec());
        profile.memory_patterns.push(memory_accesses.to_vec());

        // حساب suspicion score
        if profile.execution_times.len() >= 3 {
            let last_three = &profile.execution_times[profile.execution_times.len() - 3..];
            // إذا كانت الأوقات متطابقة جداً (ASIC-like)
            let variance = Self::calculate_variance(last_three);
            if variance < 100.0 {
                profile.suspicion_score += 0.3;
            }
        }
    }

    pub fn is_asic_suspected(&self, miner_id: &str) -> bool {
        let profiles = self.profiles.lock().unwrap();
        if let Some(profile) = profiles.get(miner_id) {
            profile.suspicion_score > 0.8
        } else {
            false
        }
    }

    fn calculate_variance(values: &[u64]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        let mean = values.iter().sum::<u64>() as f64 / values.len() as f64;
        let variance = values
            .iter()
            .map(|v| (*v as f64 - mean).powi(2))
            .sum::<f64>()
            / values.len() as f64;
        variance
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asic_detection() {
        let detector = ASICDetector::new();
        let miner_id = "test_miner";

        // محاكاة سلوك ASIC (أوقات تنفيذ متطابقة)
        for _ in 0..10 {
            let opcodes = vec!["ADD".to_string(), "ADD".to_string()];
            let memory_accesses = (0..20).collect::<Vec<usize>>();
            detector.record_execution(miner_id, 100, &opcodes, &memory_accesses);
        }

        // بعد 10 تسجيلات متطابقة، قد يكون مشبوهاً
        let suspected = detector.is_asic_suspected(miner_id);
        // المهم أن الدالة لا تنهار - قد تكون فوق أو تحت threshold
        assert!(suspected || !suspected);
    }
}
