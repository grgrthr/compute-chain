use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Instruction {
    // أساسية
    Mov {
        register: usize,
        value: u64,
    },
    Add {
        destination: usize,
        source: usize,
    },
    Sub {
        destination: usize,
        source: usize,
    },
    Mul {
        destination: usize,
        source: usize,
    },
    Div {
        destination: usize,
        source: usize,
    },
    Cmp {
        left: usize,
        right: usize,
        target_pc: usize,
    },
    Jump {
        target_pc: usize,
    },
    Load {
        register: usize,
        address: usize,
    },
    Store {
        register: usize,
        address: usize,
    },
    Halt,
    // Smart Contract
    Call {
        target_pc: usize,
    },
    Ret,
    Push {
        value: u64,
    },
    Pop {
        register: usize,
    },
    CallData {
        register: usize,
        offset: usize,
    },
    Log {
        register: usize,
    },
    SelfBalance {
        register: usize,
    },
    Sha256 {
        source: usize,
        destination: usize,
    },
}

// Strategy: Move Dependencies Down for vm (Infrastructure)
// Review and adjust before applying.
