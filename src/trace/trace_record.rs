use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceRecord {
    pub step: usize,
    pub pc: usize,
    pub instruction: crate::vm::instruction::Instruction,
    pub registers_before: Vec<u64>,
    pub registers_after: Vec<u64>,
}
impl TraceRecord {
    pub fn new(
        step: usize,
        pc: usize,
        instruction: crate::vm::instruction::Instruction,
        registers_before: Vec<u64>,
        registers_after: Vec<u64>,
    ) -> Self {
        TraceRecord {
            step,
            pc,
            instruction,
            registers_before,
            registers_after,
        }
    }
    pub fn hash(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(
            format!(
                "{}{:?}{:?}",
                self.pc, self.registers_before, self.registers_after
            )
            .as_bytes(),
        );
        hex::encode(h.finalize())
    }
}
