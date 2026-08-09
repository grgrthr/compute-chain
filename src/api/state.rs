use crate::p2p::P2PHandle;
use crate::websocket::WebSocketServer;
use std::sync::{Arc, Mutex};

pub struct AppState {
    pub miner_pool: Mutex<crate::miner::MinerPool>,
    pub gpu_miner: crate::miner::gpu::GpuMiner,
    pub marketplace: Mutex<crate::marketplace::Marketplace>,
    pub p2p_handle: P2PHandle,
    pub consensus: Arc<crate::consensus::network::ConsensusNetwork>,
    pub token_engine: Arc<crate::economic::token::TokenEngine>,
    pub contract_storage: Arc<crate::contract::storage::ContractStorage>,
    pub nft_engine: Arc<crate::economic::nft::NFTEngine>,
    pub ws_server: Arc<WebSocketServer>,
}
