use crate::api::handlers::AppState;
use crate::consensus::types::{Block, Transaction};
use crate::p2p::{PbftPrePrepareMsg, PbftVoteMsg};
use crate::trace::tracer::Tracer;
use crate::vm::cpu::Cpu;
use crate::vm::executor::Executor;
use crate::vm::instruction::Instruction;
use crate::vm::memory::Memory;
use crate::vm::program::Program;
use axum::{extract::State, Json};
use std::collections::HashMap;
use std::sync::Arc;

pub async fn receive_block_handler(
    State(state): State<Arc<AppState>>,
    Json(block_data): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let height = block_data["height"].as_u64().unwrap_or(0);
    let hash = block_data["hash"].as_str().unwrap_or("");
    let validator = block_data["validator_id"].as_str().unwrap_or("unknown");

    tracing::info!(
        "📦 Received block from peer: height={}, hash={}",
        height,
        &hash[..16.min(hash.len())]
    );
    let current_height = state.consensus.get_blockchain_height();

    if validator.is_empty() || hash.is_empty() {
        return Json(serde_json::json!({"status": "rejected", "reason": "invalid block data"}));
    }
    if height == 0 {
        return Json(serde_json::json!({"status": "rejected", "reason": "genesis block only"}));
    }
    let validators = state.consensus.get_validator_set();
    if !validators.contains(&validator.to_string()) {
        return Json(serde_json::json!({"status": "rejected", "reason": "unregistered validator"}));
    }

    if height > current_height {
        let last_block = state.consensus.get_last_block();
        let tx = Transaction::new("network".to_string(), validator.to_string(), 100, 0);
        let mut block = Block::new(height, last_block.hash, validator.to_string(), vec![tx]);
        block.hash = hash.to_string();
        match state.consensus.blockchain.add_block(block) {
            Ok(_) => {
                tracing::info!("✅ Block accepted: height={}", height);
                Json(serde_json::json!({"status": "accepted", "height": height}))
            }
            Err(e) => Json(serde_json::json!({"status": "error", "error": e})),
        }
    } else if height == current_height {
        let my_hash = state.consensus.get_last_block().hash.clone();
        if hash != my_hash {
            tracing::info!("🔄 Fork auto-resolving");
            let tx = Transaction::new("network".to_string(), validator.to_string(), 100, 0);
            let mut block = Block::new(height, my_hash, validator.to_string(), vec![tx]);
            block.hash = hash.to_string();
            match state.consensus.blockchain.add_block(block) {
                Ok(_) => Json(serde_json::json!({"status": "fork_resolved", "height": height})),
                Err(e) => Json(serde_json::json!({"status": "error", "error": e})),
            }
        } else {
            Json(serde_json::json!({"status": "already_synced", "height": height}))
        }
    } else {
        Json(serde_json::json!({"status": "skipped", "height": height, "current": current_height}))
    }
}

pub async fn block_vote_handler(
    State(state): State<Arc<AppState>>,
    Json(vote_data): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let height = vote_data["height"].as_u64().unwrap_or(0);
    tracing::info!("🗳 Vote request: height={}", height);
    let current = state.consensus.get_blockchain_height();
    if height == current + 1 {
        Json(serde_json::json!({"vote": "approve", "height": height}))
    } else {
        Json(serde_json::json!({"vote": "reject", "height": height, "current": current}))
    }
}

