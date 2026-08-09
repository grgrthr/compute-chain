use crate::trace::trace_record::TraceRecord;
use crate::vm::instruction::Instruction;

pub struct TraceSerializer;

impl TraceSerializer {
    pub fn serialize_record(record: &TraceRecord) -> String {
        format!(
            "step={},pc={},op={},reg_before={:?},reg_after={:?}",
            record.step,
            record.pc,
            Self::serialize_instruction(&record.instruction),
            record.registers_before,
            record.registers_after,
        )
    }

    pub fn serialize_instruction(instruction: &Instruction) -> String {
        match instruction {
            Instruction::Mov { register, value } => {
                format!("MOV r{}={}", register, value)
            }
            Instruction::Add {
                destination,
                source,
            } => {
                format!("ADD r{}+=r{}", destination, source)
            }
            Instruction::Sub {
                destination,
                source,
            } => {
                format!("SUB r{}-=r{}", destination, source)
            }
            Instruction::Mul {
                destination,
                source,
            } => {
                format!("MUL r{}*=r{}", destination, source)
            }
            Instruction::Div {
                destination,
                source,
            } => {
                format!("DIV r{}/=r{}", destination, source)
            }
            Instruction::Cmp {
                left,
                right,
                target_pc,
            } => {
                format!("CMP r{}==r{} ? {}", left, right, target_pc)
            }
            Instruction::Jump { target_pc } => {
                format!("JMP {}", target_pc)
            }
            Instruction::Load { register, address } => {
                format!("LOAD r{}=[{}]", register, address)
            }
            Instruction::Store { register, address } => {
                format!("STORE [{}]=r{}", address, register)
            }
            Instruction::Halt => "HALT".to_string(),
            Instruction::Call { target_pc } => format!("CALL {}", target_pc),
            Instruction::Ret => "RET".to_string(),
            Instruction::Push { value } => format!("PUSH {}", value),
            Instruction::Pop { register } => format!("POP r{}", register),
            Instruction::CallData { register, offset } => {
                format!("CALLDATA r{}[{}]", register, offset)
            }
            Instruction::Log { register } => format!("LOG r{}", register),
            Instruction::SelfBalance { register } => format!("BALANCE r{}", register),
            Instruction::Sha256 {
                source,
                destination,
            } => format!("SHA256 r{}->r{}", source, destination),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_mov() {
        let inst = Instruction::Mov {
            register: 1,
            value: 42,
        };
        assert_eq!(TraceSerializer::serialize_instruction(&inst), "MOV r1=42");
    }

    #[test]
    fn test_serialize_push_pop() {
        let push = Instruction::Push { value: 100 };
        let pop = Instruction::Pop { register: 2 };
        assert_eq!(TraceSerializer::serialize_instruction(&push), "PUSH 100");
        assert_eq!(TraceSerializer::serialize_instruction(&pop), "POP r2");
    }
}
