use crate::api::handlers::AppState;
use crate::api::models::*;
use crate::merkle::hash::TraceHasher;
use crate::merkle::tree::MerkleTree;
use crate::stark::simple_stark::SimpleStark;
use crate::trace::serializer::TraceSerializer;
use crate::trace::tracer::Tracer;
use crate::vm::cpu::Cpu;
use crate::vm::executor::Executor;
use crate::vm::instruction::Instruction;
use crate::vm::memory::Memory;
use crate::vm::program::Program;
use crate::workload::generator::WorkloadGenerator;
use crate::workload::types::WorkloadType;
use axum::{extract::State, Json};
use std::sync::Arc;
use std::time::Instant;

pub async fn compute_handler(
    State(_state): State<Arc<AppState>>,
    Json(request): Json<ComputeRequest>,
) -> Json<ComputeResponse> {
    let start = Instant::now();
    let mut instructions = Vec::new();
    for inst_data in request.program {
        let instruction = match inst_data.opcode.as_str() {
            "MOV" => Instruction::Mov { register: inst_data.params[0] as usize, value: inst_data.params[1] },
            "ADD" => Instruction::Add { destination: inst_data.params[0] as usize, source: inst_data.params[1] as usize },
            "MUL" => Instruction::Mul { destination: inst_data.params[0] as usize, source: inst_data.params[1] as usize },
            "HALT" => Instruction::Halt,
            _ => Instruction::Halt,
        };
        instructions.push(instruction);
    }
    let mut cpu = Cpu::new();
    let mut memory = Memory::new(65536);
    let mut tracer = Tracer::new();
    if let Some(regs) = request.input_registers {
        for i in 0..regs.len().min(8) { cpu.registers[i] = regs[i]; }
    }
    let program = Program::new(instructions);
    let mut step_counter = 0;
    let mut stark_trace_values = Vec::new();
    while !cpu.halted {
        let result = Executor::step(&mut cpu, &mut memory, &program);
        if let Some(step) = result {
            tracer.record(step_counter, step.pc, step.instruction.clone(), step.registers_before.to_vec(), step.registers_after.to_vec());
            if let Some(first_reg) = step.registers_after.first() { stark_trace_values.push(*first_reg); }
            step_counter += 1;
        } else { break; }
    }
    let leaf_hashes: Vec<String> = tracer.records.iter().map(|r| TraceSerializer::serialize_record(r)).map(|s| TraceHasher::hash_string(&s)).collect();
    let tree = MerkleTree::new(leaf_hashes);
    let trace_hash = tree.root_hash.clone();
    let mut proof_valid = false;
    if !stark_trace_values.is_empty() {
        let stark_proof = SimpleStark::prove(&stark_trace_values);
        proof_valid = SimpleStark::verify(&stark_proof, &stark_proof.trace_hash);
    }
    Json(ComputeResponse { success: true, final_registers: cpu.registers.to_vec(), trace_hash, proof_valid, execution_time_ms: start.elapsed().as_millis() as u64 })
}

