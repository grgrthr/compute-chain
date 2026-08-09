use crate::api::handlers::AppState;
use axum::{extract::State, Json};
use std::collections::HashMap;
use std::sync::Arc;

pub async fn get_p2p_peer_id_handler(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let peer_id = state.p2p_handle.local_peer_id.to_string();
    let listen_addr = state.p2p_handle.listen_addr.to_string();
    Json(
        serde_json::json!({ "peer_id": peer_id, "listen_addr": listen_addr, "full_addr": format!("{}/p2p/{}", listen_addr, peer_id) }),
    )
}

pub async fn get_p2p_peers_handler(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    match state.p2p_handle.get_connected_peers().await {
        Ok(peers) => {
            tracing::info!("👥 P2P peers: {}", peers.len());
            Json(serde_json::json!({ "peers": peers, "count": peers.len() }))
        }
        Err(e) => {
            tracing::error!("Failed to get peers: {}", e);
            Json(serde_json::json!({ "peers": [], "count": 0, "error": e.to_string() }))
        }
    }
}

pub async fn get_p2p_peer_count_handler(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    match state.p2p_handle.get_peer_count().await {
        Ok(count) => Json(serde_json::json!({ "count": count })),
        Err(e) => Json(serde_json::json!({ "count": 0, "error": e.to_string() })),
    }
}

pub async fn p2p_dial_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(p): axum::extract::Query<HashMap<String, String>>,
) -> Json<serde_json::Value> {
    let addr_raw = p.get("addr").cloned().unwrap_or_default();
    if addr_raw.is_empty() {
        return Json(
            serde_json::json!({ "status": "error", "message": "Missing 'addr' parameter" }),
        );
    }
    let multiaddr_str = if addr_raw.contains(':') && !addr_raw.starts_with('/') {
        let parts: Vec<&str> = addr_raw.split(':').collect();
        if parts.len() >= 2 {
            format!("/ip4/{}/tcp/{}", parts[0], parts[1])
        } else {
            addr_raw
        }
    } else {
        addr_raw
    };

    tracing::info!("📞 P2P dial: {}", multiaddr_str);
    match state.p2p_handle.dial_peer(&multiaddr_str).await {
        Ok(()) => {
            tracing::info!("✅ P2P dial succeeded: {}", multiaddr_str);
            let count = state.p2p_handle.get_peer_count().await.unwrap_or(0);
            Json(
                serde_json::json!({ "status": "connected", "address": multiaddr_str, "peers": count }),
            )
        }
        Err(e) => {
            tracing::error!("❌ P2P dial failed: {}", e);
            Json(serde_json::json!({ "status": "error", "message": e.to_string() }))
        }
    }
}

// Strategy: Insert Mediator for api (Interface)
// Review and adjust before applying.
