use crate::trace::step_result::StepResult;
use crate::vm::cpu::Cpu;
use crate::vm::instruction::Instruction;
use crate::vm::memory::Memory;
use crate::vm::program::Program;

pub struct Executor;

impl Executor {
    pub fn step(cpu: &mut Cpu, memory: &mut Memory, program: &Program) -> Option<StepResult> {
        if cpu.halted {
            return None;
        }

        let pc = cpu.pc;

        if pc >= program.instructions.len() {
            cpu.halted = true;
            return None;
        }

        let instruction = program.instructions[pc].clone();
        let registers_before = cpu.registers.to_vec();

        match instruction {
            Instruction::Mov { register, value } => {
                if register < 8 {
                    cpu.registers[register] = value;
                }
                cpu.pc += 1;
            }
            Instruction::Add {
                destination,
                source,
            } => {
                if destination < 8 && source < 8 {
                    let result = cpu.registers[destination].wrapping_add(cpu.registers[source]);
                    cpu.registers[destination] = result;
                }
                cpu.pc += 1;
            }
            Instruction::Sub {
                destination,
                source,
            } => {
                if destination < 8 && source < 8 {
                    let result = if cpu.registers[destination] >= cpu.registers[source] {
                        cpu.registers[destination] - cpu.registers[source]
                    } else {
                        0
                    };
                    cpu.registers[destination] = result;
                }
                cpu.pc += 1;
            }
            Instruction::Mul {
                destination,
                source,
            } => {
                if destination < 8 && source < 8 {
                    let result = cpu.registers[destination].wrapping_mul(cpu.registers[source]);
                    cpu.registers[destination] = result;
                }
                cpu.pc += 1;
            }
            Instruction::Div {
                destination,
                source,
            } => {
                if destination < 8 && source < 8 {
                    if cpu.registers[source] != 0 {
                        let result = cpu.registers[destination] / cpu.registers[source];
                        cpu.registers[destination] = result;
                    } else {
                        cpu.registers[destination] = 0;
                    }
                }
                cpu.pc += 1;
            }
            Instruction::Cmp {
                left,
                right,
                target_pc,
            } => {
                if left < 8 && right < 8 {
                    cpu.cmp_flag = cpu.registers[left] == cpu.registers[right];
                    if cpu.cmp_flag {
                        cpu.pc = target_pc;
                    } else {
                        cpu.pc += 1;
                    }
                } else {
                    cpu.pc += 1;
                }
            }
            Instruction::Jump { target_pc } => {
                cpu.pc = target_pc;
            }
            Instruction::Store { register, address } => {
                if register < 8 {
                    let value = cpu.registers[register];
                    if address + 8 <= memory.data.len() {
                        memory.write_u64(address, value);
                    }
                }
                cpu.pc += 1;
            }
            Instruction::Load { register, address } => {
                if register < 8 {
                    if address + 8 <= memory.data.len() {
                        let value = memory.read_u64(address);
                        cpu.registers[register] = value;
                    } else {
                        cpu.registers[register] = 0;
                    }
                }
                cpu.pc += 1;
            }
            Instruction::Halt => {
                cpu.halted = true;
            }
            // 🆕 Smart Contract Instructions
            Instruction::Call { target_pc } => {
                cpu.push(cpu.pc as u64 + 1);
                cpu.pc = target_pc;
            }
            Instruction::Ret => {
                if let Some(return_pc) = cpu.pop() {
                    cpu.pc = return_pc as usize;
                } else {
                    cpu.halted = true;
                }
            }
            Instruction::Push { value } => {
                cpu.push(value);
                cpu.pc += 1;
            }
            Instruction::Pop { register } => {
                if register < 8 {
                    cpu.registers[register] = cpu.pop().unwrap_or(0);
                }
                cpu.pc += 1;
            }
            Instruction::CallData { register, offset } => {
                if register < 8 {
                    cpu.registers[register] = cpu.calldata.get(offset).copied().unwrap_or(0);
                }
                cpu.pc += 1;
            }
            Instruction::Log { register } => {
                if register < 8 {
                    println!("📝 LOG[{}]: {}", register, cpu.registers[register]);
                }
                cpu.pc += 1;
            }
            Instruction::SelfBalance { register } => {
                if register < 8 {
                    cpu.registers[register] = 0;
                }
                cpu.pc += 1;
            }
            Instruction::Sha256 {
                source,
                destination,
            } => {
                if source < 8 && destination < 8 {
                    use sha2::{Digest, Sha256 as Sha256Hasher};
                    let mut hasher = Sha256Hasher::new();
                    hasher.update(cpu.registers[source].to_le_bytes());
                    let hash = hasher.finalize();
                    let hash_u64 = u64::from_le_bytes(hash[..8].try_into().unwrap_or([0; 8]));
                    cpu.registers[destination] = hash_u64;
                }
                cpu.pc += 1;
            }
        }

        let registers_after = cpu.registers.to_vec();

        Some(StepResult::new(
            pc,
            instruction,
            registers_before,
            registers_after,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::program::Program;

    #[test]
    fn test_push_pop() {
        let mut cpu = Cpu::new();
        let mut memory = Memory::new(1024);
        let program = Program::new(vec![
            Instruction::Push { value: 42 },
            Instruction::Pop { register: 1 },
            Instruction::Halt,
        ]);
        Executor::step(&mut cpu, &mut memory, &program);
        Executor::step(&mut cpu, &mut memory, &program);
        assert_eq!(cpu.registers[1], 42);
    }

    #[test]
    fn test_call_ret() {
        let mut cpu = Cpu::new();
        let mut memory = Memory::new(1024);
        let program = Program::new(vec![
            Instruction::Call { target_pc: 3 },
            Instruction::Mov {
                register: 1,
                value: 99,
            },
            Instruction::Halt,
            Instruction::Push { value: 1 },
            Instruction::Pop { register: 2 },
            Instruction::Ret,
        ]);
        // CALL -> PUSH -> POP -> RET -> MOV
        Executor::step(&mut cpu, &mut memory, &program); // CALL
        assert_eq!(cpu.pc, 3);
        Executor::step(&mut cpu, &mut memory, &program); // PUSH
        Executor::step(&mut cpu, &mut memory, &program); // POP
        assert_eq!(cpu.registers[2], 1);
        Executor::step(&mut cpu, &mut memory, &program); // RET
        assert_eq!(cpu.pc, 1); // رجع للتعليمة بعد CALL
    }

    #[test]
    fn test_mov_instruction() {
        let mut cpu = Cpu::new();
        let mut memory = Memory::new(1024);
        let program = Program::new(vec![
            Instruction::Mov {
                register: 1,
                value: 42,
            },
            Instruction::Halt,
        ]);
        Executor::step(&mut cpu, &mut memory, &program);
        assert_eq!(cpu.registers[1], 42);
    }
}

// Strategy: Move Dependencies Down for vm (Infrastructure)
// Review and adjust before applying.
