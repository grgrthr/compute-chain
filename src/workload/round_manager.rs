//! Round Manager — Deterministic round progression engine.
//!
//! Every node independently agrees on current round, seed, and difficulty.
//! Integrates with NetworkSeed, WorkloadGenerator, and Scheduler.

use crate::workload::generator::WorkloadGenerator;
use crate::workload::network_seed::{NetworkSeed, RoundMetadata};
use std::collections::HashSet;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Configuration for the round manager.
#[derive(Debug, Clone, PartialEq)]
pub struct RoundConfig {
    /// Duration of each round in seconds.
    pub round_duration_secs: u64,
    /// Initial difficulty.
    pub initial_difficulty: u32,
    /// Maximum allowed rounds ahead of current (reject future rounds).
    pub max_round_ahead: u64,
}

impl Default for RoundConfig {
    fn default() -> Self {
        RoundConfig {
            round_duration_secs: 60,
            initial_difficulty: 3,
            max_round_ahead: 1,
        }
    }
}

/// The current round state managed by each node.
#[derive(Debug, Clone, PartialEq)]
pub struct RoundState {
    /// Current round number.
    pub current_round: u64,
    /// The network seed for this round.
    pub seed: NetworkSeed,
    /// Current difficulty.
    pub difficulty: u32,
    /// When this round started (wall clock).
    pub started_at: u64,
    /// Active workers in this round.
    pub active_workers: Vec<String>,
    /// Whether this round has been finalized.
    pub finalized: bool,
}

/// Manages deterministic round progression.
pub struct RoundManager {
    config: RoundConfig,
    current: RoundState,
    previous_seeds: Vec<String>,
    finalized_rounds: HashSet<u64>,
}

impl RoundManager {
    /// Create a new RoundManager starting at genesis (round 0).
    pub fn new(config: RoundConfig) -> Self {
        let genesis_seed = NetworkSeed::genesis();
        RoundManager {
            config: config.clone(),
            current: RoundState {
                current_round: 0,
                seed: genesis_seed,
                difficulty: config.initial_difficulty,
                started_at: now(),
                active_workers: vec![],
                finalized: false,
            },
            previous_seeds: vec![NetworkSeed::GENESIS.to_string()],
            finalized_rounds: HashSet::new(),
        }
    }

    /// Create a RoundManager starting from a specific round.
    pub fn from_round(round: u64, previous_seed: &str, difficulty: u32) -> Self {
        let seed = NetworkSeed::compute(&RoundMetadata {
            previous_seed: previous_seed.into(),
            previous_block_hash: String::new(),
            current_round: round,
            active_workers_hash: String::new(),
            difficulty,
        });
        RoundManager {
            config: RoundConfig::default(),
            current: RoundState {
                current_round: round,
                seed,
                difficulty,
                started_at: now(),
                active_workers: vec![],
                finalized: false,
            },
            previous_seeds: vec![NetworkSeed::GENESIS.to_string(), previous_seed.to_string()],
            finalized_rounds: HashSet::new(),
        }
    }

    /// Get the current round number.
    pub fn current_round(&self) -> u64 {
        self.current.current_round
    }

    /// Get the current network seed.
    pub fn current_seed(&self) -> &NetworkSeed {
        &self.current.seed
    }

    /// Get the current difficulty.
    pub fn current_difficulty(&self) -> u32 {
        self.current.difficulty
    }

    /// Check if the round has exceeded its duration.
    pub fn is_round_expired(&self) -> bool {
        let elapsed = now().saturating_sub(self.current.started_at);
        elapsed > self.config.round_duration_secs
    }

    /// Check if a given round number is valid (not stale, not future).
    pub fn is_valid_round(&self, round: u64) -> bool {
        if round < self.current.current_round {
            return false; // Stale round
        }
        if round > self.current.current_round + self.config.max_round_ahead {
            return false; // Too far in the future
        }
        true
    }