pub async fn mine_block_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let validator_id = request["validator_id"].as_str().unwrap_or("validator1");
    tracing::info!("⛏️ Mine block request: validator={}", validator_id);
    let program = request["program"].as_array();
    let mut instructions = Vec::new();
    if let Some(prog) = program {
        for inst in prog {
            let opcode = inst["opcode"].as_str().unwrap_or("HALT");
            let params: Vec<u64> = inst["params"]
                .as_array()
                .map(|a| a.iter().map(|v| v.as_u64().unwrap_or(0)).collect())
                .unwrap_or_default();
            let instruction = match opcode {
                "MOV" => Instruction::Mov {
                    register: params[0] as usize,
                    value: params.get(1).copied().unwrap_or(0),
                },
                "ADD" => Instruction::Add {
                    destination: params[0] as usize,
                    source: params.get(1).copied().unwrap_or(0) as usize,
                },
                "MUL" => Instruction::Mul {
                    destination: params[0] as usize,
                    source: params.get(1).copied().unwrap_or(0) as usize,
                },
                "HALT" => Instruction::Halt,
                _ => Instruction::Halt,
            };
            instructions.push(instruction);
        }
    }
    let mut cpu = Cpu::new();
    let mut memory = Memory::new(65536);
    let mut tracer = Tracer::new();
    let program = Program::new(instructions);
    let mut step_counter = 0;
    while !cpu.halted {
        let result = Executor::step(&mut cpu, &mut memory, &program);
        if let Some(step) = result {
            tracer.record(
                step_counter,
                step.pc,
                step.instruction.clone(),
                step.registers_before.to_vec(),
                step.registers_after.to_vec(),
            );
            step_counter += 1;
        } else {
            break;
        }
    }
    use crate::stark::prover::ComputeProver;
    use crate::stark::simple_stark::SimpleProof;
    use crate::stark::trace::build_trace_from_records;
    let memory_states = vec![(vec![], vec![]); tracer.records.len()];
    let trace = build_trace_from_records(&tracer.records, &memory_states);
    let prover = ComputeProver::new();
    match prover.prove(&trace) {
        Ok(stark_proof) => {
            let last_block = state.consensus.get_last_block();

            // ============ PBFT Consensus ============
            let pbft = &state.consensus.pbft;
            let current_leader = pbft.get_primary();

            tracing::info!(
                "🔍 Current PBFT leader: {}, requesting validator: {}",
                current_leader,
                validator_id
            );

            if validator_id != current_leader {
                tracing::warn!(
                    "⚠️ {} is not the current leader ({}). Triggering view change...",
                    validator_id,
                    current_leader
                );
                pbft.trigger_view_change();
                let new_leader = pbft.get_primary();
                tracing::info!("🔄 New leader after view change: {}", new_leader);
                if validator_id != new_leader {
                    return Json(serde_json::json!({
                        "status": "error",
                        "message": format!("Not the current leader. Leader is: {}. Try again or wait for view change.", new_leader)
                    }));
                }
            }

            // Get transactions from mempool
            let mempool_txs = {
                let mempool = state.mempool.inner.read().unwrap();
                mempool.get_top(10)
            };
            let mut transactions = mempool_txs;
            let coinbase =
                Transaction::new("network".to_string(), validator_id.to_string(), 100, 0);
            transactions.insert(0, coinbase);
            let tx_ids: Vec<String> = transactions.iter().map(|tx| tx.id.clone()).collect();
            let mut block = Block::new(
                last_block.height + 1,
                last_block.hash,
                validator_id.to_string(),
                transactions,
            );
            block.compute_proof = Some(SimpleProof {
                commitments: vec![],
                trace_hash: stark_proof.trace_hash.clone(),
                trace_length: stark_proof.trace_length,
                final_state: stark_proof.final_registers.to_vec(),
                memory_root: stark_proof.memory_root.as_bytes().to_vec(),
                fri_layers: vec![],
            });

            // ============ PBFT: PrePrepare (local) ============
            if !pbft.pre_prepare(&block, validator_id) {
                return Json(
                    serde_json::json!({ "status": "error", "message": "PrePrepare failed - not the leader" }),
                );
            }

            // ============ PBFT: Broadcast PrePrepare (fire-and-forget) ============
            let round = *pbft.current_round.lock().unwrap();
            let view = *pbft.current_view.lock().unwrap();
            let p2p_clone = state.p2p_handle.clone();
            let block_hash_clone = block.hash.clone();
            let block_height_clone = block.height;
            let block_previous_hash_clone = block.previous_hash.clone();
            let validator_clone = validator_id.to_string();
            let timestamp_clone = block.timestamp;
            tokio::spawn(async move {
                let msg = PbftPrePrepareMsg {
                    round,
                    view,
                    block_height: block_height_clone,
                    block_hash: block_hash_clone,
                    previous_hash: block_previous_hash_clone,
                    validator_id: validator_clone,
                    timestamp: timestamp_clone,
                };
                let _ = p2p_clone.broadcast_pbft_pre_prepare(msg).await;
            });

            // ============ PBFT: Prepare ============
            pbft.prepare(&block.hash, validator_id);

            // ============ PBFT: Commit ============
            let committed = pbft.commit(&block.hash, validator_id);

            if committed || pbft.is_committed(&block.hash) {
                tracing::info!(
                    "✅ PBFT consensus reached for block height={}",
                    block.height
                );
                pbft.start_new_round();
            }

            // Add block to local blockchain
            match state.consensus.blockchain.add_block(block.clone()) {
                Ok(_) => {
                    tracing::info!("🔍 STEP 1 - minting rewards");
                    for tx in &block.transactions {
                        if tx.from == "network" {
                            state.token_engine.mint(&tx.to, tx.amount);
                        }
                    }

                    tracing::info!("🔍 STEP 2 - saving consensus to disk");
                    let _ = state.consensus.save_to_disk();

                    tracing::info!("🔍 STEP 3 - saving token state");
                    let _ = state.token_engine.save_to_disk("./chain_data");

                    tracing::info!("🔍 STEP 4 - cleaning mempool");
                    {
                        let mut mempool = state.mempool.inner.write().unwrap();
                        mempool.remove_batch(&tx_ids);
                        let _ = mempool.save_to_disk("./chain_data");
                    }

                    tracing::info!("🔍 STEP 5 - remove pending txs");
                    state
                        .consensus
                        .blockchain
                        .remove_pending_transactions(&tx_ids);

                    tracing::info!("🔍 STEP 6 - get balance");
                    let miner_balance = state.token_engine.get_balance(validator_id);

                    tracing::info!("🔍 STEP 7 - get round info");
                    let round_info = pbft.get_round_info();

                    tracing::info!("✅ Block mined: height={}, hash={}, reward=100, round={}, view={}, leader={}", 
                        block.height, &block.hash[..16], round_info.round, round_info.view, round_info.leader);

                    // Fire-and-forget P2P block broadcast
                    tracing::info!("🔍 STEP 8 - spawning P2P broadcast");
                    let p2p = state.p2p_handle.clone();
                    let h = block.height;
                    let hash = block.hash.clone();
                    let prev = block.previous_hash.clone();
                    let val = validator_id.to_string();
                    let tx_count = block.transactions.len() as u64;
                    let ts = block.timestamp;
                    tokio::spawn(async move {
                        tracing::info!("🔍 TRACE: About to broadcast block height={}", h);
                    match p2p.broadcast_block(h, hash.clone(), prev.clone(), val.clone(), tx_count, ts).await {
                        Ok(()) => tracing::info!("📤 TRACE: Block broadcast SUCCESS height={}", h),
                        Err(e) => tracing::error!("❌ TRACE: Block broadcast FAILED height={}: {}", h, e),
                    }
                    });

                    // WebSocket broadcast - commented out for diagnosis
                    // let ws_msg = serde_json::json!({"type":"new_block","height":block.height,"hash":&block.hash[..16]}).to_string();
                    // let ws_server = state.ws_server.clone();
                    // tokio::spawn(async move { let _ = ws_server.broadcast(&ws_msg).await; });

                    tracing::info!("🔍 STEP 9 - returning Json response");

                    Json(serde_json::json!({
                        "status": "block_mined",
                        "block_height": block.height,
                        "block_hash": block.hash,
                        "trace_length": trace.len(),
                        "proof_hash": hex::encode(&stark_proof.trace_hash[..8]),
                        "reward": 100u64,
                        "miner_balance": miner_balance,
                        "transaction_count": block.transactions.len(),
                        "pbft_round": round_info.round,
                        "pbft_view": round_info.view,
                        "pbft_leader": round_info.leader
                    }))
                }
                Err(e) => {
                    tracing::error!("❌ Failed to add block: {}", e);
                    Json(serde_json::json!({ "status": "consensus_failed", "error": e }))
                }
            }
        }
        Err(e) => {
            tracing::error!("❌ Proof generation failed: {}", e);
            Json(serde_json::json!({ "status": "proof_failed", "error": e }))
        }
    }
}

