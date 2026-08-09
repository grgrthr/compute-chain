use sha2::{Digest, Sha256};

pub struct TraceHasher;

impl TraceHasher {
    // النسخة الجديدة (المستخدمة في main)
    pub fn hash_string(input: &str) -> String {
        Self::hash(input)
    }

    // النسخة المطلوبة من tree.rs (fix error)
    pub fn hash(input: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        let result = hasher.finalize();
        format!("{:x}", result)
    }
}

// Strategy: Move Dependencies Down for merkle (Infrastructure)
// Review and adjust before applying.
