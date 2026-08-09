use crate::ai::model::ModelManager;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceRequest {
    pub id: String,
    pub model_id: String,
    pub input: Vec<f64>,
    pub priority: u32,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceResult {
    pub request_id: String,
    pub output: Vec<f64>,
    pub confidence: f64,
    pub execution_time_ms: u64,
    pub verified: bool,
}

pub struct InferenceEngine {
    requests: Arc<Mutex<VecDeque<InferenceRequest>>>,
    results: Arc<Mutex<Vec<InferenceResult>>>,
    model_manager: Arc<ModelManager>,
    max_queue_size: usize,
}

impl InferenceEngine {
    pub fn new(model_manager: Arc<ModelManager>) -> Self {
        Self {
            requests: Arc::new(Mutex::new(VecDeque::new())),
            results: Arc::new(Mutex::new(Vec::new())),
            model_manager,
            max_queue_size: 1000,
        }
    }

    pub fn submit_request(&self, model_id: String, input: Vec<f64>, priority: u32) -> String {
        let request_id = uuid::Uuid::new_v4().to_string();
        let request = InferenceRequest {
            id: request_id.clone(),
            model_id,
            input,
            priority,
            created_at: Self::current_time(),
        };

        let mut requests = self.requests.lock().unwrap();

        // إدراج حسب الأولوية
        let insert_pos = requests
            .iter()
            .position(|r| r.priority < priority)
            .unwrap_or(requests.len());
        requests.insert(insert_pos, request);

        // منع التراكم الزائد
        while requests.len() > self.max_queue_size {
            requests.pop_back();
        }

        request_id
    }

    pub fn process_next(&self) -> Option<InferenceResult> {
        let request = {
            let mut requests = self.requests.lock().unwrap();
            requests.pop_front()
        };

        if let Some(req) = request {
            let start = std::time::Instant::now();

            let prediction = self.model_manager.predict(&req.model_id, &req.input);

            let execution_time = start.elapsed().as_millis() as u64;

            let result = match prediction {
                Some(pred) => InferenceResult {
                    request_id: req.id,
                    output: pred.output,
                    confidence: pred.confidence,
                    execution_time_ms: execution_time,
                    verified: true,
                },
                None => InferenceResult {
                    request_id: req.id,
                    output: vec![],
                    confidence: 0.0,
                    execution_time_ms: execution_time,
                    verified: false,
                },
            };

            let mut results = self.results.lock().unwrap();
            results.push(result.clone());

            if results.len() > 1000 {
                results.remove(0);
            }

            Some(result)
        } else {
            None
        }
    }

    pub fn process_batch(&self, batch_size: usize) -> Vec<InferenceResult> {
        let mut results = Vec::new();
        for _ in 0..batch_size {
            if let Some(result) = self.process_next() {
                results.push(result);
            } else {
                break;
            }
        }
        results
    }

    pub fn get_queue_size(&self) -> usize {
        let requests = self.requests.lock().unwrap();
        requests.len()
    }

    pub fn get_results(&self, limit: usize) -> Vec<InferenceResult> {
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
    fn test_submit_and_process() {
        let model_manager = Arc::new(ModelManager::new());
        let engine = InferenceEngine::new(model_manager);

        let id = engine.submit_request("default".to_string(), vec![1.0, 2.0], 5);
        assert!(!id.is_empty());
        assert_eq!(engine.get_queue_size(), 1);

        let result = engine.process_next();
        assert!(result.is_some());
        assert_eq!(engine.get_queue_size(), 0);
    }
}
