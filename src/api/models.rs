use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeRequest {
    pub program: Vec<InstructionData>,
    pub input_registers: Option<Vec<u64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveComputeRequest {
    pub difficulty: u32,
    pub workload_type: Option<String>,
    pub input_registers: Option<Vec<u64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeResponse {
    pub success: bool,
    pub final_registers: Vec<u64>,
    pub trace_hash: String,
    pub proof_valid: bool,
    pub execution_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstructionData {
    pub opcode: String,
    pub params: Vec<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyRequest {
    pub trace_hash: String,
    pub proof: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResponse {
    pub valid: bool,
    pub message: String,
}

// Marketplace models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrderRequest {
    pub order_type: String, // "buy" or "sell"
    pub miner_id: String,
    pub compute_units: u64,
    pub price_per_unit: u64,
    pub difficulty_level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResponse {
    pub id: String,
    pub order_type: String,
    pub miner_id: String,
    pub compute_units: u64,
    pub price_per_unit: u64,
    pub difficulty_level: u32,
    pub status: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketStatsResponse {
    pub total_orders: u64,
    pub open_orders: u64,
    pub total_compute_units: u64,
    pub avg_price: f64,
    pub total_volume: u64,
}

// Strategy: Insert Mediator for api (Interface)
// Review and adjust before applying.
