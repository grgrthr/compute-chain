use crate::ai::model::{AIModel, ModelManager, ModelType};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingJob {
    pub id: String,
    pub model_type: ModelType,
    pub training_data: Vec<TrainingSample>,
    pub epochs: u32,
    pub learning_rate: f64,
    pub status: TrainingStatus,
    pub created_at: u64,
    pub completed_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingSample {
    pub input: Vec<f64>,
    pub output: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TrainingStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingResult {
    pub job_id: String,
    pub model_id: String,
    pub final_accuracy: f64,
    pub training_time_ms: u64,
    pub epochs_completed: u32,
}

pub struct TrainingEngine {
    jobs: Arc<Mutex<Vec<TrainingJob>>>,
    results: Arc<Mutex<Vec<TrainingResult>>>,
    model_manager: Arc<ModelManager>,
}

impl TrainingEngine {
    pub fn new(model_manager: Arc<ModelManager>) -> Self {
        Self {
            jobs: Arc::new(Mutex::new(Vec::new())),
            results: Arc::new(Mutex::new(Vec::new())),
            model_manager,
        }
    }

    pub fn submit_training_job(
        &self,
        model_type: ModelType,
        samples: Vec<TrainingSample>,
        epochs: u32,
        learning_rate: f64,
    ) -> String {
        let job_id = uuid::Uuid::new_v4().to_string();
        let job = TrainingJob {
            id: job_id.clone(),
            model_type,
            training_data: samples,
            epochs,
            learning_rate,
            status: TrainingStatus::Pending,
            created_at: Self::current_time(),
            completed_at: None,
        };

        let mut jobs = self.jobs.lock().unwrap();
        jobs.push(job);

        job_id
    }

    pub fn train_next(&self) -> Option<TrainingResult> {
        let job_index = {
            let jobs = self.jobs.lock().unwrap();
            jobs.iter()
                .position(|j| j.status == TrainingStatus::Pending)
        };

        if let Some(index) = job_index {
            let mut jobs = self.jobs.lock().unwrap();
            let job = &mut jobs[index];
            job.status = TrainingStatus::Running;

            let start = std::time::Instant::now();

            let accuracy = self.simulate_training(job);

            let training_time = start.elapsed().as_millis() as u64;

            let model = AIModel {
                id: uuid::Uuid::new_v4().to_string(),
                name: format!("Trained Model {:?}", job.model_type),
                version: "1.0.0".to_string(),
                model_type: job.model_type.clone(),
                parameters: self.extract_parameters(job),
                accuracy,
                size_mb: (job.training_data.len() / 1000) as u64,
                created_at: Self::current_time(),
            };

            let model_id = self.model_manager.register_model(model);

            job.status = TrainingStatus::Completed;
            job.completed_at = Some(Self::current_time());

            let result = TrainingResult {
                job_id: job.id.clone(),
                model_id,
                final_accuracy: accuracy,
                training_time_ms: training_time,
                epochs_completed: job.epochs,
            };

            let mut results = self.results.lock().unwrap();
            results.push(result.clone());

            Some(result)
        } else {
            None
        }
    }

    fn simulate_training(&self, job: &TrainingJob) -> f64 {
        let base_accuracy = 0.5;
        let sample_boost = (job.training_data.len() as f64 / 100.0).min(0.3);
        let epoch_boost = (job.epochs as f64 / 10.0).min(0.2);
        let lr_boost = job.learning_rate.min(0.1);

        (base_accuracy + sample_boost + epoch_boost + lr_boost).min(0.95)
    }

    fn extract_parameters(&self, job: &TrainingJob) -> std::collections::HashMap<String, f64> {
        let mut params = std::collections::HashMap::new();

        match job.model_type {
            ModelType::LinearRegression => {
                params.insert("weight".to_string(), 0.5 + (job.learning_rate * 10.0));
                params.insert("bias".to_string(), 0.1);
            }
            ModelType::NeuralNetwork => {
                params.insert("layer1_weight".to_string(), 0.6);
                params.insert("layer2_weight".to_string(), 0.4);
            }
            _ => {
                params.insert("factor".to_string(), 1.0);
            }
        }

        params
    }

    pub fn get_jobs(&self) -> Vec<TrainingJob> {
        let jobs = self.jobs.lock().unwrap();
        jobs.clone()
    }

    pub fn get_results(&self, limit: usize) -> Vec<TrainingResult> {
        let results = self.results.lock().unwrap();
        results.iter().rev().take(limit).cloned().collect()
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
    fn test_training() {
        let model_manager = Arc::new(ModelManager::new());
        let engine = TrainingEngine::new(model_manager);

        let samples = vec![
            TrainingSample {
                input: vec![1.0],
                output: vec![2.0],
            },
            TrainingSample {
                input: vec![2.0],
                output: vec![4.0],
            },
        ];

        let job_id = engine.submit_training_job(ModelType::LinearRegression, samples, 10, 0.01);
        assert!(!job_id.is_empty());

        let result = engine.train_next();
        assert!(result.is_some());
        assert!(result.unwrap().final_accuracy > 0.5);
    }
}
