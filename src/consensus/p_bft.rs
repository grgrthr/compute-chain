use crate::consensus::types::{Block, Vote, VoteType};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Represents a PBFT consensus round
#[derive(Debug, Clone, PartialEq)]
pub enum RoundState {
    /// Waiting for leader to propose
    Proposing,
    /// Validators are voting (PrePrepare + Prepare)
    Voting,
    /// Block is being committed
    Committing,
    /// Round completed successfully
    Completed,
    /// Round timed out - needs view change
    TimedOut,
}

/// PBFT Consensus Engine with Leader Rotation and View Change
pub struct PracticalBFT {
    /// List of validator IDs in order
    pub validators: Arc<Mutex<Vec<String>>>,
    /// Current round number (increments each block)
    pub current_round: Arc<Mutex<u64>>,
    /// Current view number (increments on timeout/leader failure)
    pub current_view: Arc<Mutex<u64>>,
    /// Current round state
    pub round_state: Arc<Mutex<RoundState>>,
    /// Timestamp when current round started
    pub round_start_time: Arc<Mutex<u64>>,
    /// Timeout duration in seconds
    pub timeout_seconds: Arc<Mutex<u64>>,
    /// Maximum number of faulty validators tolerated
    pub f: usize,
    /// Prepare votes for current round
    pub prepare_votes: Arc<Mutex<BTreeMap<String, Vec<Vote>>>>,
    /// Commit votes for current round
    pub commit_votes: Arc<Mutex<BTreeMap<String, Vec<Vote>>>>,
    /// Vote counts per block hash
    pub prepare_counts: Arc<Mutex<BTreeMap<String, usize>>>,
    pub commit_counts: Arc<Mutex<BTreeMap<String, usize>>>,
}

