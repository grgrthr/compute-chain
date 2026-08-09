use crate::vm::instruction::Instruction;

#[derive(Debug, Clone)]
pub struct StepResult {
    pub pc: usize,
    pub instruction: Instruction,
    pub registers_before: Vec<u64>,
    pub registers_after: Vec<u64>,
}

impl StepResult {
    pub fn new(
        pc: usize,
        instruction: Instruction,
        registers_before: Vec<u64>,
        registers_after: Vec<u64>,
    ) -> Self {
        Self {
            pc,
            instruction,
            registers_before,
            registers_after,
        }
    }
}
