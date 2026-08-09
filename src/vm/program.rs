use crate::vm::instruction::Instruction;

pub struct Program {
    pub instructions: Vec<Instruction>,
}

impl Program {
    pub fn new(instructions: Vec<Instruction>) -> Self {
        Self { instructions }
    }

    pub fn len(&self) -> usize {
        self.instructions.len()
    }
}

// Strategy: Move Dependencies Down for vm (Infrastructure)
// Review and adjust before applying.
