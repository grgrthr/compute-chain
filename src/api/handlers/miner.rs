use crate::api::handlers::AppState;
use crate::miner::gpu::GpuInfo;
use crate::miner::types::{Miner, MinerStats};
use axum::{extract::State, Json};
use std::collections::HashMap;
use std::sync::Arc;

pub async fn register_miner_handler(State(state): State<Arc<AppState>>) -> Json<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let address = format!("0x{}", hex::encode(id.as_bytes()));
    Json(state.miner_pool.lock().unwrap().register_miner(id, address))
}

pub async fn get_miner_stats_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(p): axum::extract::Query<HashMap<String, String>>,
) -> Json<Option<MinerStats>> {
    let miner_id = p.get("miner_id").cloned().unwrap_or_default();
    Json(state.miner_pool.lock().unwrap().get_miner_stats(&miner_id))
}

pub async fn list_miners_handler(State(state): State<Arc<AppState>>) -> Json<Vec<Miner>> {
    Json(state.miner_pool.lock().unwrap().list_miners())
}

pub async fn gpu_info_handler(State(state): State<Arc<AppState>>) -> Json<GpuInfo> {
    Json(state.gpu_miner.get_gpu_info())
}

// Strategy: Insert Mediator for api (Interface)
// Review and adjust before applying.
