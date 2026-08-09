use crate::merkle::proof::MerkleProof;
use crate::merkle::tree::MerkleTree;
use crate::trace::trace_record::TraceRecord;
use crate::trace::tracer::Tracer;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Challenge {
    pub step_index: usize,
    pub total_steps: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChallengeResponse {
    pub step: TraceRecord,
    pub merkle_proof: MerkleProof,
    pub prev_step_hash: String,
}

pub struct InteractiveVerifier {
    pub num_rounds: usize,
    pub error_probability: f64,
}

impl InteractiveVerifier {
    pub fn new(n: usize) -> Self {
        InteractiveVerifier {
            num_rounds: n,
            error_probability: 2_f64.powi(-(n as i32)),
        }
    }

    /// Deterministic challenge generation.
    ///
    /// Given the same `t` (total steps) and the same `num_rounds`,
    /// this function ALWAYS produces the same sequence of challenges.
    ///
    /// Strategy: pick evenly spaced step indices across the trace.
    /// For num_rounds = 3 and t = 4: [0, 1, 2]
    /// For num_rounds = 3 and t = 100: [0, 33, 66]
    pub fn generate_challenges(&self, t: usize) -> Vec<Challenge> {
        if t == 0 {
            return vec![];
        }

        let n = self.num_rounds.min(t);

        (0..n)
            .map(|i| {
                // Deterministic: evenly spaced across trace
                let step_index = if n == 1 {
                    0
                } else {
                    // Multiply first to avoid truncation to zero for small t
                    (i * t) / n
                };
                Challenge {
                    step_index: step_index.min(t - 1),
                    total_steps: t,
                }
            })
            .collect()
    }

    pub fn verify_challenge(&self, c: &Challenge, r: &ChallengeResponse, root: &str) -> bool {
        if !r.merkle_proof.verify(root, c.step_index) {
            return false;
        }
        if r.step.hash() != r.merkle_proof.leaf_hash {
            return false;
        }
        true
    }

    pub fn verify_interactively(
        &self,
        t: &Tracer,
        m: &MerkleTree,
        h: &dyn Fn(&Challenge) -> ChallengeResponse,
    ) -> bool {
        for c in &self.generate_challenges(t.records.len()) {
            if !self.verify_challenge(c, &h(c), &m.root_hash) {
                return false;
            }
        }
        true
    }
}

pub struct InteractiveProver {
    pub tracer: Tracer,
    pub merkle_tree: MerkleTree,
}

impl InteractiveProver {
    pub fn new(t: Tracer, m: MerkleTree) -> Self {
        InteractiveProver {
            tracer: t,
            merkle_tree: m,
        }
    }

    pub fn respond_to_challenge(&self, c: &Challenge) -> Option<ChallengeResponse> {
        if c.step_index >= self.tracer.records.len() {
            return None;
        }
        let s = self.tracer.records[c.step_index].clone();
        let p = self.merkle_tree.generate_proof(c.step_index)?;
        let prev = if c.step_index > 0 {
            self.tracer.records[c.step_index - 1].hash()
        } else {
            String::new()
        };
        Some(ChallengeResponse {
            step: s,
            merkle_proof: p,
            prev_step_hash: prev,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::cpu::Cpu;
    use crate::vm::executor::Executor;
    use crate::vm::instruction::Instruction;
    use crate::vm::memory::Memory;
    use crate::vm::program::Program;

    fn create() -> (Tracer, MerkleTree) {
        let mut c = Cpu::new();
        let mut m = Memory::new(65536);
        let p = Program::new(vec![
            Instruction::Mov {
                register: 0,
                value: 5,
            },
            Instruction::Mov {
                register: 1,
                value: 10,
            },
            Instruction::Add {
                destination: 0,
                source: 1,
            },
            Instruction::Halt,
        ]);
        let mut t = Tracer::new();
        let mut s = 0;
        while !c.halted {
            if let Some(r) = Executor::step(&mut c, &mut m, &p) {
                t.record(
                    s,
                    r.pc,
                    r.instruction,
                    r.registers_before.to_vec(),
                    r.registers_after.to_vec(),
                );
                s += 1
            } else {
                break;
            }
        }
        let h: Vec<String> = t.records.iter().map(|r| r.hash()).collect();
        (t, MerkleTree::new(h))
    }

    // ═══ DETERMINISM TESTS ═══

    #[test]
    fn test_deterministic_same_input_same_output() {
        let t = 100;
        let v = InteractiveVerifier::new(5);

        let first = v.generate_challenges(t);
        let second = v.generate_challenges(t);

        assert_eq!(first.len(), second.len());
        for i in 0..first.len() {
            assert_eq!(
                first[i].step_index, second[i].step_index,
                "Challenge {} differs: {} vs {} — generation is NON-deterministic",
                i, first[i].step_index, second[i].step_index
            );
        }
    }

    #[test]
    fn test_deterministic_different_verifiers_same_output() {
        let t = 50;
        let v1 = InteractiveVerifier::new(4);
        let v2 = InteractiveVerifier::new(4);

        let c1 = v1.generate_challenges(t);
        let c2 = v2.generate_challenges(t);

        assert_eq!(c1.len(), c2.len());
        for i in 0..c1.len() {
            assert_eq!(
                c1[i].step_index, c2[i].step_index,
                "Two verifiers with same params produced different challenges"
            );
        }
    }

    #[test]
    fn test_deterministic_edge_cases() {
        // t=0
        let v = InteractiveVerifier::new(3);
        assert!(v.generate_challenges(0).is_empty());

        // t=1
        let c = v.generate_challenges(1);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].step_index, 0);

        // rounds > steps
        let c = InteractiveVerifier::new(10).generate_challenges(3);
        assert_eq!(c.len(), 3); // clamped to t
    }

    // ═══ EXISTING TESTS (preserved) ═══

    #[test]
    fn test_passes() {
        let (t, c) = create();
        let p = InteractiveProver::new(t.clone(), c.clone());
        assert!(InteractiveVerifier::new(3)
            .verify_interactively(&t, &c, &|x| p.respond_to_challenge(x).unwrap()))
    }

    #[test]
    fn test_batch() {
        let (t, c) = create();
        let p = InteractiveProver::new(t.clone(), c.clone());
        assert!(InteractiveVerifier::new(5)
            .verify_interactively(&t, &c, &|x| p.respond_to_challenge(x).unwrap()))
    }

    #[test]
    fn test_invalid() {
        let (_, c) = create();
        let p = InteractiveProver::new(Tracer::new(), c);
        assert!(p
            .respond_to_challenge(&Challenge {
                step_index: 999,
                total_steps: 4
            })
            .is_none())
    }

    #[test]
    fn test_real_compute() {
        let mut cpu = Cpu::new();
        let mut m = Memory::new(65536);
        let prog = Program::new(vec![
            Instruction::Mov {
                register: 0,
                value: 42,
            },
            Instruction::Mov {
                register: 1,
                value: 7,
            },
            Instruction::Mul {
                destination: 0,
                source: 1,
            },
            Instruction::Halt,
        ]);
        let mut t = Tracer::new();
        let mut s = 0;
        while !cpu.halted {
            if let Some(r) = Executor::step(&mut cpu, &mut m, &prog) {
                t.record(
                    s,
                    r.pc,
                    r.instruction.clone(),
                    r.registers_before.to_vec(),
                    r.registers_after.to_vec(),
                );
                s += 1
            } else {
                break;
            }
        }
        let h: Vec<String> = t.records.iter().map(|r| r.hash()).collect();
        let tree = MerkleTree::new(h);
        let p = InteractiveProver::new(t.clone(), tree.clone());
        assert!(InteractiveVerifier::new(3)
            .verify_interactively(&t, &tree, &|x| p.respond_to_challenge(x).unwrap()));
        assert_eq!(cpu.registers[0], 294)
    }
}
