use crate::compute_pool::{PoolJob, PoolJobStatus};
use crate::p2p::proof_network::WorkerNode;
use crate::consensus::types::{Block, Transaction};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct WorkerConfig {
    pub node_id: String,
    pub is_leader: bool,
    pub job_queue: Arc<std::sync::Mutex<Vec<PoolJob>>>,
    pub consensus: Arc<crate::consensus::network::ConsensusNetwork>,
    pub token_engine: Arc<crate::economic::token::TokenEngine>,
    pub p2p_handle: crate::p2p::P2PHandle,
}

pub async fn run_worker_loop(config: WorkerConfig) {
    let mut interval = tokio::time::interval(Duration::from_secs(3));
    loop {
        interval.tick().await;
        process_pending_jobs(&config);
    }
}

fn process_pending_jobs(cfg: &WorkerConfig) {
    let mut queue = cfg.job_queue.lock().unwrap();
    for job in queue.iter_mut()
        .filter(|j| matches!(j.status, PoolJobStatus::Pending | PoolJobStatus::Assigned))
        .take(1)
    {
        let worker = WorkerNode::new(&cfg.node_id);
        let result = worker.execute_job(&job.compute_job);
        
        if result.success {
            job.status = PoolJobStatus::Completed;
            job.result_hash = Some(result.trace_root.clone());
            
            if cfg.is_leader {
                // Leader: build block directly
                let _ = cfg.token_engine.transfer("genesis", &cfg.node_id, 10);
                build_and_broadcast_block(cfg, &result.trace_root);
                tracing::info!("🤖 LEADER {}: job={} block built", cfg.node_id, job.id);
            } else {
                // Worker: send result to leader
                let p2p = cfg.p2p_handle.clone();
                let workload_result = crate::p2p::message::WorkloadResult {
                    result_id: format!("res_{}", job.id),
                    workload_id: job.id.clone(),
                    worker_peer: cfg.node_id.clone(),
                    trace_root: result.trace_root.clone(),
                    execution_steps: result.execution_steps,
                    execution_time_ms: result.execution_time_ms,
                    success: true,
                    timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
                };
                tokio::spawn(async move {
                    let _ = p2p.command_tx.send(crate::p2p::node::P2PCommand::BroadcastResult { result: workload_result }).await;
                });
                tracing::info!("🤖 WORKER {}: job={} executed, result sent to leader", cfg.node_id, job.id);
            }
        } else {
            job.status = PoolJobStatus::Failed("error".into());
        }
    }
}

fn build_and_broadcast_block(cfg: &WorkerConfig, trace_root: &str) {
    let last = cfg.consensus.get_last_block();
    let coinbase = Transaction::new("network".to_string(), cfg.node_id.clone(), 100, 0);
    let mut block = Block::new(last.height + 1, last.hash.clone(), cfg.node_id.clone(), vec![coinbase]);
    block.hash = format!("block_{}_{}", block.height, &trace_root[..16]);
    let _ = cfg.consensus.blockchain.add_block(block.clone());
    
    let p2p = cfg.p2p_handle.clone();
    let h = block.height; let hash = block.hash.clone(); let prev = block.previous_hash.clone();
    let node = cfg.node_id.clone();
    tokio::spawn(async move { let _ = p2p.broadcast_block(h, hash, prev, node, 1, block.timestamp).await; });
}