impl PracticalBFT {
    pub fn new(validators: Vec<String>) -> Self {
        let f = (validators.len() - 1) / 3;
        tracing::info!(
            "🔧 PBFT initialized with {} validators, f={}",
            validators.len(),
            f
        );

        Self {
            validators: Arc::new(Mutex::new(validators)),
            current_round: Arc::new(Mutex::new(0)),
            current_view: Arc::new(Mutex::new(0)),
            round_state: Arc::new(Mutex::new(RoundState::Proposing)),
            round_start_time: Arc::new(Mutex::new(Self::current_time())),
            timeout_seconds: Arc::new(Mutex::new(30)),
            f,
            prepare_votes: Arc::new(Mutex::new(BTreeMap::new())),
            commit_votes: Arc::new(Mutex::new(BTreeMap::new())),
            prepare_counts: Arc::new(Mutex::new(BTreeMap::new())),
            commit_counts: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    /// Get the current leader (primary) based on view and round
    pub fn get_primary(&self) -> String {
        let validators = self.validators.lock().unwrap();
        let view = *self.current_view.lock().unwrap();
        let round = *self.current_round.lock().unwrap();

        if validators.is_empty() {
            return "unknown".to_string();
        }

        let index = ((round + view) as usize) % validators.len();
        validators[index].clone()
    }

    /// Get leader for a specific round and view (uses validators lock)
    pub fn get_leader_for(&self, round: u64, view: u64) -> String {
        let validators = self.validators.lock().unwrap();
        if validators.is_empty() {
            return "unknown".to_string();
        }
        let index = ((round + view) as usize) % validators.len();
        validators[index].clone()
    }

    /// Start a new round
    pub fn start_new_round(&self) {
        let mut round = self.current_round.lock().unwrap();
        *round += 1;
        let current_round = *round;
        drop(round);

        let view = *self.current_view.lock().unwrap();
        let leader = self.get_leader_for(current_round, view);

        let mut state = self.round_state.lock().unwrap();
        *state = RoundState::Proposing;
        drop(state);

        let mut start_time = self.round_start_time.lock().unwrap();
        *start_time = Self::current_time();
        drop(start_time);

        self.prepare_votes.lock().unwrap().clear();
        self.commit_votes.lock().unwrap().clear();
        self.prepare_counts.lock().unwrap().clear();
        self.commit_counts.lock().unwrap().clear();

        tracing::info!(
            "🔄 Round {} started, leader: {}, view: {}",
            current_round,
            leader,
            view
        );
    }

    /// Check if current round has timed out
    pub fn is_timed_out(&self) -> bool {
        let start = *self.round_start_time.lock().unwrap();
        let timeout = *self.timeout_seconds.lock().unwrap();
        let now = Self::current_time();
        now - start > timeout
    }

    /// Trigger a view change (leader failed)
    pub fn trigger_view_change(&self) {
        let current_round = *self.current_round.lock().unwrap();

        let mut view = self.current_view.lock().unwrap();
        *view += 1;
        let current_view = *view;
        drop(view);

        let old_leader = self.get_leader_for(current_round, current_view - 1);
        let new_leader = self.get_leader_for(current_round, current_view);

        tracing::warn!(
            "🔄 View Change: {} -> {} (new leader: {} -> {})",
            current_view - 1,
            current_view,
            old_leader,
            new_leader
        );

        let mut state = self.round_state.lock().unwrap();
        *state = RoundState::Proposing;
        drop(state);

        let mut start_time = self.round_start_time.lock().unwrap();
        *start_time = Self::current_time();
        drop(start_time);

        self.prepare_votes.lock().unwrap().clear();
        self.commit_votes.lock().unwrap().clear();
        self.prepare_counts.lock().unwrap().clear();
        self.commit_counts.lock().unwrap().clear();
    }

    /// PrePrepare phase: leader proposes block
    pub fn pre_prepare(&self, block: &Block, primary: &str) -> bool {
        let current_leader = self.get_primary();
        if primary != current_leader {
            tracing::warn!(
                "PrePrepare rejected: {} is not current leader ({})",
                primary,
                current_leader
            );
            return false;
        }

        let mut state = self.round_state.lock().unwrap();
        *state = RoundState::Voting;
        tracing::info!(
            "📋 PrePrepare: leader {} proposed block height={}",
            primary,
            block.height
        );
        true
    }

    /// Prepare phase: validators vote on the proposed block
    pub fn prepare(&self, block_hash: &str, validator_id: &str) -> bool {
        let mut prepare_votes = self.prepare_votes.lock().unwrap();
        let votes = prepare_votes
            .entry(block_hash.to_string())
            .or_insert(Vec::new());

        if votes.iter().any(|v| v.validator_id == validator_id) {
            return false;
        }

        let vote = Vote {
            validator_id: validator_id.to_string(),
            block_hash: block_hash.to_string(),
            vote_type: VoteType::Prepare,
            signature: String::new(),
            timestamp: Self::current_time(),
        };
        votes.push(vote);

        let mut counts = self.prepare_counts.lock().unwrap();
        *counts.entry(block_hash.to_string()).or_insert(0) += 1;

        let count = *counts.get(block_hash).unwrap_or(&0);
        let required = 2 * self.f + 1;

        tracing::info!(
            "📝 Prepare vote: {} for block {} ({}/{})",
            validator_id,
            &block_hash[..12],
            count,
            required
        );

        count >= required
    }

    /// Commit phase: validators commit the block
    pub fn commit(&self, block_hash: &str, validator_id: &str) -> bool {
        let mut commit_votes = self.commit_votes.lock().unwrap();
        let votes = commit_votes
            .entry(block_hash.to_string())
            .or_insert(Vec::new());

        if votes.iter().any(|v| v.validator_id == validator_id) {
            return false;
        }

        let vote = Vote {
            validator_id: validator_id.to_string(),
            block_hash: block_hash.to_string(),
            vote_type: VoteType::Commit,
            signature: String::new(),
            timestamp: Self::current_time(),
        };
        votes.push(vote);

        let mut counts = self.commit_counts.lock().unwrap();
        *counts.entry(block_hash.to_string()).or_insert(0) += 1;

        let count = *counts.get(block_hash).unwrap_or(&0);
        let required = 2 * self.f + 1;

        tracing::info!(
            "✅ Commit vote: {} for block {} ({}/{})",
            validator_id,
            &block_hash[..12],
            count,
            required
        );

        if count >= required {
            let mut state = self.round_state.lock().unwrap();
            *state = RoundState::Completed;
        }

        count >= required
    }

    /// Check if consensus is reached for a block
    pub fn is_committed(&self, block_hash: &str) -> bool {
        let counts = self.commit_counts.lock().unwrap();
        let count = *counts.get(block_hash).unwrap_or(&0);
        count >= 2 * self.f + 1
    }

    /// Get current round info
    pub fn get_round_info(&self) -> RoundInfo {
        tracing::info!("🔍 R1 - before current_round.lock");
        let round = *self.current_round.lock().unwrap();
        tracing::info!("🔍 R2 - round={}, before current_view.lock", round);
        let view = *self.current_view.lock().unwrap();
        tracing::info!("🔍 R3 - view={}, before get_primary", view);
        let leader = self.get_primary();
        tracing::info!("🔍 R4 - leader={}, before round_state.lock", leader);
        let state = format!("{:?}", self.round_state.lock().unwrap());
        tracing::info!("🔍 R5 - state={}, before validators.lock", state);
        let validator_count = self.validators.lock().unwrap().len();
        tracing::info!("🔍 R6 - validator_count={}, returning", validator_count);

        RoundInfo {
            round,
            view,
            leader,
            state,
            validator_count,
        }
    }

    /// Adjust timeout based on network conditions
    pub fn adjust_timeout(&self, last_block_time_ms: u64) {
        let mut timeout = self.timeout_seconds.lock().unwrap();
        if last_block_time_ms < 2000 {
            *timeout = (*timeout).min(10);
        } else if last_block_time_ms > 10000 {
            *timeout = (*timeout).max(60);
        }
    }

    /// Get list of all validators
    pub fn get_validators(&self) -> Vec<String> {
        self.validators.lock().unwrap().clone()
    }

    /// Add a validator
    pub fn add_validator(&self, id: String) {
        let mut validators = self.validators.lock().unwrap();
        if !validators.contains(&id) {
            validators.push(id);
        }
    }

    /// Remove a validator
    pub fn remove_validator(&self, id: &str) {
        let mut validators = self.validators.lock().unwrap();
        validators.retain(|v| v != id);
    }

    fn current_time() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

/// Information about the current consensus round
#[derive(Debug, Clone)]
pub struct RoundInfo {
    pub round: u64,
    pub view: u64,
    pub leader: String,
    pub state: String,
    pub validator_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leader_rotation() {
        let validators = vec!["v1".to_string(), "v2".to_string(), "v3".to_string()];
        let pbft = PracticalBFT::new(validators);

        assert_eq!(pbft.get_primary(), "v1");

        pbft.start_new_round();
        assert_eq!(pbft.get_primary(), "v2");

        pbft.trigger_view_change();
        assert_eq!(pbft.get_primary(), "v3");
    }

    #[test]
    fn test_view_change() {
        let validators = vec!["v1".to_string(), "v2".to_string()];
        let pbft = PracticalBFT::new(validators);

        assert_eq!(pbft.get_primary(), "v1");
        pbft.trigger_view_change();
        assert_eq!(pbft.get_primary(), "v2");
        pbft.trigger_view_change();
        assert_eq!(pbft.get_primary(), "v1");
    }

    #[test]
    fn test_prepare_commit() {
        let validators = vec![
            "v1".to_string(),
            "v2".to_string(),
            "v3".to_string(),
            "v4".to_string(),
        ];
        let pbft = PracticalBFT::new(validators);

        assert!(!pbft.prepare("hash1", "v1"));
        assert!(!pbft.prepare("hash1", "v2"));
        assert!(pbft.prepare("hash1", "v3"));

        assert!(!pbft.commit("hash1", "v1"));
        assert!(!pbft.commit("hash1", "v2"));
        assert!(pbft.commit("hash1", "v3"));

        assert!(pbft.is_committed("hash1"));
    }

    #[test]
    fn test_timeout_detection() {
        let validators = vec!["v1".to_string()];
        let pbft = PracticalBFT::new(validators);

        *pbft.timeout_seconds.lock().unwrap() = 0;
        {
            let mut start = pbft.round_start_time.lock().unwrap();
            *start = start.saturating_sub(10);
        }
        assert!(pbft.is_timed_out());
    }

    #[test]
    fn test_duplicate_vote_prevention() {
        let validators = vec![
            "v1".to_string(),
            "v2".to_string(),
            "v3".to_string(),
            "v4".to_string(),
        ];
        let pbft = PracticalBFT::new(validators);

        assert!(!pbft.prepare("hash1", "v1"));
        assert!(!pbft.prepare("hash1", "v1"));

        let counts = pbft.prepare_counts.lock().unwrap();
        assert_eq!(*counts.get("hash1").unwrap(), 1);
    }
}

// Strategy: Dependency Inversion for consensus (Core)
// Review and adjust before applying.
