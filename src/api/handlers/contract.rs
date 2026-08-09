use crate::api::handlers::AppState;
use crate::vm::instruction::Instruction;
use axum::{extract::State, Json};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, serde::Deserialize)]
pub struct DeployRequest {
    pub owner: String,
    pub program: Vec<crate::api::models::InstructionData>,
}
#[derive(Debug, serde::Deserialize)]
pub struct CallRequest {
    pub contract_id: String,
    pub args: Vec<u64>,
}
#[derive(Debug, serde::Deserialize)]
pub struct CallWithGasRequest {
    pub contract_id: String,
    pub args: Vec<u64>,
    pub gas_limit: u64,
}

pub async fn deploy_contract_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<DeployRequest>,
) -> Json<serde_json::Value> {
    let mut instructions = Vec::new();
    for inst_data in &request.program {
        let instruction = match inst_data.opcode.as_str() {
            "MOV" => Instruction::Mov {
                register: inst_data.params[0] as usize,
                value: inst_data.params.get(1).copied().unwrap_or(0),
            },
            "ADD" => Instruction::Add {
                destination: inst_data.params[0] as usize,
                source: inst_data.params.get(1).copied().unwrap_or(0) as usize,
            },
            "HALT" => Instruction::Halt,
            _ => Instruction::Halt,
        };
        instructions.push(instruction);
    }
    let contract_id = state.contract_storage.deploy(&request.owner, instructions);
    Json(
        serde_json::json!({ "status": "deployed", "contract_id": contract_id, "owner": request.owner, "code_length": request.program.len() }),
    )
}

pub async fn call_contract_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CallRequest>,
) -> Json<serde_json::Value> {
    match state
        .contract_storage
        .call(&request.contract_id, &request.args, 100000)
    {
        Ok(result) => Json(
            serde_json::json!({ "status": "success", "contract_id": request.contract_id, "result": result.registers, "gas_used": result.gas_used }),
        ),
        Err(e) => Json(serde_json::json!({ "status": "error", "message": e })),
    }
}

pub async fn get_contract_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(p): axum::extract::Query<HashMap<String, String>>,
) -> Json<serde_json::Value> {
    let id = p.get("id").cloned().unwrap_or_default();
    if id.is_empty() {
        return Json(serde_json::json!({"error": "Missing 'id' parameter"}));
    }
    match state.contract_storage.get(&id) {
        Some(contract) => Json(
            serde_json::json!({ "found": true, "contract": { "id": contract.id, "owner": contract.owner, "balance": contract.balance, "call_count": contract.call_count, "code_length": contract.code.len(), "storage": contract.storage }}),
        ),
        None => Json(serde_json::json!({ "found": false })),
    }
}

pub async fn call_with_gas_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CallWithGasRequest>,
) -> Json<serde_json::Value> {
    match state
        .contract_storage
        .call(&request.contract_id, &request.args, request.gas_limit)
    {
        Ok(result) => Json(
            serde_json::json!({ "status": "success", "result": result.registers, "gas_used": result.gas_used }),
        ),
        Err(e) => Json(serde_json::json!({ "status": "error", "message": e })),
    }
}

pub async fn estimate_gas_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let program_data = request["program"].as_array();
    let mut instructions = Vec::new();
    if let Some(prog) = program_data {
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
                "SHA256" => Instruction::Sha256 {
                    source: params.get(0).copied().unwrap_or(0) as usize,
                    destination: params.get(1).copied().unwrap_or(0) as usize,
                },
                "HALT" => Instruction::Halt,
                _ => Instruction::Halt,
            };
            instructions.push(instruction);
        }
    }
    let gas_estimate = state.contract_storage.estimate_gas(&instructions);
    let fee_estimate = state.contract_storage.gas_to_fee(gas_estimate);
    Json(
        serde_json::json!({ "gas_estimate": gas_estimate, "fee_estimate": fee_estimate, "instruction_count": instructions.len() }),
    )
}

pub async fn gas_stats_handler(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let stats = state.contract_storage.gas_stats();
    Json(
        serde_json::json!({ "total_gas_used": stats.total_gas_used, "total_fees_collected": stats.total_fees_collected, "contract_count": stats.contract_count }),
    )
}

// Strategy: Insert Mediator for api (Interface)
// Review and adjust before applying.
