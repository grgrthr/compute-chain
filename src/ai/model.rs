use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// دالة مساعدة للسجمويد
fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ModelType {
    LinearRegression,
    NeuralNetwork,
    RandomForest,
    Transformer,
    Custom,
}

impl std::fmt::Display for ModelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ModelType::LinearRegression => "LinearRegression",
            ModelType::NeuralNetwork => "NeuralNetwork",
            ModelType::RandomForest => "RandomForest",
            ModelType::Transformer => "Transformer",
            ModelType::Custom => "Custom",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIModel {
    pub id: String,
    pub name: String,
    pub version: String,
    pub model_type: ModelType,
    pub parameters: HashMap<String, f64>,
    pub accuracy: f64,
    pub size_mb: u64,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPrediction {
    pub model_id: String,
    pub input_hash: String,
    pub output: Vec<f64>,
    pub confidence: f64,
    pub execution_time_ms: u64,
    pub proof_hash: String,
}

pub struct ModelManager {
    models: Arc<Mutex<HashMap<String, AIModel>>>,
    predictions: Arc<Mutex<Vec<ModelPrediction>>>,
}

impl ModelManager {
    pub fn new() -> Self {
        let mut models = HashMap::new();

        models.insert(
            "default".to_string(),
            AIModel {
                id: "default".to_string(),
                name: "Simple Linear Model".to_string(),
                version: "1.0.0".to_string(),
                model_type: ModelType::LinearRegression,
                parameters: HashMap::new(),
                accuracy: 0.85,
                size_mb: 1,
                created_at: Self::current_time(),
            },
        );

        Self {
            models: Arc::new(Mutex::new(models)),
            predictions: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn register_model(&self, model: AIModel) -> String {
        let mut models = self.models.lock().unwrap();
        let id = model.id.clone();
        models.insert(id.clone(), model);
        id
    }

    pub fn get_model(&self, model_id: &str) -> Option<AIModel> {
        let models = self.models.lock().unwrap();
        models.get(model_id).cloned()
    }

    pub fn list_models(&self) -> Vec<AIModel> {
        let models = self.models.lock().unwrap();
        models.values().cloned().collect()
    }

    pub fn predict(&self, model_id: &str, input: &[f64]) -> Option<ModelPrediction> {
        let start = std::time::Instant::now();

        let model = self.get_model(model_id)?;

        let output = match model.model_type {
            ModelType::LinearRegression => self.linear_regression_predict(&model, input),
            ModelType::NeuralNetwork => self.neural_network_predict(&model, input),
            _ => self.default_predict(&model, input),
        };

        let execution_time = start.elapsed().as_millis() as u64;
        let input_hash = format!("{:?}", input);

        let prediction = ModelPrediction {
            model_id: model_id.to_string(),
            input_hash,
            output,
            confidence: model.accuracy,
            execution_time_ms: execution_time,
            proof_hash: format!("proof_{}", model_id),
        };

        let mut predictions = self.predictions.lock().unwrap();
        predictions.push(prediction.clone());

        if predictions.len() > 1000 {
            predictions.remove(0);
        }

        Some(prediction)
    }

    fn linear_regression_predict(&self, model: &AIModel, input: &[f64]) -> Vec<f64> {
        let w = model.parameters.get("weight").unwrap_or(&1.0);
        let b = model.parameters.get("bias").unwrap_or(&0.0);

        input.iter().map(|x| w * x + b).collect()
    }

    fn neural_network_predict(&self, model: &AIModel, input: &[f64]) -> Vec<f64> {
        let w1 = model.parameters.get("layer1_weight").unwrap_or(&0.5);
        let w2 = model.parameters.get("layer2_weight").unwrap_or(&0.3);

        input
            .iter()
            .map(|x| {
                let hidden = (x * w1).tanh();
                sigmoid(hidden * w2)
            })
            .collect()
    }

    fn default_predict(&self, _model: &AIModel, input: &[f64]) -> Vec<f64> {
        input.iter().map(|x| x * 2.0).collect()
    }

    pub fn get_prediction_history(&self, limit: usize) -> Vec<ModelPrediction> {
        let predictions = self.predictions.lock().unwrap();
        predictions.iter().rev().take(limit).cloned().collect()
    }

    pub fn update_model_accuracy(&self, model_id: &str, new_accuracy: f64) {
        let mut models = self.models.lock().unwrap();
        if let Some(model) = models.get_mut(model_id) {
            model.accuracy = (model.accuracy * 0.9 + new_accuracy * 0.1).min(1.0);
        }
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
    fn test_model_registration() {
        let manager = ModelManager::new();
        let models = manager.list_models();
        assert!(!models.is_empty());
    }

    #[test]
    fn test_prediction() {
        let manager = ModelManager::new();
        let prediction = manager.predict("default", &[1.0, 2.0, 3.0]);
        assert!(prediction.is_some());
        assert_eq!(prediction.unwrap().output.len(), 3);
    }
}
