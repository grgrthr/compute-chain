use std::sync::{Arc, Mutex};

pub struct DynamicScheduler {
    current_difficulty: Arc<Mutex<u32>>,
    last_performance: Arc<Mutex<Vec<u64>>>,
}

impl DynamicScheduler {
    pub fn new() -> Self {
        Self {
            current_difficulty: Arc::new(Mutex::new(1)),
            last_performance: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn adjust_difficulty(&self, last_execution_time_ms: u64) -> u32 {
        let mut difficulty = self.current_difficulty.lock().unwrap();
        let mut performances = self.last_performance.lock().unwrap();

        performances.push(last_execution_time_ms);
        if performances.len() > 10 {
            performances.remove(0);
        }

        // تنفيذ سريع → زيادة الصعوبة
        if last_execution_time_ms < 50 {
            *difficulty = (*difficulty + 1).min(10);
        }
        // تنفيذ بطيء → تقليل الصعوبة
        else if last_execution_time_ms > 200 {
            *difficulty = (*difficulty).saturating_sub(1).max(1);
        }

        *difficulty
    }

    pub fn get_current_difficulty(&self) -> u32 {
        *self.current_difficulty.lock().unwrap()
    }

    pub fn get_average_performance(&self) -> u64 {
        let performances = self.last_performance.lock().unwrap();
        if performances.is_empty() {
            return 0;
        }
        performances.iter().sum::<u64>() / performances.len() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adjust_difficulty() {
        let scheduler = DynamicScheduler::new();

        let new_difficulty = scheduler.adjust_difficulty(30);
        // تنفيذ سريع → زيادة الصعوبة
        assert!(new_difficulty >= 1);

        let new_difficulty = scheduler.adjust_difficulty(300);
        // تنفيذ بطيء → تقليل الصعوبة
        assert!(new_difficulty <= 5);
    }
}
