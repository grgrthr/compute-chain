use axum::response::{Html, IntoResponse};
use axum::http::header;
use axum::Json;

pub async fn investor_handler() -> impl IntoResponse {
    let mut headers = header::HeaderMap::new();
    headers.insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store, no-cache, must-revalidate, max-age=0"));
    headers.insert(header::PRAGMA, header::HeaderValue::from_static("no-cache"));
    headers.insert(header::EXPIRES, header::HeaderValue::from_static("0"));
    (headers, Html(include_str!("../../assets/investor.html").to_string()))
}

pub async fn investor_api_status() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "network": {"status":"online","nodes_total":3,"nodes_connected":3,"current_round":0,"blockchain_height":0,"total_blocks":0,"tps":0.0,"avg_block_time_ms":0,"network_health_pct":100.0},
        "jobs":{"pending":0,"running":0,"completed":0,"failed":0,"proofs_generated":0,"proofs_verified":0,"proofs_rejected":0,"avg_verification_time_ms":0,"avg_job_time_ms":0,"success_rate_pct":100.0},
        "consensus":{"status":"healthy","current_view":0,"validators":3,"last_block_time":"","fork_count":0,"finality":"instant"},
        "nodes":[
            {"id":"demo_node_1","role":"Producer","status":"online","cpu":12.5,"memory":34.2,"uptime":0,"round":0,"height":0,"jobs_completed":0,"proofs_verified":0,"peer_count":2,"blocks_produced":0,"last_activity":"just now"},
            {"id":"demo_node_2","role":"Validator","status":"online","cpu":8.3,"memory":28.7,"uptime":0,"round":0,"height":0,"jobs_completed":0,"proofs_verified":0,"peer_count":2,"blocks_produced":0,"last_activity":"just now"},
            {"id":"demo_node_3","role":"Validator","status":"online","cpu":15.1,"memory":41.0,"uptime":0,"round":0,"height":0,"jobs_completed":0,"proofs_verified":0,"peer_count":2,"blocks_produced":0,"last_activity":"just now"}
        ],
        "blocks":[],"events":[],"performance":{"tps_history":[],"cpu_history":[],"memory_history":[],"job_history":[],"proof_history":[]},
        "wallets":[
            {"address":"0x1a2b3c4d5e6f","balance":1000000,"label":"Genesis","tx_count":0},
            {"address":"0x7g8h9i0j1k2l","balance":500000,"label":"Validator 1","tx_count":0},
            {"address":"0x3m4n5o6p7q8r","balance":250000,"label":"Validator 2","tx_count":0}
        ],
        "transactions":[],"storage":{"total_gb":500,"used_gb":0.5,"blockchain_gb":0.01,"proofs_gb":0.02,"growth_rate_mb_per_day":5},"alerts":[]
    }))
}

pub async fn demo_state_handler() -> Json<serde_json::Value> {
    let workers: Vec<serde_json::Value> = crate::api::worker_registry::WORKER_REGISTRY
        .lock().unwrap().iter().map(|(id, info)| serde_json::json!({
            "worker_id": id, "status": format!("{:?}", info.status),
            "current_job": info.current_job, "capabilities": info.capabilities
        })).collect();

    let jobs: Vec<serde_json::Value> = crate::api::handlers::browser_jobs::BROWSER_JOBS
        .lock().unwrap().iter().map(|(id, record)| serde_json::json!({
            "job_id": id, "task_type": record.task_type, "status": record.status,
            "progress": record.progress, "verification": record.verification_status,
            "merkle_root": record.merkle_root.as_ref().map(|h| &h[..16]),
            "proof_hash": record.proof_hash.as_ref().map(|h| &h[..16]),
            "proof_verified": record.proof_verified, "block_height": record.block_height,
            "reward": record.reward, "worker_id": record.worker_id
        })).collect();

    let chain_height = jobs.iter()
        .filter_map(|j| j.get("block_height").and_then(|v| v.as_u64()))
        .max().unwrap_or(0);

    Json(serde_json::json!({
        "workers": workers, "worker_count": workers.len(),
        "jobs": jobs, "jobs_count": jobs.len(),
        "chain_height": chain_height,
        "timestamp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
    }))
}
