use crate::api::handlers::AppState;
use axum::{extract::State, Json};
use std::sync::Arc;

pub async fn get_gpu_info_handler(State(_state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let gpu = crate::miner::gpu::GpuManager::new();
    Json(
        serde_json::json!({ "has_gpu": gpu.has_real_gpu(), "device_count": gpu.device_count(), "devices": gpu.all_gpu_info() }),
    )
}

pub async fn gpu_execute_handler(
    State(_state): State<Arc<AppState>>,
    Json(workload): Json<crate::miner::gpu::GpuWorkload>,
) -> Json<serde_json::Value> {
    let gpu = crate::miner::gpu::GpuManager::new();
    match gpu.execute(&workload) {
        Ok(result) => Json(serde_json::json!({ "success": true, "result": result })),
        Err(e) => Json(serde_json::json!({ "success": false, "error": e })),
    }
}

// Strategy: Insert Mediator for api (Interface)
// Review and adjust before applying.
