use crate::p2p::P2PEvent;
use crate::p2p::message::{WorkloadAssignment, WorkloadResult};
use crate::p2p::proof_network::ComputeJob;
use crate::compute_pool::PoolJob;
use crate::p2p::node::P2PCommand;
use crate::api::handlers::browser_jobs;
use std::sync::Arc;

pub struct EventContext {
    pub node_id: String,
    pub job_queue: Arc<std::sync::Mutex<Vec<PoolJob>>>,
    pub p2p_handle: crate::p2p::P2PHandle,
    pub consensus: Option<Arc<crate::consensus::network::ConsensusNetwork>>,
    pub token_engine: Option<Arc<crate::economic::token::TokenEngine>>,
}

pub async fn handle_p2p_event(event: P2PEvent, ctx: &EventContext) {
    match event {
        P2PEvent::WorkloadAssignmentReceived(assignment) => {
            handle_assignment(assignment, ctx).await;
        }
        P2PEvent::WorkloadResultReceived(msg) => {
            handle_result(msg, ctx).await;
        }
        P2PEvent::BlockReceived(block) => {
            tracing::info!("📦 Block received via gossip: height={} hash={}", block.height, block.hash);
        }
        P2PEvent::PeerConnected { peer_id } => {
            tracing::info!("🔗 Peer connected: {}", peer_id);
        }
        P2PEvent::PeerDisconnected { peer_id } => {
            tracing::info!("🔌 Peer disconnected: {}", peer_id);
        }
        P2PEvent::WorkloadAnnounceReceived(_) => {}
        _ => {}
    }
}
/// Handle browser_result_completed: proof → block → reward
pub async fn handle_browser_result_completed(job_id: &str, worker_id: &str, ctx: &EventContext) {
    // Check if already finalized
    if browser_jobs::is_already_finalized(job_id) {
        tracing::info!("🔁 BrowserJob {} already finalized, skipping", job_id);
        return;
    }
    
    let consensus = match ctx.consensus.as_ref() {
        Some(c) => c,
        None => {
            tracing::warn!("❌ No consensus available for browser job {}", job_id);
            return;
        }
    };
    
    let token_engine = match ctx.token_engine.as_ref() {
        Some(t) => t,
        None => {
            tracing::warn!("❌ No token engine available for browser job {}", job_id);
            return;
        }
    };
    
    match browser_jobs::finalize_browser_job(job_id, consensus, token_engine) {
        Ok(record) => {
            tracing::info!("✅ BrowserJob {} finalized: block={:?}, reward={:?}", 
                job_id, record.block_height, record.reward);
        }
        Err(e) => {
            tracing::warn!("❌ BrowserJob {} finalization failed: {}", job_id, e);
        }
    }
}

async fn handle_assignment(assignment: WorkloadAssignment, ctx: &EventContext) {
    if assignment.assigned_peer != ctx.node_id {
        tracing::info!("⏭️ Skipping job {} — for {}", assignment.workload_id, assignment.assigned_peer);
        return;
    }
    tracing::info!("📥 ASSIGNED: job {} to me!", assignment.workload_id);
    let job = ComputeJob {
        job_id: assignment.workload_id.clone(),
        instructions: assignment.program.clone(),
        input_registers: vec![0; 8],
        difficulty: 1,
    };
    let pool_job = PoolJob::new(job);
    ctx.job_queue.lock().unwrap().push(pool_job);
}

async fn handle_result(msg: WorkloadResult, ctx: &EventContext) {
    tracing::info!("📥 Leader received result: job={} worker={} success={}", msg.workload_id, msg.worker_peer, msg.success);
    if !msg.success {
        tracing::warn!("❌ Job {} FAILED — no block, no reward", msg.workload_id);
        let event = serde_json::json!({
            "type": "job_failed",
            "job_id": msg.workload_id,
            "worker_id": msg.worker_peer,
            "error": "worker reported failure"
        });
        let _ = crate::api::worker_ws::WORKER_EVENTS.send(event.to_string());
        return;
    }
    if msg.success {
        let hash = format!("block_{}_{}", msg.workload_id, &msg.trace_root[..16]);
        let worker = msg.worker_peer.clone();
        let now_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
        
        // Add to local blockchain if consensus is available
        if let Some(ref consensus) = ctx.consensus {
            let last = consensus.get_last_block();
            let coinbase = crate::consensus::types::Transaction::new("network".to_string(), worker.clone(), 100, 0);
            let mut block = crate::consensus::types::Block::new(last.height + 1, last.hash.clone(), worker.clone(), vec![coinbase]);
            block.hash = hash.clone();
            let _ = consensus.blockchain.add_block(block.clone());
            tracing::info!("🤖 LEADER: block added to local chain height={}", last.height + 1);
        }
        
        // Broadcast to network
        let p2p = ctx.p2p_handle.clone();
        tokio::spawn(async move {
            let _ = p2p.command_tx.send(P2PCommand::BroadcastBlock {
                height: 1u64, hash: hash.clone(), previous_hash: "genesis".to_string(),
                validator_id: worker.clone(), transaction_count: 1, timestamp: now_ts,
            }).await;
            tracing::info!("📡 Block broadcast: {}", hash);
        });
    }
}
