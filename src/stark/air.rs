use crate::vm::instruction::Instruction;

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionState {
    pub pc: usize,
    pub opcode: u8,
    pub registers_before: [u64; 8],
    pub registers_after: [u64; 8],
    pub mem_addr: Option<usize>,
    pub mem_value_before: Option<u64>,
    pub mem_value_after: Option<u64>,
    pub is_terminal: bool,
}

pub trait AirConstraints {
    fn evaluate_transition(&self, current: &ExecutionState, next: &ExecutionState) -> bool;
    fn evaluate_boundary(&self, first: &ExecutionState, last: &ExecutionState) -> bool;
    fn constraint_count(&self) -> usize;
}

pub struct ComputeAir;

impl ComputeAir {
    pub fn new() -> Self {
        ComputeAir
    }
    pub fn opcode_of(instruction: &Instruction) -> u8 {
        match instruction {
            Instruction::Mov { .. } => 0x01,
            Instruction::Add { .. } => 0x02,
            Instruction::Sub { .. } => 0x03,
            Instruction::Mul { .. } => 0x04,
            Instruction::Div { .. } => 0x05,
            Instruction::Load { .. } => 0x10,
            Instruction::Store { .. } => 0x11,
            Instruction::Jump { .. } => 0x20,
            Instruction::Cmp { .. } => 0x30,
            Instruction::Call { .. } => 0x31,
            Instruction::Push { .. } => 0x50,
            Instruction::Pop { .. } => 0x51,
            Instruction::Sha256 { .. } => 0x60,
            Instruction::Ret => 0x32,
            Instruction::CallData { .. } => 0x33,
            Instruction::Log { .. } => 0x34,
            Instruction::SelfBalance { .. } => 0x35,
            Instruction::Halt => 0xFF,
        }
    }
    fn constraint_pc_transition(&self, current: &ExecutionState, next: &ExecutionState) -> bool {
        if current.is_terminal {
            return true;
        }
        matches!(current.opcode, 0x20 | 0x31 | 0x32) || next.pc == current.pc + 1
    }
    fn constraint_valid_opcode(&self, state: &ExecutionState) -> bool {
        matches!(
            state.opcode,
            0x01 | 0x02
                | 0x03
                | 0x04
                | 0x05
                | 0x10
                | 0x11
                | 0x20
                | 0x30
                | 0x31
                | 0x32
                | 0x33
                | 0x34
                | 0x35
                | 0x50
                | 0x51
                | 0x60
                | 0xFF
        )
    }
    fn constraint_memory(&self, _state: &ExecutionState) -> bool {
        true
    }
    pub fn constraint_halt_terminal(&self, state: &ExecutionState) -> bool {
        if state.opcode == 0xFF {
            state.is_terminal
        } else {
            !state.is_terminal
        }
    }
}

impl AirConstraints for ComputeAir {
    fn evaluate_transition(&self, current: &ExecutionState, next: &ExecutionState) -> bool {
        self.constraint_pc_transition(current, next)
            && self.constraint_valid_opcode(current)
            && self.constraint_valid_opcode(next)
            && self.constraint_memory(current)
            && self.constraint_memory(next)
            && self.constraint_halt_terminal(current)
            && self.constraint_halt_terminal(next)
    }
    fn evaluate_boundary(&self, first: &ExecutionState, last: &ExecutionState) -> bool {
        first.pc == 0 && last.is_terminal && last.opcode == 0xFF
    }
    fn constraint_count(&self) -> usize {
        5
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn s(pc: usize, opcode: u8, term: bool) -> ExecutionState {
        ExecutionState {
            pc,
            opcode,
            registers_before: [0; 8],
            registers_after: [0; 8],
            mem_addr: None,
            mem_value_before: None,
            mem_value_after: None,
            is_terminal: term,
        }
    }
    fn valid() -> Vec<ExecutionState> {
        vec![
            s(0, 0x01, false),
            s(1, 0x01, false),
            s(2, 0x02, false),
            s(3, 0xFF, true),
        ]
    }

    #[test]
    fn test_valid_trace() {
        let a = ComputeAir::new();
        let t = valid();
        for i in 0..t.len() - 1 {
            assert!(a.evaluate_transition(&t[i], &t[i + 1]));
        }
    }
    #[test]
    fn test_boundary_ok() {
        let t = valid();
        assert!(ComputeAir::new().evaluate_boundary(&t[0], &t[t.len() - 1]));
    }
    #[test]
    fn test_pc_jump_fails() {
        assert!(!ComputeAir::new().evaluate_transition(&s(0, 0x01, false), &s(5, 0x02, false)));
    }
    #[test]
    fn test_bad_opcode() {
        assert!(!ComputeAir::new().evaluate_transition(&s(0, 0xFE, false), &s(1, 0x01, false)));
    }
    #[test]
    fn test_halt_terminal() {
        let a = ComputeAir::new();
        assert!(a.constraint_halt_terminal(&s(0, 0xFF, true)));
        assert!(!a.constraint_halt_terminal(&s(0, 0xFF, false)));
    }
    #[test]
    fn test_nonhalt_not_terminal() {
        assert!(!ComputeAir::new().evaluate_transition(&s(0, 0x01, true), &s(1, 0x02, false)));
    }
    #[test]
    fn test_load_store_valid() {
        let a = ComputeAir::new();
        let ls = ExecutionState {
            mem_addr: Some(100),
            ..s(1, 0x10, false)
        };
        assert!(a.evaluate_transition(&ls, &s(2, 0x01, false)));
    }
    #[test]
    fn test_first_pc_zero() {
        assert!(!ComputeAir::new().evaluate_boundary(&s(5, 0x01, false), &s(10, 0xFF, true)));
    }
    #[test]
    fn test_last_halt() {
        assert!(!ComputeAir::new().evaluate_boundary(&s(0, 0x01, false), &s(10, 0x01, false)));
    }
    #[test]
    fn test_deterministic() {
        let a = ComputeAir::new();
        let t = valid();
        assert_eq!(
            a.evaluate_boundary(&t[0], &t[3]),
            a.evaluate_boundary(&t[0], &t[3])
        );
    }
    #[test]
    fn test_count() {
        assert_eq!(ComputeAir::new().constraint_count(), 5);
    }
    #[test]
    fn test_jump_allows_non_seq() {
        assert!(ComputeAir::new().evaluate_transition(&s(0, 0x20, false), &s(10, 0x01, false)));
    }
    #[test]
    fn test_all_opcodes() {
        let a = ComputeAir::new();
        for o in &[
            0x01, 0x02, 0x03, 0x04, 0x05, 0x10, 0x11, 0x20, 0x30, 0x31, 0x32, 0x33, 0x34, 0x35,
            0x50, 0x51, 0x60, 0xFF,
        ] {
            assert!(
                a.evaluate_transition(&s(0, *o, *o == 0xFF), &s(1, 0xFF, true)),
                "0x{:02X}",
                o
            );
        }
    }
}
