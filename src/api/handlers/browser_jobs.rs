use crate::merkle::hash::TraceHasher;
use crate::merkle::tree::MerkleTree;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

lazy_static::lazy_static! {
    pub static ref BROWSER_JOBS: Mutex<HashMap<String, BrowserJobRecord>> = 
        Mutex::new(HashMap::new());
}
use lazy_static::lazy_static;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserJobRecord {
    pub job_id: String,
    pub task_type: String,
    pub worker_id: Option<String>,
    pub input_hash: String,
    pub input_size: usize,
    pub output_hash: Option<String>,
    pub output_size: Option<usize>,
    pub status: String,
    pub progress: u8,
    pub execution_time_ms: Option<u64>,
    pub verification_status: Option<String>,
    pub merkle_root: Option<String>,
    pub started_at: u64,
    pub completed_at: Option<u64>,
    pub filename: Option<String>,
    // Phase 2: Proof / Block / Reward
    pub proof_hash: Option<String>,
    pub proof_verified: Option<bool>,
    pub proof_size: Option<usize>,
    pub block_height: Option<u64>,
    pub block_hash: Option<String>,
    pub reward: Option<u64>,
    pub reward_status: Option<String>,
}

impl BrowserJobRecord {
    pub fn new(job_id: &str, task_type: &str, input_hash: &str, input_size: usize, filename: Option<&str>) -> Self {
        BrowserJobRecord {
            job_id: job_id.to_string(),
            task_type: task_type.to_string(),
            worker_id: None,
            input_hash: input_hash.to_string(),
            input_size,
            output_hash: None,
            output_size: None,
            status: "Submitted".to_string(),
            progress: 0,
            execution_time_ms: None,
            verification_status: None,
            merkle_root: None,
            started_at: now(),
            completed_at: None,
            filename: filename.map(|f| f.to_string()),
            proof_hash: None,
            proof_verified: None,
            proof_size: None,
            block_height: None,
            block_hash: None,
            reward: None,
            reward_status: None,
        }
    }
}

pub fn register_browser_job(job_id: &str, task_type: &str, input_hash: &str, input_size: usize, worker_id: Option<&str>, filename: Option<&str>) {
    let mut jobs = BROWSER_JOBS.lock().unwrap();
    let mut record = BrowserJobRecord::new(job_id, task_type, input_hash, input_size, filename);
    if let Some(wid) = worker_id {
        record.worker_id = Some(wid.to_string());
        record.status = "Assigned".to_string();
    }
    jobs.insert(job_id.to_string(), record);
    tracing::info!("📋 BrowserJob registered: {} ({})", job_id, task_type);
}

pub fn update_job_progress(job_id: &str, progress: u8) {
    let mut jobs = BROWSER_JOBS.lock().unwrap();
    if let Some(record) = jobs.get_mut(job_id) {
        record.progress = progress;
        if record.status == "Submitted" || record.status == "Assigned" {
            record.status = "Running".to_string();
        }
    }
}

