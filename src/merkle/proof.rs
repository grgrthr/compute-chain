use crate::merkle::hash::TraceHasher;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MerkleProof {
    pub leaf_hash: String,
    pub sibling_hashes: Vec<String>,
}
impl MerkleProof {
    pub fn verify(&self, root: &str, i: usize) -> bool {
        let mut c = self.leaf_hash.clone();
        let mut idx = i;
        for s in &self.sibling_hashes {
            let combined = if idx % 2 == 0 {
                format!("{}{}", c, s)
            } else {
                format!("{}{}", s, c)
            };
            c = TraceHasher::hash_string(&combined);
            idx /= 2
        }
        c == root
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::merkle::tree::MerkleTree;
    #[test]
    fn test_valid() {
        let h: Vec<String> = vec!["a", "b", "c", "d"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let t = MerkleTree::new(h);
        assert!(t.generate_proof(0).unwrap().verify(&t.root_hash, 0))
    }
    #[test]
    fn test_invalid_root() {
        let h: Vec<String> = vec!["a", "b"].iter().map(|s| s.to_string()).collect();
        let t = MerkleTree::new(h);
        assert!(!t.generate_proof(0).unwrap().verify("wrong", 0))
    }
    #[test]
    fn test_wrong_index() {
        let h: Vec<String> = vec!["a", "b"].iter().map(|s| s.to_string()).collect();
        let t = MerkleTree::new(h);
        assert!(!t.generate_proof(0).unwrap().verify(&t.root_hash, 1))
    }
}
