use crate::api::handlers::AppState;
use crate::trace::tracer::Tracer;
use crate::vm::cpu::Cpu;
use crate::vm::executor::Executor;
use crate::vm::instruction::Instruction;
use crate::vm::memory::Memory;
use crate::vm::program::Program;
use axum::{extract::State, Json};
use std::sync::Arc;

#[derive(Debug, serde::Deserialize)]
pub struct StarkProveRequest {
    pub program: Vec<crate::api::models::InstructionData>,
    pub input_registers: Option<Vec<u64>>,
}

#[derive(Debug, serde::Serialize)]
pub struct StarkProveResponse {
    pub success: bool,
    pub proof_size_bytes: usize,
    pub trace_length: usize,
    pub message: String,
    pub proof_hash: String,
    pub proof_data: Option<serde_json::Value>,
}

pub async fn stark_prove_handler(
    State(_state): State<Arc<AppState>>,
    Json(request): Json<StarkProveRequest>,
) -> Json<StarkProveResponse> {
    use crate::stark::prover::ComputeProver;
    use crate::stark::trace::build_trace_from_records;
    let mut instructions = Vec::new();
    for inst_data in &request.program {
        let instruction = match inst_data.opcode.as_str() {
            "MOV" => Instruction::Mov {
                register: inst_data.params[0] as usize,
                value: inst_data.params[1],
            },
            "ADD" => Instruction::Add {
                destination: inst_data.params[0] as usize,
                source: inst_data.params[1] as usize,
            },
            "SUB" => Instruction::Sub {
                destination: inst_data.params[0] as usize,
                source: inst_data.params[1] as usize,
            },
            "MUL" => Instruction::Mul {
                destination: inst_data.params[0] as usize,
                source: inst_data.params[1] as usize,
            },
            "DIV" => Instruction::Div {
                destination: inst_data.params[0] as usize,
                source: inst_data.params[1] as usize,
            },
            "CMP" => {
                if inst_data.params.len() >= 3 {
                    Instruction::Cmp {
                        left: inst_data.params[0] as usize,
                        right: inst_data.params[1] as usize,
                        target_pc: inst_data.params[2] as usize,
                    }
                } else {
                    Instruction::Halt
                }
            }
            "JUMP" | "JMP" => {
                if inst_data.params.len() >= 1 {
                    Instruction::Jump {
                        target_pc: inst_data.params[0] as usize,
                    }
                } else {
                    Instruction::Halt
                }
            }
            "HALT" => Instruction::Halt,
            _ => Instruction::Halt,
        };
        instructions.push(instruction);
    }
    let mut cpu = Cpu::new();
    let mut memory = Memory::new(65536);
    let mut tracer = Tracer::new();
    if let Some(regs) = &request.input_registers {
        for i in 0..regs.len().min(8) {
            cpu.registers[i] = regs[i];
        }
    }
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
    let memory_states: Vec<(Vec<u8>, Vec<u8>)> = {
        let mem_tracker = Memory::new(65536);
        vec![(mem_tracker.hash_state(), mem_tracker.hash_state()); tracer.records.len()]
    };
    let trace = build_trace_from_records(&tracer.records, &memory_states);
    let prover = ComputeProver::new();
    match prover.prove(&trace) {
        Ok(proof) => {
            let proof_hash = hex::encode(
                &proof
                    .trace_hash
                    .iter()
                    .take(8)
                    .copied()
                    .collect::<Vec<u8>>(),
            );
            let proof_json = serde_json::to_value(&proof).ok();
            Json(StarkProveResponse {
                success: true,
                proof_size_bytes: format!("{:?}", proof).len(),
                trace_length: trace.len(),
                proof_hash: proof_hash.clone(),
                proof_data: proof_json,
                message: format!("STARK proof: {} steps, hash={}", trace.len(), proof_hash),
            })
        }
        Err(e) => Json(StarkProveResponse {
            success: false,
            proof_size_bytes: 0,
            trace_length: trace.len(),
            proof_hash: String::new(),
            proof_data: None,
            message: format!("Proof failed: {}", e),
        }),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct StarkVerifyRequest {
    pub program: Vec<crate::api::models::InstructionData>,
    pub input_registers: Option<Vec<u64>>,
    pub expected_result: Option<Vec<u64>>,
}

pub async fn stark_verify_handler(
    State(_state): State<Arc<AppState>>,
    Json(request): Json<StarkVerifyRequest>,
) -> Json<serde_json::Value> {
    use crate::stark::prover::ComputeProver;
    use crate::stark::trace::build_trace_from_records;
    use crate::stark::verifier::ComputeVerifier;
    let mut instructions = Vec::new();
    for inst_data in &request.program {
        let instruction = match inst_data.opcode.as_str() {
            "MOV" => Instruction::Mov {
                register: inst_data.params[0] as usize,
                value: inst_data.params[1],
            },
            "ADD" => Instruction::Add {
                destination: inst_data.params[0] as usize,
                source: inst_data.params[1] as usize,
            },
            "SUB" => Instruction::Sub {
                destination: inst_data.params[0] as usize,
                source: inst_data.params[1] as usize,
            },
            "MUL" => Instruction::Mul {
                destination: inst_data.params[0] as usize,
                source: inst_data.params[1] as usize,
            },
            "DIV" => Instruction::Div {
                destination: inst_data.params[0] as usize,
                source: inst_data.params[1] as usize,
            },
            "CMP" => {
                if inst_data.params.len() >= 3 {
                    Instruction::Cmp {
                        left: inst_data.params[0] as usize,
                        right: inst_data.params[1] as usize,
                        target_pc: inst_data.params[2] as usize,
                    }
                } else {
                    Instruction::Halt
                }
            }
            "JUMP" | "JMP" => {
                if inst_data.params.len() >= 1 {
                    Instruction::Jump {
                        target_pc: inst_data.params[0] as usize,
                    }
                } else {
                    Instruction::Halt
                }
            }
            "HALT" => Instruction::Halt,
            _ => Instruction::Halt,
        };
        instructions.push(instruction);
    }
    let mut cpu = Cpu::new();
    let mut memory = Memory::new(65536);
    let mut tracer = Tracer::new();
    if let Some(regs) = &request.input_registers {
        for i in 0..regs.len().min(8) {
            cpu.registers[i] = regs[i];
        }
    }
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
    let memory_states: Vec<(Vec<u8>, Vec<u8>)> = {
        let mem_tracker = Memory::new(65536);
        vec![(mem_tracker.hash_state(), mem_tracker.hash_state()); tracer.records.len()]
    };
    let trace = build_trace_from_records(&tracer.records, &memory_states);
    let prover = ComputeProver::new();
    let _verifier = ComputeVerifier::new();
    match prover.prove(&trace) {
        Ok(proof) => {
            let is_valid = !proof.trace_hash.is_empty() && proof.trace_length > 0;
            let proof_hash = hex::encode(
                &proof
                    .trace_hash
                    .iter()
                    .take(8)
                    .copied()
                    .collect::<Vec<u8>>(),
            );
            let result_match = match &request.expected_result {
                Some(expected) => {
                    let len = expected.len().min(8);
                    cpu.registers[..len] == expected[..len]
                }
                None => true,
            };
            Json(
                serde_json::json!({ "verified": is_valid, "result_match": result_match, "final_registers": cpu.registers.to_vec(), "trace_length": trace.len(), "proof_hash": proof_hash, "status": if is_valid && result_match { "PASS" } else if is_valid && !result_match { "WARN" } else { "FAIL" } }),
            )
        }
        Err(e) => Json(serde_json::json!({ "verified": false, "error": e })),
    }
}

// Strategy: Insert Mediator for api (Interface)
// Review and adjust before applying.
