use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerRecord {
    pub peer_id: String,
    pub addresses: Vec<String>,
    pub last_seen: u64,
}

const PEERS_FILE: &str = "chain_data/peers.json";

/// Load persisted peers from disk
/// Filters out peers with no valid addresses and invalid addresses
pub fn load_peers() -> Vec<PeerRecord> {
    if !Path::new(PEERS_FILE).exists() {
        tracing::info!("📂 No peers file found, starting fresh");
        return Vec::new();
    }

    match fs::read_to_string(PEERS_FILE) {
        Ok(json) => {
            match serde_json::from_str::<Vec<PeerRecord>>(&json) {
                Ok(peers) => {
                    let total = peers.len();
                    // Filter each peer's addresses to only valid multiaddrs
                    let cleaned: Vec<PeerRecord> = peers
                        .into_iter()
                        .map(|mut p| {
                            p.addresses.retain(|a| is_valid_multiaddr(a));
                            p
                        })
                        .filter(|p| !p.addresses.is_empty())
                        .collect();

                    tracing::info!(
                        "📂 Loaded {} peers from disk ({} valid, {} removed)",
                        total,
                        cleaned.len(),
                        total - cleaned.len()
                    );
                    cleaned
                }
                Err(e) => {
                    tracing::warn!("Failed to parse peers file: {}", e);
                    Vec::new()
                }
            }
        }
        Err(e) => {
            tracing::warn!("Failed to read peers file: {}", e);
            Vec::new()
        }
    }
}

/// Save a single peer to disk (upsert: update if exists, insert if new)
/// Only saves valid multiaddr addresses, never saves peer_id as address.
pub fn save_peer(peer_id: &str, address: &str) {
    // Never save peer_id as an address
    if !is_valid_multiaddr(address) {
        tracing::warn!("🚫 Refusing to save invalid address: {}", address);
        return;
    }

    let mut peers = load_all_peers();
    let now = current_timestamp();

    if let Some(existing) = peers.iter_mut().find(|p| p.peer_id == peer_id) {
        if !existing.addresses.contains(&address.to_string()) {
            existing.addresses.push(address.to_string());
        }
        // Clean existing addresses too
        existing.addresses.retain(|a| is_valid_multiaddr(a));
        existing.last_seen = now;
    } else {
        peers.push(PeerRecord {
            peer_id: peer_id.to_string(),
            addresses: vec![address.to_string()],
            last_seen: now,
        });
    }

    // Remove peers not seen in 7 days
    let cutoff = now.saturating_sub(7 * 24 * 3600);
    peers.retain(|p| p.last_seen >= cutoff);

    // Final cleanup: ensure no peer_id slipped in as address
    for p in &mut peers {
        p.addresses
            .retain(|a| is_valid_multiaddr(a) && *a != p.peer_id);
    }
    peers.retain(|p| !p.addresses.is_empty());

    write_peers(&peers);
}

/// Save all peers to disk
pub fn save_peers(peers: &[PeerRecord]) {
    let mut cleaned: Vec<PeerRecord> = peers
        .iter()
        .map(|p| {
            let mut p = p.clone();
            p.addresses.retain(|a| is_valid_multiaddr(a));
            p
        })
        .filter(|p| !p.addresses.is_empty())
        .collect();

    for p in &mut cleaned {
        p.addresses
            .retain(|a| is_valid_multiaddr(a) && *a != p.peer_id);
    }
    cleaned.retain(|p| !p.addresses.is_empty());

    write_peers(&cleaned);
}

/// Get all valid addresses from peer records (for auto-dial)
/// Only returns addresses that look like valid multiaddrs.
pub fn get_all_addresses(peers: &[PeerRecord]) -> Vec<String> {
    let mut addrs: Vec<String> = peers
        .iter()
        .flat_map(|p| p.addresses.clone())
        .filter(|a| is_valid_multiaddr(a))
        .collect();
    addrs.sort();
    addrs.dedup();
    addrs
}

/// Load ALL peers including unvalidated (for saving/updating)
fn load_all_peers() -> Vec<PeerRecord> {
    if !Path::new(PEERS_FILE).exists() {
        return Vec::new();
    }

    match fs::read_to_string(PEERS_FILE) {
        Ok(json) => match serde_json::from_str::<Vec<PeerRecord>>(&json) {
            Ok(peers) => peers,
            Err(_) => Vec::new(),
        },
        Err(_) => Vec::new(),
    }
}

fn write_peers(peers: &[PeerRecord]) {
    let dir = Path::new(PEERS_FILE).parent().unwrap();
    if !dir.exists() {
        let _ = fs::create_dir_all(dir);
    }

    if let Ok(json) = serde_json::to_string_pretty(peers) {
        if let Err(e) = fs::write(PEERS_FILE, json) {
            tracing::warn!("Failed to save peers: {}", e);
        } else {
            tracing::info!("💾 Saved {} peers to disk", peers.len());
        }
    }
}

/// Check if a string looks like a valid libp2p multiaddr
fn is_valid_multiaddr(addr: &str) -> bool {
    // Valid multiaddrs start with /ip4/, /ip6/, /dns/, /dns4/, /dns6/, /dnsaddr/
    let valid_prefixes = ["/ip4/", "/ip6/", "/dns4/", "/dns6/", "/dnsaddr/", "/dns/"];

    // Must start with a valid prefix
    if !valid_prefixes.iter().any(|p| addr.starts_with(p)) {
        return false;
    }

    // Must contain a port (has /tcp/, /udp/, /quic/, etc after the address)
    let transport_prefixes = ["/tcp/", "/udp/", "/quic/", "/ws/", "/wss/"];
    if !transport_prefixes.iter().any(|p| addr.contains(p)) {
        return false;
    }

    // Must not be a raw peer_id (which looks like "12D3KooW...")
    if addr.starts_with("12D3KooW") || (addr.starts_with('1') && addr.len() > 40) {
        return false;
    }

    true
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

// Strategy: Move Dependencies Down for p2p (Infrastructure)
// Review and adjust before applying.
