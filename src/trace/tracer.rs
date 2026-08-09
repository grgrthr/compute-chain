use crate::trace::trace_record::TraceRecord;

#[derive(Debug, Clone)]
pub struct Tracer {
    pub records: Vec<TraceRecord>,
}

impl Tracer {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    pub fn record(
        &mut self,
        step: usize,
        pc: usize,
        instruction: crate::vm::instruction::Instruction,
        registers_before: Vec<u64>,
        registers_after: Vec<u64>,
    ) {
        self.records.push(TraceRecord {
            step,
            pc,
            instruction,
            registers_before,
            registers_after,
        });
    }

    pub fn verify_trace(&self) -> bool {
        if self.records.is_empty() {
            return false;
        }

        for (i, record) in self.records.iter().enumerate() {
            if record.step != i {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::instruction::Instruction;

    #[test]
    fn test_tracer_new() {
        let tracer = Tracer::new();
        assert!(tracer.records.is_empty());
    }

    #[test]
    fn test_tracer_record() {
        let mut tracer = Tracer::new();

        tracer.record(
            0,
            0,
            Instruction::Mov {
                register: 1,
                value: 10,
            },
            vec![0, 0, 0],
            vec![10, 0, 0],
        );

        assert_eq!(tracer.records.len(), 1);
        assert_eq!(tracer.records[0].step, 0);
        assert_eq!(tracer.records[0].pc, 0);
    }

    #[test]
    fn test_verify_trace_valid() {
        let mut tracer = Tracer::new();

        tracer.record(0, 0, Instruction::Halt, vec![], vec![]);
        tracer.record(1, 1, Instruction::Halt, vec![], vec![]);

        assert!(tracer.verify_trace());
    }

    #[test]
    fn test_verify_trace_invalid_step_order() {
        let mut tracer = Tracer::new();

        tracer.record(0, 0, Instruction::Halt, vec![], vec![]);
        tracer.record(2, 1, Instruction::Halt, vec![], vec![]);

        assert!(!tracer.verify_trace());
    }

    #[test]
    fn test_verify_trace_empty() {
        let tracer = Tracer::new();
        assert!(!tracer.verify_trace());
    }
}