    /// Check if a round has already been finalized.
    pub fn is_duplicate_round(&self, round: u64) -> bool {
        self.finalized_rounds.contains(&round)
    }

    /// Update active workers for the current round.
    pub fn update_workers(&mut self, workers: Vec<String>) {
        self.current.active_workers = workers;
    }

    /// Update difficulty for the current round.
    pub fn update_difficulty(&mut self, difficulty: u32) {
        self.current.difficulty = difficulty;
    }

    /// Finalize the current round and advance to the next one.
    /// Returns the new RoundState.
    pub fn finalize_and_advance(
        &mut self,
        previous_block_hash: &str,
        active_workers: &[String],
    ) -> Result<RoundState, String> {
        if self.current.finalized {
            return Err("Round already finalized".into());
        }
        if !self.is_round_expired() {
            // Round hasn't expired yet - still allowed to finalize early
        }

        let round = self.current.current_round;
        if self.is_duplicate_round(round) {
            return Err(format!("Round {} already finalized", round));
        }

        // Mark current as finalized
        self.finalized_rounds.insert(round);
        self.current.finalized = true;
        self.previous_seeds.push(self.current.seed.seed.clone());

        // Compute next round
        let next_round = round + 1;
        let next_seed = NetworkSeed::next_round(
            &self.current.seed.seed,
            previous_block_hash,
            next_round,
            active_workers,
            self.current.difficulty,
        );

        // Advance
        let new_state = RoundState {
            current_round: next_round,
            seed: next_seed,
            difficulty: self.current.difficulty,
            started_at: now(),
            active_workers: active_workers.to_vec(),
            finalized: false,
        };

        self.current = new_state.clone();
        Ok(new_state)
    }

    /// Get the workload for the current round.
    /// Every node calling this with the same state produces the same program.
    pub fn current_workload(&self) -> crate::workload::generator::Workload {
        let config = self
            .current
            .seed
            .to_workload_config(self.current.current_round);
        WorkloadGenerator::generate(&config)
    }

    /// Get the current round metadata (can be broadcast to other nodes).
    pub fn current_metadata(&self) -> RoundMetadata {
        RoundMetadata {
            previous_seed: self.previous_seeds.last().cloned().unwrap_or_default(),
            previous_block_hash: String::new(),
            current_round: self.current.current_round,
            active_workers_hash: NetworkSeed::hash_worker_set(&self.current.active_workers),
            difficulty: self.current.difficulty,
        }
    }

