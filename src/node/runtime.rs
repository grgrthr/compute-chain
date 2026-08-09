use crate::node::identity::{NodeInfo, NodeRole};
use crate::node::registry::PeerRegistry;
use crate::node::event_handler::{EventContext, handle_p2p_event};
use crate::node::worker::{WorkerConfig, run_worker_loop};
use crate::compute_pool::{PoolJob, PoolJobStatus};
use crate::p2p::P2PHandle;
use crate::p2p::message::WorkloadAssignment;
use crate::p2p::node::P2PCommand;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct DistributedRuntime {
    pub node_info: NodeInfo,
    pub peer_registry: Arc<Mutex<PeerRegistry>>,
    pub job_queue: Arc<Mutex<Vec<PoolJob>>>,
    pub p2p_handle: P2PHandle,
    pub is_leader: bool,
}

impl DistributedRuntime {
    pub fn new(node_id: &str, role: NodeRole, p2p_handle: P2PHandle, is_leader: bool) -> Self {
        let node_info = NodeInfo::new(node_id, role);
        let peer_registry = Arc::new(Mutex::new(PeerRegistry::new(node_info.clone())));
        let job_queue = Arc::new(Mutex::new(Vec::new()));
        
        DistributedRuntime {
            node_info,
            peer_registry,
            job_queue,
            p2p_handle,
            is_leader,
        }
    }
    
    pub fn node_id(&self) -> String { self.node_info.node_id.clone() }
    
    pub async fn submit_job(&self, instructions: Vec<crate::p2p::proof_network::InstructionData>) -> String {
        let job_id = format!("job_{:08x}", now());
        let program = instructions.clone();
        
        // Choose worker
        let assigned = self.peer_registry.lock().unwrap()
            .get_lowest_load_worker()
            .unwrap_or_else(|| "node_beta".to_string());
        
        tracing::info!("🎯 Leader assigns job {} to worker {}", job_id, assigned);
        
        // Add to local queue
        let job = crate::p2p::proof_network::ComputeJob { job_id: job_id.clone(), instructions: program.clone(), input_registers: vec![0;8], difficulty: 1 };
        self.job_queue.lock().unwrap().push(PoolJob::new(job));
        
        // Broadcast assignment to the chosen worker
        let assignment = WorkloadAssignment {
            assignment_id: format!("asgn_{}", job_id),
            workload_id: job_id.clone(),
            program,
            assigned_peer: assigned,
            issuer_peer: self.node_id(),
            timestamp: now(),
        };
        let p2p = self.p2p_handle.clone();
        tokio::spawn(async move {
            let _ = p2p.command_tx.send(P2PCommand::BroadcastAssignment { assignment }).await;
        });
        
        job_id
    }
    
    pub fn get_job_queue(&self) -> Arc<Mutex<Vec<PoolJob>>> {
        self.job_queue.clone()
    }
    
    pub fn get_event_context(&self, consensus: Option<Arc<crate::consensus::network::ConsensusNetwork>>) -> EventContext {
        EventContext {
            node_id: self.node_id(),
            job_queue: self.job_queue.clone(),
            p2p_handle: self.p2p_handle.clone(),
            consensus,
            token_engine: None,
        }
    }
    
    pub fn get_worker_config(&self, consensus: Arc<crate::consensus::network::ConsensusNetwork>, token_engine: Arc<crate::economic::token::TokenEngine>) -> WorkerConfig {
        WorkerConfig {
            node_id: self.node_id(),
            is_leader: self.is_leader,
            job_queue: self.job_queue.clone(),
            consensus,
            token_engine,
            p2p_handle: self.p2p_handle.clone(),
        }
    }
}

fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() }
