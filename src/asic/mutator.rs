use crate::workload::types::WorkloadInstruction;
use rand::Rng;

#[derive(Debug, Clone)]
pub struct WorkloadMutator;

impl WorkloadMutator {
    pub fn new() -> Self {
        Self
    }

    pub fn mutate_instructions(
        &self,
        instructions: &[WorkloadInstruction],
    ) -> Vec<WorkloadInstruction> {
        let mut rng = rand::thread_rng();
        let mut mutated = instructions.to_vec();

        for i in 0..mutated.len() {
            if rng.gen_bool(0.3) {
                mutated[i] = self.mutate_single(&mutated[i]);
            }
        }

        mutated
    }

    pub fn mutate_single(&self, instruction: &WorkloadInstruction) -> WorkloadInstruction {
        let mut rng = rand::thread_rng();
        let mut new_params = instruction.params.clone();

        if !new_params.is_empty() {
            let param_idx = rng.gen_range(0..new_params.len());
            let delta = rng.gen_range(1..10);

            if rng.gen_bool(0.5) {
                new_params[param_idx] = new_params[param_idx].saturating_add(delta);
            } else {
                new_params[param_idx] = new_params[param_idx].saturating_sub(delta);
            }
        }

        WorkloadInstruction {
            opcode: instruction.opcode.clone(),
            params: new_params,
        }
    }

    pub fn mutate_opcode(&self, instructions: &[WorkloadInstruction]) -> Vec<WorkloadInstruction> {
        let mut rng = rand::thread_rng();
        let opcodes = vec!["MOV", "ADD", "SUB", "MUL", "LOAD", "STORE"];
        let mut mutated = instructions.to_vec();

        for i in 0..mutated.len() {
            if rng.gen_bool(0.2) {
                let new_opcode = opcodes[rng.gen_range(0..opcodes.len())];
                let new_params = self.generate_params_for_opcode(new_opcode);
                mutated[i] = WorkloadInstruction {
                    opcode: new_opcode.to_string(),
                    params: new_params,
                };
            }
        }

        mutated
    }

    pub fn generate_params_for_opcode(&self, opcode: &str) -> Vec<u64> {
        let mut rng = rand::thread_rng();
        match opcode {
            "MOV" => vec![rng.gen_range(0..8), rng.gen_range(1..100)],
            "ADD" | "SUB" | "MUL" => vec![rng.gen_range(0..8), rng.gen_range(0..8)],
            "LOAD" | "STORE" => vec![rng.gen_range(0..8), rng.gen_range(0..1000)],
            _ => vec![],
        }
    }

    pub fn mutate_memory_pressure(
        &self,
        instructions: &[WorkloadInstruction],
    ) -> Vec<WorkloadInstruction> {
        let mut rng = rand::thread_rng();
        let mut mutated = instructions.to_vec();

        for _ in 0..(mutated.len() / 10).max(1) {
            let pos = rng.gen_range(0..mutated.len());
            let new_inst = WorkloadInstruction {
                opcode: if rng.gen_bool(0.5) {
                    "LOAD".to_string()
                } else {
                    "STORE".to_string()
                },
                params: vec![rng.gen_range(0..8), rng.gen_range(0..2000)],
            };
            mutated.insert(pos, new_inst);
        }

        mutated
    }

    pub fn mutate_workload(
        &self,
        instructions: &[WorkloadInstruction],
        intensity: f64,
    ) -> Vec<WorkloadInstruction> {
        let mut result = instructions.to_vec();

        if intensity > 0.2 {
            result = self.mutate_instructions(&result);
        }
        if intensity > 0.4 {
            result = self.mutate_opcode(&result);
        }
        if intensity > 0.6 {
            result = self.mutate_memory_pressure(&result);
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workload::generator::WorkloadGenerator;
    use crate::workload::types::WorkloadType;

    #[test]
    fn test_mutate_workload() {
        let mutator = WorkloadMutator::new();
        let workload = WorkloadGenerator::generate_with_type(3, WorkloadType::ComputeHeavy);

        let mutated = mutator.mutate_workload(&workload.instructions, 0.8);
        assert!(!mutated.is_empty());
    }
}
