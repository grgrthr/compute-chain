use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct WorkerInfo {
    pub worker_id: String,
    pub capabilities: Vec<String>,
    pub sender: mpsc::UnboundedSender<String>,
    pub status: WorkerStatus,
    pub current_job: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WorkerStatus { Idle, Busy, Offline }

lazy_static::lazy_static! {
    pub static ref WORKER_REGISTRY: Arc<Mutex<HashMap<String, WorkerInfo>>> = 
        Arc::new(Mutex::new(HashMap::new()));
}
use lazy_static::lazy_static;

impl WorkerInfo {
    pub fn new(id: &str, capabilities: Vec<String>, sender: mpsc::UnboundedSender<String>) -> Self {
        WorkerInfo { worker_id: id.to_string(), capabilities, sender, status: WorkerStatus::Idle, current_job: None }
    }
}

pub fn register_worker(id: &str, capabilities: Vec<String>, sender: mpsc::UnboundedSender<String>) {
    let mut registry = WORKER_REGISTRY.lock().unwrap();
    registry.insert(id.to_string(), WorkerInfo::new(id, capabilities, sender));
    tracing::info!("📋 Worker registered: {} ({})", id, registry.len());
}

pub fn unregister_worker(id: &str) {
    let mut registry = WORKER_REGISTRY.lock().unwrap();
    registry.remove(id);
    tracing::info!("📋 Worker unregistered: {} ({})", id, registry.len());
}

pub fn find_worker_for_task(task: &str) -> Option<String> {
    let registry = WORKER_REGISTRY.lock().unwrap();
    for (id, info) in registry.iter() {
        if info.status == WorkerStatus::Idle && info.capabilities.iter().any(|c| c.contains(task) || task.contains(c)) {
            return Some(id.clone());
        }
    }
    // Fallback: any idle worker
    registry.iter()
        .find(|(_, info)| info.status == WorkerStatus::Idle)
        .map(|(id, _)| id.clone())
}

pub fn send_to_worker(worker_id: &str, message: &str) -> Result<(), String> {
    let registry = WORKER_REGISTRY.lock().unwrap();
    if let Some(info) = registry.get(worker_id) {
        info.sender.send(message.to_string())
            .map_err(|e| format!("Send failed: {}", e))
    } else {
        Err(format!("Worker {} not found", worker_id))
    }
}

pub fn set_worker_busy(worker_id: &str, job_id: &str) {
    let mut registry = WORKER_REGISTRY.lock().unwrap();
    if let Some(info) = registry.get_mut(worker_id) {
        info.status = WorkerStatus::Busy;
        info.current_job = Some(job_id.to_string());
    }
}

pub fn set_worker_idle(worker_id: &str) {
    let mut registry = WORKER_REGISTRY.lock().unwrap();
    if let Some(info) = registry.get_mut(worker_id) {
        info.status = WorkerStatus::Idle;
        info.current_job = None;
    }
}

pub fn worker_count() -> usize { WORKER_REGISTRY.lock().unwrap().len() }
