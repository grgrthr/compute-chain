use crate::merkle::hash::TraceHasher;
use crate::merkle::proof::MerkleProof;

#[derive(Debug, Clone, PartialEq)]
pub struct MerkleTree {
    pub leaves: Vec<String>,
    pub root_hash: String,
}

impl MerkleTree {
    pub fn new(leaves: Vec<String>) -> Self {
        let root_hash = Self::compute_root(&leaves);
        Self { leaves, root_hash }
    }

    fn compute_root(leaves: &[String]) -> String {
        if leaves.is_empty() {
            return TraceHasher::hash_string("empty");
        }

        let mut current_level: Vec<String> = leaves.to_vec();

        while current_level.len() > 1 {
            let mut next_level = Vec::new();

            for i in (0..current_level.len()).step_by(2) {
                let left = &current_level[i];
                let right = if i + 1 < current_level.len() {
                    &current_level[i + 1]
                } else {
                    left
                };
                let combined = format!("{}{}", left, right);
                next_level.push(TraceHasher::hash_string(&combined));
            }

            current_level = next_level;
        }

        current_level[0].clone()
    }

    pub fn generate_proof(&self, index: usize) -> Option<MerkleProof> {
        if index >= self.leaves.len() {
            return None;
        }

        let leaf_hash = self.leaves[index].clone();
        let mut sibling_hashes = Vec::new();
        let mut current_index = index;
        let mut current_level = self.leaves.clone();

        while current_level.len() > 1 {
            let sibling_index = if current_index % 2 == 0 {
                current_index + 1
            } else {
                current_index - 1
            };

            if sibling_index < current_level.len() {
                sibling_hashes.push(current_level[sibling_index].clone());
            }

            let mut next_level = Vec::new();
            for i in (0..current_level.len()).step_by(2) {
                let left = &current_level[i];
                let right = if i + 1 < current_level.len() {
                    &current_level[i + 1]
                } else {
                    left
                };
                let combined = format!("{}{}", left, right);
                next_level.push(TraceHasher::hash_string(&combined));
            }

            current_level = next_level;
            current_index /= 2;
        }

        Some(MerkleProof {
            leaf_hash,
            sibling_hashes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_leaves() -> Vec<String> {
        vec![
            TraceHasher::hash_string("leaf1"),
            TraceHasher::hash_string("leaf2"),
            TraceHasher::hash_string("leaf3"),
            TraceHasher::hash_string("leaf4"),
        ]
    }

    #[test]
    fn test_merkle_tree_new() {
        let leaves = create_test_leaves();
        let tree = MerkleTree::new(leaves.clone());
        assert_eq!(tree.leaves, leaves);
        assert!(!tree.root_hash.is_empty());
    }

    #[test]
    fn test_merkle_tree_single_leaf() {
        let leaves = vec![TraceHasher::hash_string("single")];
        let tree = MerkleTree::new(leaves);
        assert_eq!(tree.root_hash, TraceHasher::hash_string("single"));
    }

    #[test]
    fn test_merkle_tree_empty() {
        let leaves = vec![];
        let tree = MerkleTree::new(leaves);
        assert_eq!(tree.root_hash, TraceHasher::hash_string("empty"));
    }

    #[test]
    fn test_generate_proof_valid() {
        let leaves = create_test_leaves();
        let tree = MerkleTree::new(leaves);

        let proof = tree.generate_proof(0);
        assert!(proof.is_some());
    }

    #[test]
    fn test_generate_proof_invalid_index() {
        let leaves = create_test_leaves();
        let tree = MerkleTree::new(leaves);

        let proof = tree.generate_proof(99);
        assert!(proof.is_none());
    }
}

// Strategy: Move Dependencies Down for merkle (Infrastructure)
// Review and adjust before applying.
