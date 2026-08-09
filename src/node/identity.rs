use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub node_id: String,
    pub role: NodeRole,
    pub status: NodeStatus,
    pub capabilities: Vec<String>,
    pub reputation: f64,
    pub current_load: f64,
    pub last_seen: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum NodeRole { Producer, Validator, Worker, Observer }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeStatus { Online, Busy, Offline, Degraded }

impl NodeInfo {
    pub fn new(node_id: &str, role: NodeRole) -> Self {
        NodeInfo {
            node_id: node_id.to_string(),
            role,
            status: NodeStatus::Online,
            capabilities: vec!["compute".into(), "stark_proof".into()],
            reputation: 1.0,
            current_load: 0.0,
            last_seen: now(),
        }
    }
    pub fn is_available(&self) -> bool {
        self.status == NodeStatus::Online && self.current_load < 90.0
    }
}

fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() }
