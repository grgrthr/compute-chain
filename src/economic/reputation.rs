use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationScore {
    pub address: String,
    pub score: f64, // 0 - 1000
    pub completed_work: u64,
    pub failed_work: u64,
    pub avg_response_time_ms: u64,
    pub total_rewards: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationEvent {
    pub address: String,
    pub event_type: ReputationEventType,
    pub delta: f64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ReputationEventType {
    WorkCompleted,
    WorkFailed,
    ProofValid,
    ProofInvalid,
    EarlyUnstake,
    LongStake,
}

pub struct ReputationSystem {
    scores: Arc<Mutex<HashMap<String, ReputationScore>>>,
    events: Arc<Mutex<Vec<ReputationEvent>>>,
}

impl ReputationSystem {
    pub fn new() -> Self {
        Self {
            scores: Arc::new(Mutex::new(HashMap::new())),
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn get_score(&self, address: &str) -> f64 {
        let scores = self.scores.lock().unwrap();
        scores.get(address).map(|s| s.score).unwrap_or(500.0)
    }

    pub fn update_reputation(&self, address: &str, event: ReputationEventType) {
        let mut scores = self.scores.lock().unwrap();
        let score = scores
            .entry(address.to_string())
            .or_insert(ReputationScore {
                address: address.to_string(),
                score: 500.0,
                completed_work: 0,
                failed_work: 0,
                avg_response_time_ms: 0,
                total_rewards: 0,
            });

        let delta = match event {
            ReputationEventType::WorkCompleted => 10.0,
            ReputationEventType::WorkFailed => -20.0,
            ReputationEventType::ProofValid => 5.0,
            ReputationEventType::ProofInvalid => -15.0,
            ReputationEventType::EarlyUnstake => -50.0,
            ReputationEventType::LongStake => 20.0,
        };

        score.score += delta;
        score.score = score.score.max(0.0).min(1000.0);

        match event {
            ReputationEventType::WorkCompleted => score.completed_work += 1,
            ReputationEventType::WorkFailed => score.failed_work += 1,
            _ => {}
        }

        let mut events = self.events.lock().unwrap();
        events.push(ReputationEvent {
            address: address.to_string(),
            event_type: event,
            delta,
            timestamp: Self::current_time(),
        });

        if events.len() > 1000 {
            events.remove(0);
        }
    }

    pub fn update_response_time(&self, address: &str, response_time_ms: u64) {
        let mut scores = self.scores.lock().unwrap();
        if let Some(score) = scores.get_mut(address) {
            let total_responses = score.completed_work + score.failed_work;
            if total_responses > 0 {
                score.avg_response_time_ms = (score.avg_response_time_ms * (total_responses - 1)
                    + response_time_ms)
                    / total_responses;
            } else {
                score.avg_response_time_ms = response_time_ms;
            }
        }
    }

    pub fn update_rewards(&self, address: &str, reward: u64) {
        let mut scores = self.scores.lock().unwrap();
        if let Some(score) = scores.get_mut(address) {
            score.total_rewards += reward;
        }
    }

    pub fn get_leaderboard(&self, limit: usize) -> Vec<ReputationScore> {
        let mut scores: Vec<ReputationScore> =
            self.scores.lock().unwrap().values().cloned().collect();

        scores.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        scores.truncate(limit);
        scores
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
    fn test_reputation_changes() {
        let rep = ReputationSystem::new();

        rep.update_reputation("alice", ReputationEventType::WorkCompleted);
        assert_eq!(rep.get_score("alice"), 510.0);

        rep.update_reputation("alice", ReputationEventType::WorkFailed);
        assert_eq!(rep.get_score("alice"), 490.0);
    }
}

// Strategy: Dependency Inversion for economic (Core)
// Review and adjust before applying.
