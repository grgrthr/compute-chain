use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIProof {
    pub inference_id: String,
    pub model_hash: String,
    pub input_hash: String,
    pub output_hash: String,
    pub proof_data: Vec<u8>,
    pub verification_key: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub valid: bool,
    pub message: String,
}

pub struct AIProofGenerator;

impl AIProofGenerator {
    pub fn new() -> Self {
        Self
    }

    pub fn generate_proof(&self, model_data: &[u8], input: &[f64], output: &[f64]) -> AIProof {
        let inference_id = uuid::Uuid::new_v4().to_string();

        let model_hash = Self::hash_bytes(model_data);
        let input_hash = Self::hash_f64(input);
        let output_hash = Self::hash_f64(output);

        let proof_data = Self::create_proof_data(model_data, input, output);

        AIProof {
            inference_id,
            model_hash,
            input_hash,
            output_hash,
            proof_data,
            verification_key: "simple_zk_proof".to_string(),
            timestamp: Self::current_time(),
        }
    }

    pub fn verify_proof(
        &self,
        proof: &AIProof,
        model_data: &[u8],
        input: &[f64],
        output: &[f64],
    ) -> VerificationResult {
        let model_hash = Self::hash_bytes(model_data);
        let input_hash = Self::hash_f64(input);
        let output_hash = Self::hash_f64(output);

        if model_hash != proof.model_hash {
            return VerificationResult {
                valid: false,
                message: "Model hash mismatch".to_string(),
            };
        }

        if input_hash != proof.input_hash {
            return VerificationResult {
                valid: false,
                message: "Input hash mismatch".to_string(),
            };
        }

        if output_hash != proof.output_hash {
            return VerificationResult {
                valid: false,
                message: "Output hash mismatch".to_string(),
            };
        }

        // التحقق من صحة الإثبات
        let expected_proof = Self::create_proof_data(model_data, input, output);
        if proof.proof_data != expected_proof {
            return VerificationResult {
                valid: false,
                message: "Proof data invalid".to_string(),
            };
        }

        VerificationResult {
            valid: true,
            message: "Proof verified successfully".to_string(),
        }
    }

    fn hash_bytes(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    fn hash_f64(values: &[f64]) -> String {
        let mut hasher = Sha256::new();
        for &value in values {
            hasher.update(&value.to_le_bytes());
        }
        hex::encode(hasher.finalize())
    }

    fn create_proof_data(model_data: &[u8], input: &[f64], output: &[f64]) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(model_data);

        for &value in input {
            data.extend_from_slice(&value.to_le_bytes());
        }

        for &value in output {
            data.extend_from_slice(&value.to_le_bytes());
        }

        let mut hasher = Sha256::new();
        hasher.update(&data);
        hasher.finalize().to_vec()
    }

    fn current_time() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proof_generation_and_verification() {
        let generator = AIProofGenerator::new();

        let model_data = b"simple_model_v1";
        let input = vec![1.0, 2.0, 3.0];
        let output = vec![2.0, 4.0, 6.0];

        let proof = generator.generate_proof(model_data, &input, &output);

        let result = generator.verify_proof(&proof, model_data, &input, &output);
        assert!(result.valid);

        let wrong_output = vec![3.0, 5.0, 7.0];
        let wrong_result = generator.verify_proof(&proof, model_data, &input, &wrong_output);
        assert!(!wrong_result.valid);
    }
}
