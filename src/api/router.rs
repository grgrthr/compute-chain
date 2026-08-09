use crate::api::dashboard::dashboard_handler;
use crate::api::handlers::chain::*;
use crate::api::handlers::compute::*;
use crate::api::handlers::contract::*;
use crate::api::handlers::gpu::*;
use crate::api::handlers::health::*;
use crate::api::handlers::jobs::*;
use crate::api::handlers::marketplace::*;
use crate::api::handlers::miner::*;
use crate::api::handlers::nft::*;
use crate::api::handlers::p2p::*;
use crate::api::handlers::stark::*;
use crate::api::handlers::token::*;
use crate::api::handlers::AppState;
use crate::api::worker_ws::{worker_ws_handler, worker_events_ws, worker_page_handler, tasks_js_handler, demo_page_handler, sha256_js_handler};
use crate::api::investor::{investor_handler, investor_api_status, demo_state_handler};
use axum::{routing::{get, post}, Router};
use std::sync::Arc;

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/dashboard", get(dashboard_handler))
        .route("/investor", get(investor_handler))
        .route("/investor/api/status", get(investor_api_status))
        .route("/api/demo/state", get(demo_state_handler))
        .route("/worker/ws", get(worker_ws_handler))
        .route("/worker/events", get(worker_events_ws))
        .route("/demo", get(demo_page_handler))
        .route("/worker", get(worker_page_handler))
        .route("/assets/tasks.js", get(tasks_js_handler))
        .route("/assets/sha256.js", get(sha256_js_handler))
        .route("/health", get(health_handler))
        .route("/compute", post(compute_handler))
        .route("/compute/adaptive", post(adaptive_compute_handler))
        .route("/pipeline", post(pipeline_handler))
        .route("/jobs", post(submit_job_handler))
        .route("/jobs/upload", post(upload_job_handler))
        .route("/jobs", get(list_jobs_handler))
        .route("/jobs/pending", get(pending_jobs_handler))
        .route("/jobs/completed", get(completed_jobs_handler))
        .route("/jobs/{id}", get(get_job_handler))
        .route("/jobs/process", post(process_jobs_handler))
        .route("/workers", get(list_workers_handler))
        .route("/miner/register", post(register_miner_handler))
        .route("/miner/stats", get(get_miner_stats_handler))
        .route("/miner/list", get(list_miners_handler))
        .route("/miner/gpu/info", get(gpu_info_handler))
        .route("/gpu/info", get(get_gpu_info_handler))
        .route("/gpu/execute", post(gpu_execute_handler))
        .route("/marketplace/order", post(create_order_handler))
        .route("/marketplace/orders", get(get_open_orders_handler))
        .route("/marketplace/stats", get(get_market_stats_handler))
        .route("/p2p/id", get(get_p2p_peer_id_handler))
        .route("/p2p/peers", get(get_p2p_peers_handler))
        .route("/p2p/count", get(get_p2p_peer_count_handler))
        .route("/p2p/dial", get(p2p_dial_handler))
        .route("/stark/prove", post(stark_prove_handler))
        .route("/stark/verify", post(stark_verify_handler))
        .route("/block/vote", post(block_vote_handler))
        .route("/block/mine", post(mine_block_handler))
        .route("/block/receive", post(receive_block_handler))
        .route("/chain/sync", get(sync_chain_handler))
        .route("/chain", get(get_chain_handler))
        .route("/block", get(get_block_handler))
        .route("/blocks", get(list_blocks_handler))
        .route("/tx/send", post(send_transaction_handler))
        .route("/tx/balance", get(get_balance_handler))
        .route("/contract/deploy", post(deploy_contract_handler))
        .route("/contract/call", post(call_contract_handler))
        .route("/contract", get(get_contract_handler))
        .route("/contract/call/gas", post(call_with_gas_handler))
        .route("/contract/estimate_gas", post(estimate_gas_handler))
        .route("/gas/stats", get(gas_stats_handler))
        .route("/nft/id", get(get_nft_by_id_handler))
        .route("/nft/mint", post(mint_nft_handler))
        .route("/nft/transfer", post(transfer_nft_handler))
        .route("/nft", get(get_nft_handler))
        .route("/nfts", get(list_nfts_handler))
        .route("/nft/owner", get(get_owner_nfts_handler))
        .with_state(state)
}
