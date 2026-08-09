use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct Delegate {
    pub id: String,
    pub address: String,
    pub votes: u64,
    pub is_active: bool,
}

pub struct DelegatedProofOfStake {
    delegates: Arc<Mutex<BTreeMap<String, Delegate>>>,
    total_votes: Arc<Mutex<u64>>,
    active_delegates_count: usize,
}

impl DelegatedProofOfStake {
    pub fn new(active_count: usize) -> Self {
        Self {
            delegates: Arc::new(Mutex::new(BTreeMap::new())),
            total_votes: Arc::new(Mutex::new(0)),
            active_delegates_count: active_count,
        }
    }

    pub fn register_delegate(&self, id: String, address: String) {
        let mut delegates = self.delegates.lock().unwrap();
        let delegate = Delegate {
            id: id.clone(),
            address,
            votes: 0,
            is_active: false,
        };
        delegates.insert(id, delegate);
    }

    pub fn vote(&self, delegate_id: &str, vote_count: u64) {
        let mut delegates = self.delegates.lock().unwrap();
        if let Some(delegate) = delegates.get_mut(delegate_id) {
            delegate.votes += vote_count;
            let mut total = self.total_votes.lock().unwrap();
            *total += vote_count;
        }
    }

    pub fn select_active_delegates(&self) -> Vec<Delegate> {
        let mut delegates: Vec<Delegate> =
            self.delegates.lock().unwrap().values().cloned().collect();

        delegates.sort_by(|a, b| b.votes.cmp(&a.votes));

        let active_count = self.active_delegates_count.min(delegates.len());
        for i in 0..active_count {
            delegates[i].is_active = true;
        }

        delegates.into_iter().take(active_count).collect()
    }

    pub fn get_active_delegates(&self) -> Vec<Delegate> {
        let delegates = self.delegates.lock().unwrap();
        delegates
            .values()
            .filter(|d| d.is_active)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dpos() {
        let dpos = DelegatedProofOfStake::new(3);
        dpos.register_delegate("d1".to_string(), "0x1".to_string());
        dpos.register_delegate("d2".to_string(), "0x2".to_string());
        dpos.register_delegate("d3".to_string(), "0x3".to_string());

        dpos.vote("d1", 1000);
        dpos.vote("d2", 500);

        let active = dpos.select_active_delegates();
        assert!(!active.is_empty());
    }
}

// Strategy: Dependency Inversion for consensus (Core)
// Review and adjust before applying.