pub async fn sync_chain_handler(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let blocks = state.consensus.get_all_blocks();
    Json(
        serde_json::json!({"blocks": blocks.iter().map(|b| serde_json::json!({
        "height": b.height, "hash": b.hash, "previous_hash": b.previous_hash, "validator_id": b.validator_id
    })).collect::<Vec<_>>()}),
    )
}

pub async fn get_chain_handler(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let round_info = state.consensus.pbft.get_round_info();
    Json(serde_json::json!({
        "height": state.consensus.get_blockchain_height(),
        "last_block_hash": state.consensus.get_last_block().hash,
        "validator_count": state.consensus.get_validator_set().len(),
        "pbft_round": round_info.round,
        "pbft_view": round_info.view,
        "pbft_leader": round_info.leader
    }))
}

pub async fn get_block_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(p): axum::extract::Query<HashMap<String, String>>,
) -> Json<serde_json::Value> {
    let h: u64 = p.get("height").and_then(|v| v.parse().ok()).unwrap_or(0);
    match state.consensus.get_block_by_height(h) {
        Some(block) => Json(
            serde_json::json!({ "found": true, "block": { "height": block.height, "hash": block.hash, "previous_hash": block.previous_hash, "transaction_count": block.transactions.len(), "has_compute_proof": block.compute_proof.is_some() }}),
        ),
        None => Json(serde_json::json!({ "found": false })),
    }
}

pub async fn list_blocks_handler(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let blocks = state.consensus.get_all_blocks();
    let summaries: Vec<serde_json::Value> = blocks.iter().map(|b| serde_json::json!({ "height": b.height, "hash": &b.hash[..16.min(b.hash.len())], "previous_hash": &b.previous_hash[..16.min(b.previous_hash.len())], "validator_id": b.validator_id, "has_proof": b.compute_proof.is_some() })).collect();
    Json(serde_json::json!({ "count": summaries.len(), "blocks": summaries }))
}

// Strategy: Insert Mediator for api (Interface)
// Review and adjust before applying.
