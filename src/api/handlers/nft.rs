use crate::api::handlers::AppState;
use axum::{extract::State, Json};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, serde::Deserialize)]
pub struct MintNFTRequest {
    pub owner: String,
    pub name: String,
    pub data: String,
}

pub async fn mint_nft_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<MintNFTRequest>,
) -> Json<serde_json::Value> {
    tracing::info!(
        "🎨 NFT mint: owner={}, name={}",
        request.owner,
        request.name
    );
    let nft_id = state
        .nft_engine
        .mint(&request.owner, &request.name, &request.data);
    Json(
        serde_json::json!({ "status": "minted", "nft_id": nft_id, "owner": request.owner, "name": request.name }),
    )
}

pub async fn transfer_nft_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(p): axum::extract::Query<HashMap<String, String>>,
    Json(payload): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let nft_id = p.get("nft_id").cloned().unwrap_or_default();
    let from = payload["from"].as_str().unwrap_or("");
    let to = payload["to"].as_str().unwrap_or("");
    match state.nft_engine.transfer(&nft_id, from, to) {
        Ok(_) => Json(
            serde_json::json!({"status": "transferred", "nft_id": nft_id, "from": from, "to": to}),
        ),
        Err(e) => Json(serde_json::json!({"status": "error", "message": e})),
    }
}

pub async fn get_nft_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(p): axum::extract::Query<HashMap<String, String>>,
) -> Json<serde_json::Value> {
    let nft_id = p.get("nft_id").cloned().unwrap_or_default();
    match state.nft_engine.get(&nft_id) {
        Some(nft) => Json(
            serde_json::json!({"found": true, "id": nft.id, "name": nft.name, "owner": nft.owner, "data": nft.data, "transfer_count": nft.transfer_count}),
        ),
        None => Json(serde_json::json!({"found": false, "id": nft_id})),
    }
}

pub async fn get_nft_by_id_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(p): axum::extract::Query<HashMap<String, String>>,
) -> Json<serde_json::Value> {
    let id = p.get("id").cloned().unwrap_or_default();
    match state.nft_engine.get(&id) {
        Some(nft) => {
            Json(serde_json::json!({"ok":true,"id":nft.id,"name":nft.name,"owner":nft.owner}))
        }
        None => Json(serde_json::json!({"ok":false})),
    }
}

pub async fn list_nfts_handler(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let nfts = state.nft_engine.list_all();
    let summaries: Vec<serde_json::Value> = nfts
        .iter()
        .map(|n| serde_json::json!({"id": n.id, "name": n.name, "owner": n.owner}))
        .collect();
    Json(serde_json::json!({"count": summaries.len(), "nfts": summaries}))
}

pub async fn get_owner_nfts_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(p): axum::extract::Query<HashMap<String, String>>,
) -> Json<serde_json::Value> {
    let address = p.get("address").cloned().unwrap_or_default();
    let nfts = state.nft_engine.get_by_owner(&address);
    let summaries: Vec<serde_json::Value> = nfts
        .iter()
        .map(|n| serde_json::json!({"id": n.id, "name": n.name, "owner": n.owner, "data": n.data}))
        .collect();
    Json(serde_json::json!({"owner": address, "count": summaries.len(), "nfts": summaries}))
}

// Strategy: Insert Mediator for api (Interface)
// Review and adjust before applying.
