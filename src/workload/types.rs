use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workload {
    pub id: String,
    pub workload_type: WorkloadType,
    pub instructions: Vec<WorkloadInstruction>,
    pub difficulty: Difficulty,
    pub memory_required: usize,
    pub compute_operations: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkloadType {
    ComputeHeavy, // عمليات حسابية كثيرة
    MemoryHeavy,  // عمليات ذاكرة كثيرة
    Mixed,        // خليط
    Random,       // عشوائي بالكامل
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadInstruction {
    pub opcode: String,
    pub params: Vec<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Difficulty {
    pub level: u32,          // 1-10
    pub target_time_ms: u64, // الوقت المستهدف
    pub complexity: f64,     // معامل التعقيد
}

impl Difficulty {
    pub fn new(level: u32) -> Self {
        Self {
            level: level.min(10).max(1),
            target_time_ms: 100 * level as u64,
            complexity: 1.0 + (level as f64 * 0.5),
        }
    }
}
