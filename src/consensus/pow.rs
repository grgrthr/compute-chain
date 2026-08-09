use rand::Rng;
use sha2::{Digest, Sha256};

pub struct ProofOfWork {
    difficulty: u32,
}

impl ProofOfWork {
    pub fn new(difficulty: u32) -> Self {
        Self { difficulty }
    }

    pub fn mine(&self, data: &[u8]) -> (u64, Vec<u8>) {
        let mut rng = rand::thread_rng();
        let mut nonce = rng.gen::<u64>();

        loop {
            let hash = self.compute_hash(data, nonce);
            if self.is_valid(&hash) {
                return (nonce, hash);
            }
            nonce = nonce.wrapping_add(1);
        }
    }

    pub fn verify(&self, data: &[u8], nonce: u64, hash: &[u8]) -> bool {
        let computed_hash = self.compute_hash(data, nonce);
        computed_hash == hash && self.is_valid(&computed_hash)
    }

    fn compute_hash(&self, data: &[u8], nonce: u64) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.update(nonce.to_le_bytes());
        hasher.finalize().to_vec()
    }

    fn is_valid(&self, hash: &[u8]) -> bool {
        if hash.len() < 4 {
            return false;
        }

        let target = self.difficulty as usize;
        for i in 0..target.min(4) {
            if hash[i] != 0 {
                return false;
            }
        }
        true
    }

    pub fn adjust_difficulty(&mut self, last_block_time_ms: u64, target_time_ms: u64) {
        if last_block_time_ms < target_time_ms / 2 {
            self.difficulty = self.difficulty.saturating_add(1);
        } else if last_block_time_ms > target_time_ms * 2 {
            self.difficulty = self.difficulty.saturating_sub(1);
        }

        self.difficulty = self.difficulty.max(1).min(32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pow() {
        let pow = ProofOfWork::new(2);
        let data = b"test_block_data";

        let (nonce, hash) = pow.mine(data);
        assert!(pow.verify(data, nonce, &hash));
    }
}

// Strategy: Dependency Inversion for consensus (Core)
// Review and adjust before applying.
