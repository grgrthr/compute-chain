use crate::node::identity::{NodeInfo, NodeRole, NodeStatus};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct PeerRegistry {
    pub local_node: NodeInfo,
    pub peers: HashMap<String, NodeInfo>,
}

impl PeerRegistry {
    pub fn new(local_node: NodeInfo) -> Self {
        PeerRegistry { local_node, peers: HashMap::new() }
    }

    pub fn update_peer(&mut self, peer: NodeInfo) {
        self.peers.insert(peer.node_id.clone(), peer);
    }

    pub fn remove_peer(&mut self, node_id: &str) {
        self.peers.remove(node_id);
    }

    pub fn get_peer(&self, node_id: &str) -> Option<&NodeInfo> {
        self.peers.get(node_id)
    }

    pub fn get_available_workers(&self) -> Vec<&NodeInfo> {
        self.peers.values()
            .filter(|p| p.is_available() && p.role != NodeRole::Producer)
            .collect()
    }

    pub fn get_lowest_load_worker(&self) -> Option<String> {
        self.get_available_workers()
            .iter()
            .min_by(|a, b| a.current_load.partial_cmp(&b.current_load).unwrap_or(std::cmp::Ordering::Equal))
            .map(|w| w.node_id.clone())
    }

    pub fn set_local_load(&mut self, load: f64) {
        self.local_node.current_load = load;
        self.local_node.last_seen = now();
    }

    pub fn peer_count(&self) -> usize { self.peers.len() }
}

fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() }
