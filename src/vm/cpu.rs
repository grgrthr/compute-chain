#[derive(Debug, Clone, PartialEq)]
pub struct Cpu {
    pub registers: [u64; 8],
    pub pc: usize,
    pub halted: bool,
    pub cmp_flag: bool,
    pub stack: Vec<u64>,
    pub calldata: Vec<u64>,
    pub memory_hash: Vec<u8>,
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            registers: [0; 8],
            pc: 0,
            halted: false,
            cmp_flag: false,
            stack: Vec::new(),
            calldata: Vec::new(),
            memory_hash: Vec::new(),
        }
    }

    pub fn push(&mut self, value: u64) {
        self.stack.push(value);
    }

    pub fn pop(&mut self) -> Option<u64> {
        self.stack.pop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_new() {
        let cpu = Cpu::new();
        assert_eq!(cpu.registers, [0, 0, 0, 0, 0, 0, 0, 0]);
        assert!(cpu.stack.is_empty());
    }

    #[test]
    fn test_push_pop() {
        let mut cpu = Cpu::new();
        cpu.push(42);
        assert_eq!(cpu.pop(), Some(42));
        assert!(cpu.stack.is_empty());
    }
}

// Strategy: Move Dependencies Down for vm (Infrastructure)
// Review and adjust before applying.
