use crate::api::router::create_router;
use crate::api::handlers::AppState;
use crate::node::identity::NodeRole;
use crate::node::runtime::DistributedRuntime;
use crate::node::event_handler::handle_p2p_event;
use crate::p2p::{spawn_p2p_actor, persistence, bootstrap, P2PEvent};
use crate::consensus::network::ConsensusNetwork;
use crate::consensus::mempool::SharedMempool;
use crate::economic::token::TokenEngine;
use crate::economic::nft::NFTEngine;
use crate::contract::storage::ContractStorage;
use crate::miner::MinerPool;
use crate::miner::gpu::GpuMiner;
use crate::marketplace::Marketplace;
use crate::crypto::wallet::Wallet;
use crate::websocket::WebSocketServer;
use std::sync::Arc;
use std::collections::HashSet;
use axum::Router;

pub async fn start_server(api_port: u16) {
    let p2p_port: u16 = 5000 + (api_port - 3000);
    let self_addr = format!("/ip4/127.0.0.1/tcp/{}", p2p_port);
    
    // Auto-dial peers
    let mut auto_dial: HashSet<String> = bootstrap::bootstrap_addresses().into_iter().collect();
    let saved_peers = persistence::load_peers();
    for addr in persistence::get_all_addresses(&saved_peers) { auto_dial.insert(addr); }
    auto_dial.remove(&self_addr);
    let auto_dial_peers: Vec<String> = auto_dial.into_iter().collect();
    
    tracing::info!("🔗 Auto-dial list: {} peers", auto_dial_peers.len());
    
    let (p2p_handle, mut event_rx, _p2p_task) = spawn_p2p_actor(p2p_port, auto_dial_peers.clone()).await.unwrap();
    tracing::info!("P2P Actor spawned on port {}", p2p_port);
    
    // Dial bootstrap peers
    let dial_handle = p2p_handle.clone();
    let dial_peers = auto_dial_peers.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        for addr in &dial_peers {
            let _ = dial_handle.dial_peer(addr).await;
        }
    });
    
    // ═══ DISTRIBUTED RUNTIME SETUP ═══
    let node_id = match p2p_port { 5000 => "node_alpha", 5001 => "node_beta", 5002 => "node_gamma", _ => "node_unknown" };
    let role = match p2p_port { 5000 => NodeRole::Producer, _ => NodeRole::Validator };
    let is_leader = p2p_port == 5000;
    
    let runtime = Arc::new(DistributedRuntime::new(node_id, role, p2p_handle.clone(), is_leader));
    tracing::info!("🆔 Node: {} ({:?}) leader={}", node_id, format!("{:?}", &role), is_leader);
    
    // Token + Consensus + Storage
    let token_engine = TokenEngine::load_from_disk("./chain_data").unwrap_or_else(|_| { let t = TokenEngine::new(); t.save_to_disk("./chain_data").ok(); t });
    let contract_storage = ContractStorage::load_from_disk("./chain_data").unwrap_or_else(|_| ContractStorage::new());
    let nft_engine = NFTEngine::load_from_disk("./chain_data").unwrap_or_else(|_| { let n = NFTEngine::new(); n.save_to_disk("./chain_data").ok(); n });
    let mempool = SharedMempool::new(10000);
    let wallet = Wallet::new();
    
    let consensus = ConsensusNetwork::new();
    let ws_server = Arc::new(WebSocketServer::new(9000 + (api_port - 3000)));
    
    let state = Arc::new(AppState {
        miner_pool: std::sync::Mutex::new(MinerPool::new()),
        gpu_miner: GpuMiner::new(),
        marketplace: std::sync::Mutex::new(Marketplace::new()),
        p2p_handle: p2p_handle.clone(),
        consensus: Arc::new(consensus),
        token_engine: Arc::new(token_engine),
        contract_storage: Arc::new(contract_storage),
        nft_engine: Arc::new(nft_engine),
        ws_server: ws_server.clone(),
        wallet: std::sync::Mutex::new(wallet),
        mempool: std::sync::Arc::new(mempool),
    });
    
    // ═══ BACKGROUND WORKER ═══
    let worker_cfg = runtime.get_worker_config(state.consensus.clone(), state.token_engine.clone());
    tokio::spawn(async move { crate::node::worker::run_worker_loop(worker_cfg).await });
    
    // ═══ P2P EVENT LISTENER ═══
    let mut event_ctx = runtime.get_event_context(Some(state.consensus.clone()));
    event_ctx.token_engine = Some(state.token_engine.clone());
    let event_ctx = std::sync::Arc::new(event_ctx);
    let event_ctx_p2p = event_ctx.clone();
    tokio::spawn(async move {
        loop {
            match event_rx.recv().await {
                Ok(event) => handle_p2p_event(event, &event_ctx_p2p).await,
                Err(_) => break,
            }
        }
    });
    
    // ═══ WORKER EVENTS LISTENER (browser_result_completed → proof → block → reward) ═══
    let event_ctx_worker = event_ctx.clone();
    tokio::spawn(async move {
        let mut worker_rx = crate::api::worker_ws::WORKER_EVENTS.subscribe();
        loop {
            match worker_rx.recv().await {
                Ok(msg) => {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&msg) {
                        if parsed.get("type").and_then(|v| v.as_str()) == Some("browser_result_completed") {
                            let job_id = parsed.get("job_id").and_then(|v| v.as_str()).unwrap_or("");
                            let worker_id = parsed.get("worker_id").and_then(|v| v.as_str()).unwrap_or("");
                            if !job_id.is_empty() {
                                crate::node::event_handler::handle_browser_result_completed(
                                    job_id, worker_id, &event_ctx_worker
                                ).await;
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }
    });
    
    // ═══ HTTP SERVER ═══
    let app = create_router(state);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", api_port)).await.unwrap();
    tracing::info!("🛡️ API: http://localhost:{}, P2P: {}, WS: {}", api_port, p2p_port, 9000 + (api_port - 3000));
    axum::serve(listener, app).await.unwrap();
}