pub fn handle_browser_result(job_id: &str, worker_id: &str, success: bool, result_data: &serde_json::Value) -> Result<BrowserJobRecord, String> {
    let mut jobs = BROWSER_JOBS.lock().unwrap();
    let record = jobs.get_mut(job_id).ok_or_else(|| format!("Job {} not found", job_id))?;
    
    // Validate worker
    if let Some(ref assigned) = record.worker_id {
        if assigned != worker_id {
            return Err(format!("Worker mismatch: expected {}, got {}", assigned, worker_id));
        }
    }
    
    record.completed_at = Some(now());
    
    if !success {
        record.status = "Failed".to_string();
        record.verification_status = Some("Failed".to_string());
        let event = serde_json::json!({
            "type": "verification_failed",
            "job_id": job_id,
            "worker_id": worker_id,
            "reason": "worker reported failure"
        });
        let _ = crate::api::worker_ws::WORKER_EVENTS.send(event.to_string());
        return Ok(record.clone());
    }
    
    // Extract output data
    let output_data_str = result_data.get("thumbnail_data")
        .or_else(|| result_data.get("output_data"))
        .or_else(|| result_data.get("output"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    
    // Calculate output hash
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(output_data_str.as_bytes());
    let output_hash = format!("{:x}", hasher.finalize());
    
    // Calculate output size
    let output_size = output_data_str.len();
    
    // Get worker-reported hash if available
    let worker_output_hash = result_data.get("output_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    
    // Get execution time
    let execution_time_ms = result_data.get("execution_time_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    
    record.output_hash = Some(output_hash.clone());
    record.output_size = Some(output_size);
    record.execution_time_ms = Some(execution_time_ms);
    
    // ============ VERIFICATION ============
    let mut verification_passed = true;
    let mut failure_reasons: Vec<String> = Vec::new();
    
    if output_data_str.is_empty() {
        verification_passed = false;
        failure_reasons.push("No output data".to_string());
    }
    
    if !worker_output_hash.is_empty() && worker_output_hash != output_hash {
        verification_passed = false;
        failure_reasons.push(format!("Output hash mismatch: worker={}, computed={}", &worker_output_hash[..16], &output_hash[..16]));
    }
    
    let task_type = result_data.get("task_type")
        .and_then(|v| v.as_str())
        .unwrap_or(&record.task_type);
    if task_type != record.task_type {
        verification_passed = false;
        failure_reasons.push(format!("Task type mismatch: expected {}, got {}", record.task_type, task_type));
    }
    
    if verification_passed {
        record.verification_status = Some("Verified".to_string());
        record.status = "Completed".to_string();
        
        // ============ MERKLE COMMITMENT ============
        let mut leaves: Vec<String> = vec![
            TraceHasher::hash(&record.input_hash),
            TraceHasher::hash(&record.task_type),
            TraceHasher::hash(worker_id),
            TraceHasher::hash(&output_hash),
            TraceHasher::hash(job_id),
        ];
        leaves.sort();
        let tree = MerkleTree::new(leaves);
        record.merkle_root = Some(tree.root_hash.clone());
        
        // Broadcast events
        let event_v = serde_json::json!({
            "type": "verification_passed",
            "job_id": job_id,
            "worker_id": worker_id,
            "output_hash": &output_hash[..16],
            "output_size": output_size
        });
        let _ = crate::api::worker_ws::WORKER_EVENTS.send(event_v.to_string());
        
        let event_m = serde_json::json!({
            "type": "merkle_committed",
            "job_id": job_id,
            "worker_id": worker_id,
            "merkle_root": &tree.root_hash[..16]
        });
        let _ = crate::api::worker_ws::WORKER_EVENTS.send(event_m.to_string());
        
        let event_c = serde_json::json!({
            "type": "execution_completed",
            "job_id": job_id,
            "worker_id": worker_id,
            "status": "completed",
            "output_hash": &output_hash[..16],
            "merkle_root": &tree.root_hash[..16],
            "verification": "Verified"
        });
        let _ = crate::api::worker_ws::WORKER_EVENTS.send(event_c.to_string());
        
        tracing::info!("✅ BrowserJob {} verified, merkle_root={}", job_id, &tree.root_hash[..16]);
    } else {
        record.verification_status = Some("Failed".to_string());
        record.status = "Failed".to_string();
        let reason = failure_reasons.join("; ");
        
        let event = serde_json::json!({
            "type": "verification_failed",
            "job_id": job_id,
            "worker_id": worker_id,
            "reason": reason
        });
        let _ = crate::api::worker_ws::WORKER_EVENTS.send(event.to_string());
        
        tracing::warn!("❌ BrowserJob {} verification failed: {}", job_id, reason);
    }
    
    Ok(record.clone())
}

// ═══════════════════════════════════════════
// PHASE 2: Commitment Proof + Block + Reward
// ═══════════════════════════════════════════

/// Build a deterministic commitment trace from verified browser job data.
/// Same record ALWAYS produces the same trace.
pub fn build_commitment_trace(record: &BrowserJobRecord) -> Vec<u64> {
    let mut values: Vec<u64> = Vec::new();
    
    // input_hash as u64 chunks
    push_hash_as_u64s(&mut values, &record.input_hash);
    
    // output_hash as u64 chunks
    if let Some(ref out) = record.output_hash {
        push_hash_as_u64s(&mut values, out);
    }
    
    // task_type hash → u64
    let task_hash = TraceHasher::hash(&record.task_type);
    let task_u64 = hex_prefix_to_u64(&task_hash);
    values.push(task_u64);
    
    // worker_id hash → u64
    if let Some(ref wid) = record.worker_id {
        let wid_hash = TraceHasher::hash(wid);
        let wid_u64 = hex_prefix_to_u64(&wid_hash);
        values.push(wid_u64);
    }
    
    // job_id hash → u64
    let job_hash = TraceHasher::hash(&record.job_id);
    let job_u64 = hex_prefix_to_u64(&job_hash);
    values.push(job_u64);
    
    // merkle_root as u64 chunks
    if let Some(ref merkle) = record.merkle_root {
        push_hash_as_u64s(&mut values, merkle);
    }
    
    values
}

fn push_hash_as_u64s(values: &mut Vec<u64>, hash_str: &str) {
    let bytes = hex::decode(hash_str).unwrap_or(vec![0; 32]);
    for chunk in bytes.chunks(8) {
        let mut arr = [0u8; 8];
        for (i, &b) in chunk.iter().enumerate() {
            if i < 8 { arr[i] = b; }
        }
        values.push(u64::from_le_bytes(arr));
    }
}

fn hex_prefix_to_u64(hex_str: &str) -> u64 {
    u64::from_str_radix(&hex_str[..16.min(hex_str.len())], 16).unwrap_or(0)
}

/// Finalize a browser job: generate commitment proof, build block, distribute reward.
/// Must be called after verification + merkle commitment succeeded.
/// Idempotent: will not double-finalize.
pub fn finalize_browser_job(
    job_id: &str,
    consensus: &crate::consensus::network::ConsensusNetwork,
    token_engine: &crate::economic::token::TokenEngine,
) -> Result<BrowserJobRecord, String> {
    // Check idempotency
    {
        let jobs = BROWSER_JOBS.lock().unwrap();
        if let Some(record) = jobs.get(job_id) {
            if record.block_height.is_some() || record.reward_status.as_deref() == Some("distributed") {
                tracing::info!("🔁 BrowserJob {} already finalized, skipping", job_id);
                return Ok(record.clone());
            }
            if record.verification_status.as_deref() != Some("Verified") {
                return Err(format!("Job {} not verified, cannot finalize", job_id));
            }
            if record.merkle_root.is_none() {
                return Err(format!("Job {} has no merkle root, cannot finalize", job_id));
            }
        } else {
            return Err(format!("Job {} not found", job_id));
        }
    }
    
    let record_snapshot;
    {
        let jobs = BROWSER_JOBS.lock().unwrap();
        record_snapshot = jobs.get(job_id).cloned()
            .ok_or_else(|| format!("Job {} not found", job_id))?;
    }
    
    let worker_id = record_snapshot.worker_id.clone().unwrap_or_default();
    
    // ============ BUILD COMMITMENT TRACE ============
    let trace = build_commitment_trace(&record_snapshot);
    
    // ============ GENERATE PROOF ============
    let proof = crate::stark::simple_stark::SimpleStark::prove(&trace);
    let proof_hash = hex::encode(&proof.trace_hash);
    let proof_size = serde_json::to_vec(&proof).map(|v| v.len()).unwrap_or(0);
    
    // ============ INDEPENDENT VERIFICATION ============
    let verified = crate::stark::simple_stark::SimpleStark::quick_verify(&trace, &proof);
    
    // Store proof metadata
    {
        let mut jobs = BROWSER_JOBS.lock().unwrap();
        if let Some(record) = jobs.get_mut(job_id) {
            record.proof_hash = Some(proof_hash.clone());
            record.proof_size = Some(proof_size);
            record.proof_verified = Some(verified);
        }
    }
    
    if !verified {
        let event = serde_json::json!({
            "type": "commitment_proof_failed",
            "job_id": job_id,
            "worker_id": worker_id,
            "proof_hash": &proof_hash[..16]
        });
        let _ = crate::api::worker_ws::WORKER_EVENTS.send(event.to_string());
        return Err(format!("Commitment proof verification failed for job {}", job_id));
    }
    
    // Broadcast proof events
    let event_pg = serde_json::json!({
        "type": "commitment_proof_generated",
        "job_id": job_id,
        "worker_id": worker_id,
        "proof_hash": &proof_hash[..16],
        "proof_size": proof_size
    });
    let _ = crate::api::worker_ws::WORKER_EVENTS.send(event_pg.to_string());
    
    let event_pv = serde_json::json!({
        "type": "commitment_proof_verified",
        "job_id": job_id,
        "worker_id": worker_id,
        "proof_hash": &proof_hash[..16]
    });
    let _ = crate::api::worker_ws::WORKER_EVENTS.send(event_pv.to_string());
    
    tracing::info!("🔐 BrowserJob {} commitment proof verified", job_id);
    
    // ============ BUILD BLOCK ============
    let last_block = consensus.get_last_block();
    let coinbase = crate::consensus::types::Transaction::new(
        "network".to_string(),
        worker_id.clone(),
        100,
        0,
    );
    let mut block = crate::consensus::types::Block::new(
        last_block.height + 1,
        last_block.hash.clone(),
        "browser_chain".to_string(),
        vec![coinbase],
    );
    block.hash = format!("browser_block_{}_{}", block.height, &proof_hash[..16]);
    
    // ============ BLOCK ACCEPTANCE ============
    match consensus.blockchain.add_block(block.clone()) {
        Ok(()) => {
            let block_height = block.height;
            let block_hash = block.hash.clone();
            
            // Store block info
            {
                let mut jobs = BROWSER_JOBS.lock().unwrap();
                if let Some(record) = jobs.get_mut(job_id) {
                    record.block_height = Some(block_height);
                    record.block_hash = Some(block_hash.clone());
                }
            }
            
            let event_ba = serde_json::json!({
                "type": "block_accepted",
                "job_id": job_id,
                "block_height": block_height,
                "block_hash": &block_hash[..16],
                "worker_id": worker_id
            });
            let _ = crate::api::worker_ws::WORKER_EVENTS.send(event_ba.to_string());
            
            tracing::info!("⛓️ BrowserJob {} block accepted: height={}", job_id, block_height);
            
            // ============ REWARD ============
            token_engine.mint(&worker_id, 10);
            
            // Store reward info
            {
                let mut jobs = BROWSER_JOBS.lock().unwrap();
                if let Some(record) = jobs.get_mut(job_id) {
                    record.reward = Some(10);
                    record.reward_status = Some("distributed".to_string());
                }
            }
            
            let event_rd = serde_json::json!({
                "type": "reward_distributed",
                "job_id": job_id,
                "worker_id": worker_id,
                "amount": 10
            });
            let _ = crate::api::worker_ws::WORKER_EVENTS.send(event_rd.to_string());
            
            tracing::info!("💰 BrowserJob {} reward distributed: 10 tokens to {}", job_id, worker_id);
        }
        Err(e) => {
            let event = serde_json::json!({
                "type": "block_rejected",
                "job_id": job_id,
                "worker_id": worker_id,
                "error": e
            });
            let _ = crate::api::worker_ws::WORKER_EVENTS.send(event.to_string());
            return Err(format!("Block rejected: {}", e));
        }
    }
    
    BROWSER_JOBS.lock().unwrap().get(job_id).cloned()
        .ok_or_else(|| format!("Job {} not found after finalization", job_id))
}

pub fn get_browser_job(job_id: &str) -> Option<BrowserJobRecord> {
    BROWSER_JOBS.lock().unwrap().get(job_id).cloned()
}

pub fn is_already_finalized(job_id: &str) -> bool {
    BROWSER_JOBS.lock().unwrap()
        .get(job_id)
        .map(|r| r.block_height.is_some() || r.reward_status.as_deref() == Some("distributed"))
        .unwrap_or(false)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_job() {
        register_browser_job("job_test1", "thumbnail", "abc123", 1024, None, Some("test.jpg"));
        let job = get_browser_job("job_test1").unwrap();
        assert_eq!(job.job_id, "job_test1");
        assert_eq!(job.task_type, "thumbnail");
        assert_eq!(job.input_hash, "abc123");
        assert_eq!(job.status, "Submitted");
    }

    #[test]
    fn test_handle_result_verified() {
        register_browser_job("job_test2", "thumbnail", "input_hash_1", 500, Some("worker_1"), None);
        BROWSER_JOBS.lock().unwrap().get_mut("job_test2").unwrap().worker_id = Some("worker_1".to_string());
        
        let result = serde_json::json!({
            "task_type": "thumbnail",
            "thumbnail_data": "base64_output_data",
            "output_hash": "",
            "execution_time_ms": 1500
        });
        
        let record = handle_browser_result("job_test2", "worker_1", true, &result).unwrap();
        assert_eq!(record.status, "Completed");
        assert_eq!(record.verification_status, Some("Verified".to_string()));
        assert!(record.output_hash.is_some());
        assert!(record.merkle_root.is_some());
    }

    #[test]
    fn test_handle_result_failed_worker() {
        register_browser_job("job_test3", "hash", "input_hash_2", 200, Some("worker_2"), None);
        BROWSER_JOBS.lock().unwrap().get_mut("job_test3").unwrap().worker_id = Some("worker_2".to_string());
        
        let result = serde_json::json!({});
        let record = handle_browser_result("job_test3", "worker_2", false, &result).unwrap();
        assert_eq!(record.status, "Failed");
        assert_eq!(record.verification_status, Some("Failed".to_string()));
    }

    #[test]
    fn test_wrong_worker_rejected() {
        register_browser_job("job_test4", "hash", "hash_in", 100, Some("worker_a"), None);
        BROWSER_JOBS.lock().unwrap().get_mut("job_test4").unwrap().worker_id = Some("worker_a".to_string());
        
        let result = serde_json::json!({"output_data": "test"});
        let err = handle_browser_result("job_test4", "worker_b", true, &result);
        assert!(err.is_err());
    }

    #[test]
    fn test_hash_mismatch_detected() {
        register_browser_job("job_test5", "thumbnail", "input_h", 300, Some("worker_x"), None);
        BROWSER_JOBS.lock().unwrap().get_mut("job_test5").unwrap().worker_id = Some("worker_x".to_string());
        
        let result = serde_json::json!({
            "task_type": "thumbnail",
            "thumbnail_data": "real_output_here",
            "output_hash": "0000000000000000000000000000000000000000000000000000000000000000",
        });
        
        let record = handle_browser_result("job_test5", "worker_x", true, &result).unwrap();
        assert_eq!(record.verification_status, Some("Failed".to_string()));
        assert_eq!(record.status, "Failed");
    }

    #[test]
    fn test_merkle_deterministic() {
        register_browser_job("job_det", "thumbnail", "hash_in", 100, Some("w1"), None);
        BROWSER_JOBS.lock().unwrap().get_mut("job_det").unwrap().worker_id = Some("w1".to_string());
        
        let result = serde_json::json!({"thumbnail_data": "same_output", "task_type": "thumbnail"});
        
        let r1 = handle_browser_result("job_det", "w1", true, &result).unwrap();
        
        BROWSER_JOBS.lock().unwrap().remove("job_det");
        register_browser_job("job_det", "thumbnail", "hash_in", 100, Some("w1"), None);
        BROWSER_JOBS.lock().unwrap().get_mut("job_det").unwrap().worker_id = Some("w1".to_string());
        
        let r2 = handle_browser_result("job_det", "w1", true, &result).unwrap();
        
        assert_eq!(r1.merkle_root, r2.merkle_root);
        assert_eq!(r1.output_hash, r2.output_hash);
    }

    // ═══ PHASE 2 TESTS ═══

    #[test]
    fn test_commitment_trace_deterministic() {
        register_browser_job("job_ct", "thumbnail", "aaa111", 100, Some("w1"), None);
        BROWSER_JOBS.lock().unwrap().get_mut("job_ct").unwrap().worker_id = Some("w1".to_string());
        BROWSER_JOBS.lock().unwrap().get_mut("job_ct").unwrap().output_hash = Some("bbb222".to_string());
        BROWSER_JOBS.lock().unwrap().get_mut("job_ct").unwrap().merkle_root = Some("ccc333".to_string());
        
        let record = get_browser_job("job_ct").unwrap();
        let t1 = build_commitment_trace(&record);
        let t2 = build_commitment_trace(&record);
        assert_eq!(t1, t2);
    }

    #[test]
    fn test_commitment_proof_generated() {
        register_browser_job("job_cp", "thumbnail", "ddd444", 200, Some("w2"), None);
        BROWSER_JOBS.lock().unwrap().get_mut("job_cp").unwrap().worker_id = Some("w2".to_string());
        BROWSER_JOBS.lock().unwrap().get_mut("job_cp").unwrap().output_hash = Some("eee555".to_string());
        BROWSER_JOBS.lock().unwrap().get_mut("job_cp").unwrap().merkle_root = Some("fff666".to_string());
        
        let record = get_browser_job("job_cp").unwrap();
        let trace = build_commitment_trace(&record);
        let proof = crate::stark::simple_stark::SimpleStark::prove(&trace);
        assert!(!proof.trace_hash.is_empty());
        assert!(proof.trace_length > 0);
    }

    #[test]
    fn test_commitment_proof_independent_verify() {
        register_browser_job("job_iv", "hash", "ggg777", 300, Some("w3"), None);
        BROWSER_JOBS.lock().unwrap().get_mut("job_iv").unwrap().worker_id = Some("w3".to_string());
        BROWSER_JOBS.lock().unwrap().get_mut("job_iv").unwrap().output_hash = Some("hhh888".to_string());
        BROWSER_JOBS.lock().unwrap().get_mut("job_iv").unwrap().merkle_root = Some("iii999".to_string());
        
        let record = get_browser_job("job_iv").unwrap();
        let trace = build_commitment_trace(&record);
        let proof = crate::stark::simple_stark::SimpleStark::prove(&trace);
        assert!(crate::stark::simple_stark::SimpleStark::quick_verify(&trace, &proof));
    }

    #[test]
    fn test_tampered_record_rejected() {
        register_browser_job("job_tr", "thumbnail", "jjj000", 400, Some("w4"), None);
        BROWSER_JOBS.lock().unwrap().get_mut("job_tr").unwrap().worker_id = Some("w4".to_string());
        BROWSER_JOBS.lock().unwrap().get_mut("job_tr").unwrap().output_hash = Some("kkk111".to_string());
        BROWSER_JOBS.lock().unwrap().get_mut("job_tr").unwrap().merkle_root = Some("lll222".to_string());
        
        let record = get_browser_job("job_tr").unwrap();
        let trace = build_commitment_trace(&record);
        let proof = crate::stark::simple_stark::SimpleStark::prove(&trace);
        
        // Tamper: different trace
        let mut tampered = trace.clone();
        if !tampered.is_empty() { tampered[0] = tampered[0].wrapping_add(1); }
        
        assert!(!crate::stark::simple_stark::SimpleStark::quick_verify(&tampered, &proof));
    }

    #[test]
    fn test_is_already_finalized() {
        register_browser_job("job_idem", "hash", "mmm333", 500, Some("w5"), None);
        assert!(!is_already_finalized("job_idem"));
        
        BROWSER_JOBS.lock().unwrap().get_mut("job_idem").unwrap().block_height = Some(5);
        assert!(is_already_finalized("job_idem"));
    }

    #[test]
    fn test_no_finalize_without_verification() {
        register_browser_job("job_nv", "hash", "nnn444", 600, Some("w6"), None);
        // Not verified, not completed
        
        let consensus = crate::consensus::network::ConsensusNetwork::new();
        let token = crate::economic::token::TokenEngine::new();
        
        let result = finalize_browser_job("job_nv", &consensus, &token);
        assert!(result.is_err());
    }
}
