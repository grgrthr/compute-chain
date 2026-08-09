use crate::consensus::types::Validator;
use rand::Rng;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub struct ValidatorSet {
    validators: Arc<Mutex<BTreeMap<String, Validator>>>,
    total_stake: Arc<Mutex<u64>>,
}

impl ValidatorSet {
    pub fn new() -> Self {
        Self {
            validators: Arc::new(Mutex::new(BTreeMap::new())),
            total_stake: Arc::new(Mutex::new(0)),
        }
    }

    pub fn register_validator(
        &self,
        id: String,
        address: String,
        stake: u64,
        commission: f64,
    ) -> String {
        let mut validators = self.validators.lock().unwrap();
        let validator = Validator::new(id.clone(), address, stake, commission);
        validators.insert(id.clone(), validator);

        let mut total_stake = self.total_stake.lock().unwrap();
        *total_stake += stake;

        id
    }

    pub fn get_validator(&self, id: &str) -> Option<Validator> {
        let validators = self.validators.lock().unwrap();
        validators.get(id).cloned()
    }

    pub fn list_validators(&self) -> Vec<Validator> {
        let validators = self.validators.lock().unwrap();
        validators.values().cloned().collect()
    }

    pub fn get_active_validators(&self) -> Vec<Validator> {
        let validators = self.validators.lock().unwrap();
        validators.values().filter(|v| v.active).cloned().collect()
    }

    pub fn select_validator(&self) -> Option<Validator> {
        let validators = self.get_active_validators();
        if validators.is_empty() {
            return None;
        }

        let total_stake: u64 = validators.iter().map(|v| v.stake).sum();
        let mut rng = rand::thread_rng();
        let mut target = rng.gen_range(0..total_stake);

        for validator in &validators {
            if target < validator.stake {
                return Some(validator.clone());
            }
            target -= validator.stake;
        }

        validators.first().cloned()
    }

    pub fn update_stake(&self, id: &str, new_stake: u64) {
        let mut validators = self.validators.lock().unwrap();
        if let Some(validator) = validators.get_mut(id) {
            let mut total_stake = self.total_stake.lock().unwrap();
            *total_stake = total_stake.saturating_sub(validator.stake);
            validator.stake = new_stake;
            *total_stake += new_stake;
        }
    }

    pub fn get_total_stake(&self) -> u64 {
        *self.total_stake.lock().unwrap()
    }

    pub fn deactivate_validator(&self, id: &str) {
        let mut validators = self.validators.lock().unwrap();
        if let Some(validator) = validators.get_mut(id) {
            validator.active = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_validator() {
        let set = ValidatorSet::new();
        let id = set.register_validator("validator1".to_string(), "0x123".to_string(), 1000, 0.05);
        assert_eq!(id, "validator1");

        let validator = set.get_validator("validator1").unwrap();
        assert_eq!(validator.stake, 1000);
    }

    #[test]
    fn test_select_validator() {
        let set = ValidatorSet::new();
        set.register_validator("v1".to_string(), "0x1".to_string(), 1000, 0.05);
        set.register_validator("v2".to_string(), "0x2".to_string(), 2000, 0.05);

        let selected = set.select_validator();
        assert!(selected.is_some());
    }
}

// Strategy: Dependency Inversion for consensus (Core)
// Review and adjust before applying.