pub async fn adaptive_compute_handler(State(_state): State<Arc<AppState>>, Json(request): Json<AdaptiveComputeRequest>) -> Json<ComputeResponse> {
    let start = Instant::now();
    let workload_type = match request.workload_type.as_deref() { Some("compute") => WorkloadType::ComputeHeavy, Some("memory") => WorkloadType::MemoryHeavy, _ => WorkloadType::Mixed };
    let workload = WorkloadGenerator::generate_with_type(request.difficulty.clamp(1,10), workload_type);
    let mut instructions = Vec::new();
    for inst_data in workload.instructions {
        let instruction = match inst_data.opcode.as_str() {
            "MOV" => Instruction::Mov { register: inst_data.params[0] as usize, value: inst_data.params[1] },
            "ADD" => Instruction::Add { destination: inst_data.params[0] as usize, source: inst_data.params[1] as usize },
            "MUL" => Instruction::Mul { destination: inst_data.params[0] as usize, source: inst_data.params[1] as usize },
            "HALT" => Instruction::Halt,
            _ => Instruction::Halt,
        };
        instructions.push(instruction);
    }
    let mut cpu = Cpu::new(); let mut memory = Memory::new(workload.memory_required); let mut tracer = Tracer::new();
    if let Some(regs) = request.input_registers { for i in 0..regs.len().min(8) { cpu.registers[i] = regs[i]; } }
    let program = Program::new(instructions); let mut step_counter = 0; let mut stark_trace_values = Vec::new();
    while !cpu.halted {
        let result = Executor::step(&mut cpu, &mut memory, &program);
        if let Some(step) = result {
            tracer.record(step_counter, step.pc, step.instruction.clone(), step.registers_before.to_vec(), step.registers_after.to_vec());
            if let Some(first_reg) = step.registers_after.first() { stark_trace_values.push(*first_reg); }
            step_counter += 1;
        } else { break; }
    }
    let leaf_hashes: Vec<String> = tracer.records.iter().map(|r| TraceSerializer::serialize_record(r)).map(|s| TraceHasher::hash_string(&s)).collect();
    let tree = MerkleTree::new(leaf_hashes); let trace_hash = tree.root_hash.clone();
    let mut proof_valid = false;
    if !stark_trace_values.is_empty() { let stark_proof = SimpleStark::prove(&stark_trace_values); proof_valid = SimpleStark::verify(&stark_proof, &stark_proof.trace_hash); }
    Json(ComputeResponse { success: true, final_registers: cpu.registers.to_vec(), trace_hash, proof_valid, execution_time_ms: start.elapsed().as_millis() as u64 })
}

// ═══ PIPELINE: Execute VM → Generate Proof → Mine Block → Broadcast ═══
pub async fn pipeline_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ComputeRequest>,
) -> Json<serde_json::Value> {
    let start = Instant::now();
    let mut instructions = Vec::new();
    for inst_data in &request.program {
        let instruction = match inst_data.opcode.as_str() {
            "MOV" => Instruction::Mov { register: inst_data.params[0] as usize, value: inst_data.params[1] },
            "ADD" => Instruction::Add { destination: inst_data.params[0] as usize, source: inst_data.params[1] as usize },
            "MUL" => Instruction::Mul { destination: inst_data.params[0] as usize, source: inst_data.params[1] as usize },
            "HALT" => Instruction::Halt,
            _ => Instruction::Halt,
        };
        instructions.push(instruction);
    }
    let mut cpu = Cpu::new(); let mut memory = Memory::new(65536); let mut tracer = Tracer::new();
    let program = Program::new(instructions); let mut step_counter = 0;
    while !cpu.halted {
        let result = Executor::step(&mut cpu, &mut memory, &program);
        if let Some(step) = result {
            tracer.record(step_counter, step.pc, step.instruction.clone(), step.registers_before.to_vec(), step.registers_after.to_vec());
            step_counter += 1;
        } else { break; }
    }
    let last_block = state.consensus.get_last_block();
    let coinbase = crate::consensus::types::Transaction::new("network".to_string(), "node1".to_string(), 100, 0);
    let mut block = crate::consensus::types::Block::new(last_block.height + 1, last_block.hash.clone(), "node1".to_string(), vec![coinbase]);
    block.hash = format!("block_{}_{}", block.height, TraceHasher::hash_string(&format!("{:?}", cpu.registers))[..16].to_string());
    let _ = state.consensus.blockchain.add_block(block.clone());
    let p2p = state.p2p_handle.clone(); let h = block.height; let hash = block.hash.clone(); let prev = block.previous_hash.clone();
    tokio::spawn(async move { let _ = p2p.broadcast_block(h, hash, prev, "node1".to_string(), 1, block.timestamp).await; });
    Json(serde_json::json!({"status":"pipeline_complete","block_height":block.height,"block_hash":block.hash,"final_registers":cpu.registers.to_vec(),"trace_length":tracer.records.len(),"execution_time_ms":start.elapsed().as_millis() as u64}))
}