    /// Synchronize to a metadata broadcast (from scheduler).
    /// Only updates if the metadata is for a valid future round.
    pub fn sync_to_metadata(&mut self, metadata: &RoundMetadata) -> Result<(), String> {
        if !self.is_valid_round(metadata.current_round) {
            return Err(format!(
                "Invalid round {} (current: {})",
                metadata.current_round, self.current.current_round
            ));
        }
        if self.is_duplicate_round(metadata.current_round) {
            return Err(format!(
                "Round {} already finalized",
                metadata.current_round
            ));
        }

        let seed = NetworkSeed::compute(metadata);
        self.current = RoundState {
            current_round: metadata.current_round,
            seed,
            difficulty: metadata.difficulty,
            started_at: now(),
            active_workers: vec![],
            finalized: false,
        };
        Ok(())
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_round() {
        let rm = RoundManager::new(RoundConfig::default());
        assert_eq!(rm.current_round(), 0);
        assert!(!rm.current_seed().seed.is_empty());
    }

    #[test]
    fn test_round_increment() {
        let mut rm = RoundManager::new(RoundConfig::default());
        assert_eq!(rm.current_round(), 0);

        rm.finalize_and_advance("block-hash-1", &["w1".into()])
            .unwrap();
        assert_eq!(rm.current_round(), 1);
        assert_ne!(rm.current_seed().seed, NetworkSeed::genesis().seed);
    }

    #[test]
    fn test_same_nodes_derive_identical_next_round() {
        let mut rm_a = RoundManager::new(RoundConfig::default());
        let mut rm_b = RoundManager::new(RoundConfig::default());

        rm_a.finalize_and_advance("hash1", &["w1".into(), "w2".into()])
            .unwrap();
        rm_b.finalize_and_advance("hash1", &["w1".into(), "w2".into()])
            .unwrap();

        assert_eq!(rm_a.current_round(), rm_b.current_round());
        assert_eq!(rm_a.current_seed().seed, rm_b.current_seed().seed);
    }

    #[test]
    fn test_stale_round_rejected() {
        let mut rm = RoundManager::new(RoundConfig::default());
        rm.finalize_and_advance("h1", &[]).unwrap(); // round 1
        rm.finalize_and_advance("h2", &[]).unwrap(); // round 2

        // Round 1 is stale (current is 2)
        assert!(!rm.is_valid_round(1));
    }

    #[test]
    fn test_future_round_rejected() {
        let rm = RoundManager::new(RoundConfig::default());
        // Default max_round_ahead is 1, so round 5 is too far
        assert!(!rm.is_valid_round(5));
    }

    #[test]
    fn test_duplicate_round_rejected() {
        let mut rm = RoundManager::new(RoundConfig::default());
        rm.finalize_and_advance("h1", &[]).unwrap();
        // Already finalized round 0
        assert!(rm.is_duplicate_round(0));
    }

    #[test]
    fn test_metadata_deterministic() {
        let mut rm = RoundManager::new(RoundConfig::default());
        rm.update_workers(vec!["a".into(), "b".into()]);

        let m1 = rm.current_metadata();
        let m2 = rm.current_metadata();
        assert_eq!(m1.current_round, m2.current_round);
        assert_eq!(m1.active_workers_hash, m2.active_workers_hash);
    }

    #[test]
    fn test_multiple_consecutive_rounds() {
        let mut rm = RoundManager::new(RoundConfig::default());

        for i in 1..=5 {
            rm.finalize_and_advance(&format!("block-{}", i), &["w1".into()])
                .unwrap();
            assert_eq!(rm.current_round(), i as u64);
            assert!(!rm.current_seed().seed.is_empty());
        }
    }

    #[test]
    fn test_workload_after_round_transition() {
        let mut rm = RoundManager::new(RoundConfig::default());

        let wl1 = rm.current_workload();
        assert!(!wl1.instructions.is_empty());

        rm.finalize_and_advance("block-1", &["w1".into()]).unwrap();
        let wl2 = rm.current_workload();

        // Different rounds produce different workloads
        assert_ne!(wl1.seed_hash, wl2.seed_hash);
    }

    #[test]
    fn test_proofs_valid_after_round_transition() {
        let mut rm = RoundManager::new(RoundConfig::default());

        for round in 0..3 {
            let wl = rm.current_workload();
            let mut runner = crate::integration::runner::IntegrationRunner::new();
            let result = runner.run(&format!("round_{}", round), wl.instructions);

            assert!(result.success, "Round {} failed: {:?}", round, result.error);
            assert!(result.proof_generated);
            assert!(result.proof_verified);

            rm.finalize_and_advance(&format!("block-{}", round), &["w1".into()])
                .unwrap();
        }
    }

    #[test]
    fn test_sync_to_metadata() {
        let mut rm = RoundManager::new(RoundConfig::default());
        rm.finalize_and_advance("h1", &[]).unwrap(); // round 1

        let metadata = RoundMetadata {
            previous_seed: rm.current_seed().seed.clone(),
            previous_block_hash: "h2".into(),
            current_round: 2,
            active_workers_hash: String::new(),
            difficulty: 5,
        };

        rm.sync_to_metadata(&metadata).unwrap();
        assert_eq!(rm.current_round(), 2);
        assert_eq!(rm.current_difficulty(), 5);
    }
}
