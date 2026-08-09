use serde::{Deserialize, Serialize};

/// Permanent entry points to the network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapNode {
    pub address: String,
    pub peer_id: Option<String>,
}

/// Default bootstrap nodes - at least one should be online
pub fn default_bootstrap_nodes() -> Vec<BootstrapNode> {
    vec![
        BootstrapNode {
            address: "/ip4/127.0.0.1/tcp/5001".to_string(),
            peer_id: None,
        },
        BootstrapNode {
            address: "/ip4/127.0.0.1/tcp/5002".to_string(),
            peer_id: None,
        },
    ]
}

/// Get all bootstrap addresses as multiaddr strings
pub fn bootstrap_addresses() -> Vec<String> {
    default_bootstrap_nodes()
        .iter()
        .map(|n| n.address.clone())
        .collect()
}

// Strategy: Move Dependencies Down for p2p (Infrastructure)
// Review and adjust before applying.
