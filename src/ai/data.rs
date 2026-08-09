use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dataset {
    pub id: String,
    pub name: String,
    pub data: Vec<DataPoint>,
    pub labels: Vec<String>,
    pub size: usize,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    pub features: Vec<f64>,
    pub label: Option<f64>,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataStats {
    pub mean: Vec<f64>,
    pub std: Vec<f64>,
    pub min: Vec<f64>,
    pub max: Vec<f64>,
    pub total_points: usize,
}

pub struct DataProcessor {
    datasets: Arc<Mutex<HashMap<String, Dataset>>>,
}

impl DataProcessor {
    pub fn new() -> Self {
        Self {
            datasets: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn create_dataset(&self, name: String, data: Vec<Vec<f64>>, labels: Vec<String>) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let data_points: Vec<DataPoint> = data
            .into_iter()
            .map(|features| DataPoint {
                features,
                label: None,
                weight: 1.0,
            })
            .collect();

        let size = data_points.len();

        let dataset = Dataset {
            id: id.clone(),
            name,
            data: data_points,
            labels,
            size,
            created_at: Self::current_time(),
        };

        let mut datasets = self.datasets.lock().unwrap();
        datasets.insert(id.clone(), dataset);

        id
    }

    pub fn get_dataset(&self, id: &str) -> Option<Dataset> {
        let datasets = self.datasets.lock().unwrap();
        datasets.get(id).cloned()
    }

    pub fn normalize(&self, dataset_id: &str) -> Result<(), String> {
        let mut datasets = self.datasets.lock().unwrap();
        let dataset = datasets.get_mut(dataset_id).ok_or("Dataset not found")?;

        let stats = self.compute_stats(dataset);

        for point in &mut dataset.data {
            for i in 0..point.features.len() {
                if stats.std[i] > 0.0 {
                    point.features[i] = (point.features[i] - stats.mean[i]) / stats.std[i];
                }
            }
        }

        Ok(())
    }

    pub fn compute_stats(&self, dataset: &Dataset) -> DataStats {
        if dataset.data.is_empty() {
            return DataStats {
                mean: vec![],
                std: vec![],
                min: vec![],
                max: vec![],
                total_points: 0,
            };
        }

        let dim = dataset.data[0].features.len();
        let mut sum = vec![0.0; dim];
        let mut sum_sq = vec![0.0; dim];
        let mut min = vec![f64::MAX; dim];
        let mut max = vec![f64::MIN; dim];

        for point in &dataset.data {
            for i in 0..dim {
                sum[i] += point.features[i];
                sum_sq[i] += point.features[i] * point.features[i];
                min[i] = min[i].min(point.features[i]);
                max[i] = max[i].max(point.features[i]);
            }
        }

        let n = dataset.data.len() as f64;
        let mean: Vec<f64> = sum.iter().map(|s| s / n).collect();
        let std: Vec<f64> = sum_sq
            .iter()
            .enumerate()
            .map(|(i, ss)| ((ss / n) - (mean[i] * mean[i])).sqrt())
            .collect();

        DataStats {
            mean,
            std,
            min,
            max,
            total_points: dataset.data.len(),
        }
    }

    pub fn split_dataset(
        &self,
        dataset_id: &str,
        train_ratio: f64,
    ) -> Result<(Vec<DataPoint>, Vec<DataPoint>), String> {
        let dataset = self.get_dataset(dataset_id).ok_or("Dataset not found")?;

        let split_point = (dataset.data.len() as f64 * train_ratio) as usize;
        let train = dataset.data[..split_point].to_vec();
        let test = dataset.data[split_point..].to_vec();

        Ok((train, test))
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
    fn test_create_dataset() {
        let processor = DataProcessor::new();
        let data = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let id = processor.create_dataset("test".to_string(), data, vec!["label1".to_string()]);

        let dataset = processor.get_dataset(&id);
        assert!(dataset.is_some());
        assert_eq!(dataset.unwrap().size, 2);
    }

    #[test]
    fn test_normalize() {
        let processor = DataProcessor::new();
        let data = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let id = processor.create_dataset("test".to_string(), data, vec![]);

        processor.normalize(&id).unwrap();
        let dataset = processor.get_dataset(&id).unwrap();

        let stats = processor.compute_stats(&dataset);
        assert!(stats.mean[0].abs() < 0.0001);
    }
}
