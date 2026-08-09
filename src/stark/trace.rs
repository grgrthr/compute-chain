use crate::trace::trace_record::TraceRecord;
use crate::vm::instruction::Instruction;

#[derive(Debug, Clone)]
pub struct ExecutionTrace {
    pub steps: Vec<TraceStep>,
}

#[derive(Debug, Clone)]
pub struct TraceStep {
    pub pc: usize,
    pub opcode: u64,
    pub reg_before: u64,
    pub reg_after: u64,
    pub mem_hash_before: Vec<u8>,
    pub mem_hash_after: Vec<u8>,
}

impl ExecutionTrace {
    pub fn new(steps: Vec<TraceStep>) -> Self {
        Self { steps }
    }
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn to_values(&self) -> Vec<u64> {
        let mut values = Vec::new();
        for step in &self.steps {
            values.push(step.pc as u64);
            values.push(step.opcode);
            values.push(step.reg_before);
            values.push(step.reg_after);
            for chunk in step.mem_hash_before.chunks(8) {
                let mut arr = [0u8; 8];
                for (i, &b) in chunk.iter().enumerate() {
                    arr[i] = b;
                }
                values.push(u64::from_le_bytes(arr));
            }
            for chunk in step.mem_hash_after.chunks(8) {
                let mut arr = [0u8; 8];
                for (i, &b) in chunk.iter().enumerate() {
                    arr[i] = b;
                }
                values.push(u64::from_le_bytes(arr));
            }
        }
        values
    }
}

pub fn build_trace_from_records(
    records: &[TraceRecord],
    memory_states: &[(Vec<u8>, Vec<u8>)],
) -> ExecutionTrace {
    let steps: Vec<TraceStep> = records
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let opcode = match &r.instruction {
                Instruction::Mov { .. } => 0x01,
                Instruction::Add { .. } => 0x02,
                Instruction::Sub { .. } => 0x03,
                Instruction::Mul { .. } => 0x04,
                Instruction::Div { .. } => 0x05,
                Instruction::Cmp { .. } => 0x30,
                Instruction::Jump { .. } => 0x20,
                Instruction::Load { .. } => 0x10,
                Instruction::Store { .. } => 0x11,
                Instruction::Halt => 0xFF,
                Instruction::Call { .. } => 0x31,
                Instruction::Ret => 0x32,
                Instruction::Push { .. } => 0x50,
                Instruction::Pop { .. } => 0x51,
                Instruction::CallData { .. } => 0x33,
                Instruction::Log { .. } => 0x34,
                Instruction::SelfBalance { .. } => 0x35,
                Instruction::Sha256 { .. } => 0x60,
            };
            let (mem_before, mem_after) = memory_states
                .get(i)
                .cloned()
                .unwrap_or_else(|| (vec![], vec![]));
            TraceStep {
                pc: r.pc,
                opcode,
                reg_before: r.registers_before.first().copied().unwrap_or(0),
                reg_after: r.registers_after.first().copied().unwrap_or(0),
                mem_hash_before: mem_before,
                mem_hash_after: mem_after,
            }
        })
        .collect();
    ExecutionTrace::new(steps)
}

// Strategy: Move Dependencies Down for stark (Infrastructure)
// Review and adjust before applying.
