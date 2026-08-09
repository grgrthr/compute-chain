use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResistanceConfig {
    pub memory_pressure: bool,
    pub dynamic_opcodes: bool,
    pub random_jumps: bool,
    pub variable_difficulty: bool,
    pub mutation_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ASICMetrics {
    pub hash_rate_variance: f64,
    pub memory_pattern_score: f64,
    pub opcode_distribution: Vec<(String, f64)>,
    pub suspected_asics: Vec<String>,
}

pub struct ASICResistance {
    config: Arc<Mutex<ResistanceConfig>>,
    metrics: Arc<Mutex<ASICMetrics>>,
    history: Arc<Mutex<VecDeque<ASICMetrics>>>,
}

impl ASICResistance {
    pub fn new() -> Self {
        Self {
            config: Arc::new(Mutex::new(ResistanceConfig {
                memory_pressure: true,
                dynamic_opcodes: true,
                random_jumps: true,
                variable_difficulty: true,
                mutation_rate: 0.3,
            })),
            metrics: Arc::new(Mutex::new(ASICMetrics {
                hash_rate_variance: 0.0,
                memory_pattern_score: 0.0,
                opcode_distribution: Vec::new(),
                suspected_asics: Vec::new(),
            })),
            history: Arc::new(Mutex::new(VecDeque::with_capacity(100))),
        }
    }

    /// Adaptive Mutation - يشغل الطفرات إذا اكتشف ASIC
    pub fn adaptive_mutation(&self) -> f64 {
        let metrics = self.metrics.lock().unwrap();
        let config = self.config.lock().unwrap();

        if !metrics.suspected_asics.is_empty() {
            println!("🛡️ ASIC detected! Increasing mutation rate");
            (config.mutation_rate * 1.5).min(1.0)
        } else {
            config.mutation_rate * 0.5
        }
    }

    pub fn update_metrics(
        &self,
        execution_times: &[u64],
        opcodes: &[String],
        memory_accesses: &[usize],
    ) {
        let mut metrics = self.metrics.lock().unwrap();

        if execution_times.len() > 1 {
            let mean: f64 =
                execution_times.iter().sum::<u64>() as f64 / execution_times.len() as f64;
            let variance: f64 = execution_times
                .iter()
                .map(|&t| (t as f64 - mean).powi(2))
                .sum::<f64>()
                / execution_times.len() as f64;
            metrics.hash_rate_variance = variance / mean;
        }

        let mut dist = std::collections::HashMap::new();
        for op in opcodes {
            *dist.entry(op.clone()).or_insert(0) += 1;
        }
        metrics.opcode_distribution = dist
            .into_iter()
            .map(|(op, count)| (op, count as f64 / opcodes.len() as f64))
            .collect();

        if !memory_accesses.is_empty() {
            let pattern_score = Self::detect_memory_pattern(memory_accesses);
            metrics.memory_pattern_score = pattern_score;
        }

        // Detect ASICs inline to avoid deadlock (was calling self.detect_asics which locks again)
        {
            let mut suspected = Vec::new();
            if metrics.hash_rate_variance < 0.1 {
                suspected.push("low_variance".to_string());
            }
            if metrics.memory_pattern_score > 0.7 {
                suspected.push("memory_pattern".to_string());
            }
            for (op, ratio) in &metrics.opcode_distribution {
                if *ratio > 0.6 {
                    suspected.push(format!("opcode_dominance_{}", op));
                }
            }
            metrics.suspected_asics = suspected;
        }

        let metrics_clone = metrics.clone();
        drop(metrics);

        let mut history = self.history.lock().unwrap();
        history.push_back(metrics_clone);
        if history.len() > 100 {
            history.pop_front();
        }
    }

    fn detect_memory_pattern(accesses: &[usize]) -> f64 {
        let mut diffs = Vec::new();
        for i in 1..accesses.len().min(100) {
            diffs.push((accesses[i] as i64 - accesses[i - 1] as i64).abs());
        }

        if diffs.is_empty() {
            return 0.0;
        }

        let mean_diff: f64 = diffs.iter().sum::<i64>() as f64 / diffs.len() as f64;
        let variance: f64 = diffs
            .iter()
            .map(|&d| (d as f64 - mean_diff).powi(2))
            .sum::<f64>()
            / diffs.len() as f64;

        if variance < 10.0 {
            (mean_diff / 100.0).min(1.0)
        } else {
            0.0
        }
    }

    pub fn should_trigger_mutation(&self) -> bool {
        let metrics = self.metrics.lock().unwrap();
        let config = self.config.lock().unwrap();
        !metrics.suspected_asics.is_empty()
            && config.mutation_rate > rand::thread_rng().gen::<f64>()
    }

    pub fn get_config(&self) -> ResistanceConfig {
        self.config.lock().unwrap().clone()
    }

    pub fn update_config(&self, new_config: ResistanceConfig) {
        let mut config = self.config.lock().unwrap();
        *config = new_config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asic_detection() {
        let resistance = ASICResistance::new();

        // Set mutation_rate to 1.0 to guarantee deterministic trigger in test
        {
            let mut config = resistance.config.lock().unwrap();
            config.mutation_rate = 1.0;
        }

        let execution_times = vec![100, 101, 100, 102, 100];
        let opcodes = vec!["ADD".to_string(), "ADD".to_string(), "ADD".to_string()];
        let memory_accesses = vec![0, 1, 2, 3, 4, 5];
        resistance.update_metrics(&execution_times, &opcodes, &memory_accesses);
        let should_mutate = resistance.should_trigger_mutation();
        assert!(should_mutate);
    }

    #[test]
    fn test_adaptive_mutation() {
        let resistance = ASICResistance::new();
        let rate = resistance.adaptive_mutation();
        assert!(rate >= 0.0 && rate <= 1.0);
    }
}
