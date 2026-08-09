use crate::api::handlers::AppState;
use crate::p2p::proof_network::{ComputeJob, InstructionData};
use crate::p2p::message::WorkloadAssignment;
use crate::p2p::node::P2PCommand;
use tracing;
use axum::{extract::{Path, State}, Json};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use lazy_static::lazy_static;

lazy_static! {
    static ref COMPLETED_HASHES: Mutex<Vec<String>> = Mutex::new(Vec::new());
}

pub async fn submit_job_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let program = body["program"].as_array().cloned().unwrap_or_default();
    let instructions: Vec<InstructionData> = program.iter().map(|inst| {
        InstructionData {
            opcode: inst["opcode"].as_str().unwrap_or("HALT").to_string(),
            params: inst["params"].as_array().map(|a| a.iter().map(|v| v.as_u64().unwrap_or(0)).collect()).unwrap_or_default(),
        }
    }).collect();

    // Deduplication: hash the program
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for inst in &instructions {
        hasher.update(inst.opcode.as_bytes());
        for p in &inst.params { hasher.update(&p.to_le_bytes()); }
    }
    let program_hash = format!("{:x}", hasher.finalize());
    
    let pid = std::process::id();
    let mut hashes = COMPLETED_HASHES.lock().unwrap();
    let hash_count_before = hashes.len();
    let is_duplicate = hashes.contains(&program_hash);
    tracing::info!("🔍 DEDUP: pid={} hash={} hash_count_before={} is_duplicate={}", pid, program_hash, hash_count_before, is_duplicate);
    if !is_duplicate {
        hashes.push(program_hash.clone());
    }
    let hash_count_after = hashes.len();
    drop(hashes);
    tracing::info!("🔍 DEDUP: hash_count_after={}", hash_count_after);
    
    if is_duplicate {
        tracing::info!("🔍 DEDUP: REJECTING duplicate");
        return Json(serde_json::json!({ "status": "rejected", "reason": "duplicate", "hash": program_hash }));
    }

    let job_id = format!("job_{:08x}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs());
    
    // Register in BrowserJob tracking
    let task_type = body["task_type"].as_str().unwrap_or("hash").to_string();
    let input_data = body["input_data"].as_str().unwrap_or("").to_string();
    let input_size = input_data.len();
    let mut hasher = Sha256::new();
    hasher.update(input_data.as_bytes());
    let input_hash = format!("{:x}", hasher.finalize());
    
    
    crate::api::handlers::browser_jobs::register_browser_job(
        &job_id,
        &task_type,
        &input_hash,
        input_size,
        None,
        None,
    );
    
    
    // Try to dispatch to a browser worker first
    let assigned_worker = crate::api::worker_registry::find_worker_for_task(&task_type);
    
    if let Some(ref wid) = assigned_worker {
        let job_msg = serde_json::json!({
            "type": "job",
            "job_id": job_id,
            "task": task_type,
            "task_type": task_type, "input_data": input_data
        });
        if crate::api::worker_registry::send_to_worker(wid, &job_msg.to_string()).is_ok() {
            crate::api::worker_registry::set_worker_busy(wid, &job_id);
            tracing::info!("🎯 Job {} dispatched to browser worker {}", job_id, wid);
            let event = serde_json::json!({
                "type": "job_assigned",
                "job_id": job_id,
                "worker_id": wid,
                "task": task_type
            });
            let _ = crate::api::worker_ws::WORKER_EVENTS.send(event.to_string());
        }
    }
    
    let p2p = state.p2p_handle.clone();
    let assignment = WorkloadAssignment {
        assignment_id: format!("asgn_{}", job_id),
        workload_id: job_id.clone(),
        program: instructions.clone(),
        assigned_peer: "node_beta".to_string(),
        issuer_peer: "api".to_string(),
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
    };
    tokio::spawn(async move {
        let _ = p2p.command_tx.send(P2PCommand::BroadcastAssignment { assignment }).await;
    });

    Json(serde_json::json!({ "status": "submitted", "job_id": job_id, "hash": program_hash }))
}

