use crate::vm::instruction::Instruction;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq)]
pub struct WorkloadConfig {
    pub network_seed: String,
    pub block_height: u64,
    pub difficulty: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Workload {
    pub config: WorkloadConfig,
    pub instructions: Vec<Instruction>,
    pub instruction_count: usize,
    pub memory_required: usize,
    pub seed_hash: String,
}

pub struct WorkloadGenerator;

impl WorkloadGenerator {
    pub fn generate(config: &WorkloadConfig) -> Workload {
        let seed_hash = Self::compute_seed_hash(config);
        let instruction_count = Self::compute_instruction_count(config);
        let instructions =
            Self::generate_instructions(&seed_hash, instruction_count, config.difficulty);
        Workload {
            config: config.clone(),
            instruction_count: instructions.len(),
            memory_required: 1024 + (config.difficulty as usize * 512),
            seed_hash,
            instructions,
        }
    }

    pub fn compute_seed_hash(config: &WorkloadConfig) -> String {
        let mut hasher = Sha256::new();
        hasher.update(config.network_seed.as_bytes());
        hasher.update(b":");
        hasher.update(config.block_height.to_le_bytes());
        hasher.update(b":");
        hasher.update(config.difficulty.to_le_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn compute_instruction_count(config: &WorkloadConfig) -> usize {
        let base = 10;
        let extra = (config.difficulty as usize * 8).min(190);
        base + extra
    }

    fn derive_u64(seed_bytes: &[u8], index: usize) -> u64 {
        let mut hasher = Sha256::new();
        hasher.update(seed_bytes);
        hasher.update(index.to_le_bytes());
        let hash = hasher.finalize();
        u64::from_le_bytes(hash[..8].try_into().unwrap_or([0; 8]))
    }

    fn generate_instructions(seed_hash: &str, count: usize, _difficulty: u32) -> Vec<Instruction> {
        let seed_bytes = hex::decode(seed_hash).unwrap_or_else(|_| vec![0u8; 32]);
        let mut instructions = Vec::with_capacity(count + 1);
        let mut idx = 0;

        while instructions.len() < count {
            let selector = Self::derive_u64(&seed_bytes, idx);
            idx += 1;
            let op_type = (selector % 100) as u8;

            let instruction = match op_type {
                0..=39 => {
                    let op = selector % 5;
                    let r1 = (Self::derive_u64(&seed_bytes, idx) % 4) as usize;
                    idx += 1;
                    let r2_val = Self::derive_u64(&seed_bytes, idx);
                    idx += 1;
                    match op {
                        0 => Instruction::Mov {
                            register: r1,
                            value: r2_val % 1000 + 1,
                        },
                        1 => Instruction::Add {
                            destination: r1,
                            source: (r2_val % 4) as usize,
                        },
                        2 => Instruction::Sub {
                            destination: r1,
                            source: (r2_val % 4) as usize,
                        },
                        3 => Instruction::Mul {
                            destination: r1,
                            source: (r2_val % 4) as usize,
                        },
                        _ => Instruction::Div {
                            destination: r1,
                            source: ((r2_val % 3) + 1) as usize,
                        },
                    }
                }
                40..=59 => {
                    let r = (Self::derive_u64(&seed_bytes, idx) % 4) as usize;
                    idx += 1;
                    let addr = Self::derive_u64(&seed_bytes, idx) as usize % 500 + 100;
                    idx += 1;
                    if selector % 2 == 0 {
                        Instruction::Store {
                            register: r,
                            address: addr,
                        }
                    } else {
                        Instruction::Load {
                            register: r,
                            address: addr,
                        }
                    }
                }
                60..=74 => {
                    let r = (Self::derive_u64(&seed_bytes, idx) % 4) as usize;
                    idx += 1;
                    let val = Self::derive_u64(&seed_bytes, idx);
                    idx += 1;
                    if selector % 2 == 0 {
                        Instruction::Push {
                            value: val % 500 + 1,
                        }
                    } else {
                        Instruction::Pop { register: r }
                    }
                }
                75..=84 => {
                    let r = (Self::derive_u64(&seed_bytes, idx) % 4) as usize;
                    idx += 1;
                    let offset = Self::derive_u64(&seed_bytes, idx) as usize % 10;
                    idx += 1;
                    Instruction::CallData {
                        register: r,
                        offset,
                    }
                }
                85..=94 => {
                    let src = (Self::derive_u64(&seed_bytes, idx) % 4) as usize;
                    idx += 1;
                    let dst = (Self::derive_u64(&seed_bytes, idx) % 4) as usize;
                    idx += 1;
                    Instruction::Sha256 {
                        source: src,
                        destination: dst,
                    }
                }
                _ => {
                    let src = (Self::derive_u64(&seed_bytes, idx) % 4) as usize;
                    idx += 1;
                    let dst = (Self::derive_u64(&seed_bytes, idx) % 4) as usize;
                    idx += 1;
                    Instruction::Sha256 {
                        source: src,
                        destination: dst,
                    }
                }
            };
            instructions.push(instruction);
        }

        instructions.push(Instruction::Halt);
        instructions
    }

    pub fn generate_with_type(
        difficulty_level: u32,
        _workload_type: crate::workload::types::WorkloadType,
    ) -> crate::workload::types::Workload {
        let config = WorkloadConfig {
            network_seed: "legacy".into(),
            block_height: 0,
            difficulty: difficulty_level,
        };
        let wl = Self::generate(&config);
        crate::workload::types::Workload {
            id: wl.seed_hash[..16].to_string(),
            workload_type: _workload_type,
            instructions: wl
                .instructions
                .iter()
                .map(|inst| crate::workload::types::WorkloadInstruction {
                    opcode: format!("{:?}", inst)
                        .split('{')
                        .next()
                        .unwrap_or("HALT")
                        .trim()
                        .to_uppercase(),
                    params: vec![],
                })
                .collect(),
            difficulty: crate::workload::types::Difficulty::new(difficulty_level),
            memory_required: wl.memory_required,
            compute_operations: wl.instruction_count as u64,
        }
    }

    pub fn generate_batch(
        count: usize,
        min_difficulty: u32,
        max_difficulty: u32,
    ) -> Vec<crate::workload::types::Workload> {
        (0..count)
            .map(|i| {
                let diff = min_difficulty + ((i as u32) % (max_difficulty - min_difficulty + 1));
                Self::generate_with_type(diff, crate::workload::types::WorkloadType::Mixed)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integration::runner::IntegrationRunner;

    fn test_config() -> WorkloadConfig {
        WorkloadConfig {
            network_seed: "test-seed-001".into(),
            block_height: 42,
            difficulty: 3,
        }
    }

    #[test]
    fn test_same_seed_same_workload() {
        let config = test_config();
        let w1 = WorkloadGenerator::generate(&config);
        let w2 = WorkloadGenerator::generate(&config);
        assert_eq!(w1.seed_hash, w2.seed_hash);
        assert_eq!(w1.instruction_count, w2.instruction_count);
    }

    #[test]
    fn test_different_seed_different_workload() {
        let mut config = test_config();
        let w1 = WorkloadGenerator::generate(&config);
        config.network_seed = "different".into();
        let w2 = WorkloadGenerator::generate(&config);
        assert_ne!(w1.seed_hash, w2.seed_hash);
    }

    #[test]
    fn test_different_difficulty_different_count() {
        let mut config = test_config();
        let w1 = WorkloadGenerator::generate(&config);
        config.difficulty = 8;
        let w2 = WorkloadGenerator::generate(&config);
        assert!(w2.instruction_count > w1.instruction_count);
    }

    #[test]
    fn test_program_always_halts() {
        let config = test_config();
        for _ in 0..10 {
            let wl = WorkloadGenerator::generate(&config);
            assert!(matches!(wl.instructions.last().unwrap(), Instruction::Halt));
        }
    }

    #[test]
    fn test_workload_executes_successfully() {
        let config = test_config();
        let wl = WorkloadGenerator::generate(&config);
        let mut runner = IntegrationRunner::new();
        let result = runner.run("wl_test", wl.instructions);
        assert!(result.success, "error={:?}", result.error);
        assert!(result.trace_length > 0);
    }

    #[test]
    fn test_workload_generates_valid_proof() {
        let config = test_config();
        let wl = WorkloadGenerator::generate(&config);
        let mut runner = IntegrationRunner::new();
        let result = runner.run("wl_proof", wl.instructions);
        assert!(result.proof_generated);
        assert!(result.proof_verified);
    }

    #[test]
    fn test_large_workload() {
        let config = WorkloadConfig {
            network_seed: "large".into(),
            block_height: 999,
            difficulty: 4,
        };
        let wl = WorkloadGenerator::generate(&config);
        assert!(wl.instruction_count > 15);
        let mut runner = IntegrationRunner::new();
        let result = runner.run("wl_large", wl.instructions);
        assert!(result.success, "error={:?}", result.error);
    }

    #[test]
    fn test_seed_hash_stable() {
        let config = test_config();
        assert_eq!(
            WorkloadGenerator::compute_seed_hash(&config),
            WorkloadGenerator::compute_seed_hash(&config)
        );
    }

    #[test]
    fn test_legacy_api() {
        let wl =
            WorkloadGenerator::generate_with_type(3, crate::workload::types::WorkloadType::Mixed);
        assert!(!wl.instructions.is_empty());
    }
}