pub async fn list_jobs_handler() -> Json<serde_json::Value> { Json(serde_json::json!({ "jobs": [] })) }
pub async fn pending_jobs_handler() -> Json<serde_json::Value> { Json(serde_json::json!({ "pending": 0 })) }
pub async fn completed_jobs_handler() -> Json<serde_json::Value> { Json(serde_json::json!({ "completed": 0 })) }
pub async fn get_job_handler(Path(_id): Path<String>) -> Json<serde_json::Value> { Json(serde_json::json!({ "found": false })) }
pub async fn list_workers_handler() -> Json<serde_json::Value> { Json(serde_json::json!({ "workers": [{"id":"node_alpha"},{"id":"node_beta"},{"id":"node_gamma"}] })) }
pub async fn process_jobs_handler() -> Json<serde_json::Value> { Json(serde_json::json!({ "processed": 0 })) }

// ═══ MULTIPART FILE UPLOAD ═══
use axum::extract::Multipart;

pub async fn upload_job_handler(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Json<serde_json::Value> {
    let mut task_type = String::from("hash");
    let mut file_bytes: Vec<u8> = Vec::new();
    let mut filename = String::from("unknown");
    
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "task_type" => {
                task_type = field.text().await.unwrap_or_default();
            }
            "file" => {
                filename = field.file_name().unwrap_or("unknown").to_string();
                file_bytes = field.bytes().await.unwrap_or_default().to_vec();
            }
            _ => {}
        }
    }
    
    if file_bytes.is_empty() {
        return Json(serde_json::json!({"status":"error","reason":"no_file"}));
    }
    
    // Calculate SHA-256 of complete file
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&file_bytes);
    let input_hash = format!("{:x}", hasher.finalize());
    
    let job_id = format!("job_{:08x}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs());
    
    // Store file temporarily
    let upload_dir = format!("/tmp/compute_chain/uploads/{}", job_id);
    std::fs::create_dir_all(&upload_dir).ok();
    std::fs::write(format!("{}/input", upload_dir), &file_bytes).ok();
    
    // Dispatch to worker
    let assigned = crate::api::worker_registry::find_worker_for_task(&task_type);
    if let Some(ref wid) = assigned {
        let job_msg = serde_json::json!({
            "type": "job",
            "job_id": job_id,
            "task_type": task_type,
            "input_data": base64::encode(&file_bytes),
            "input_hash": input_hash,
            "filename": filename,
            "size": file_bytes.len()
        });
        if crate::api::worker_registry::send_to_worker(wid, &job_msg.to_string()).is_ok() {
            crate::api::worker_registry::set_worker_busy(wid, &job_id);
            let event = serde_json::json!({
                "type": "job_assigned",
                "job_id": job_id,
                "worker_id": wid,
                "task_type": task_type
            });
            let _ = crate::api::worker_ws::WORKER_EVENTS.send(event.to_string());
        }
    }
    
    // Register in BrowserJob tracking BEFORE P2P consumes 'assigned'
    let assigned_peer = assigned.clone().unwrap_or_default();
    crate::api::handlers::browser_jobs::register_browser_job(
        &job_id,
        &task_type,
        &input_hash,
        file_bytes.len(),
        assigned.as_deref(),
        Some(&filename),
    );

    // Broadcast job_submitted event
    let event = serde_json::json!({
        "type": "job_submitted",
        "job_id": job_id,
        "task_type": task_type,
        "input_hash": &input_hash[..16],
        "input_size": file_bytes.len(),
        "worker_id": assigned,
        "filename": filename
    });
    let _ = crate::api::worker_ws::WORKER_EVENTS.send(event.to_string());

    // Also send via P2P
    let p2p = state.p2p_handle.clone();
    let assignment = WorkloadAssignment {
        assignment_id: format!("asgn_{}", job_id),
        workload_id: job_id.clone(),
        program: vec![],
        assigned_peer: assigned_peer,
        issuer_peer: "api".to_string(),
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
    };
    tokio::spawn(async move {
        let _ = p2p.command_tx.send(P2PCommand::BroadcastAssignment { assignment }).await;
    });

    Json(serde_json::json!({
        "status": "submitted",
        "job_id": job_id,
        "task_type": task_type,
        "input_hash": input_hash,
        "filename": filename,
        "size": file_bytes.len()
    }))
}
