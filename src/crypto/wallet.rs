use crate::crypto::signer;
use ed25519_dalek::{SigningKey, VerifyingKey};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Simple wallet that holds keypairs for accounts
pub struct Wallet {
    keys: Arc<Mutex<HashMap<String, SigningKey>>>,
}

impl Wallet {
    pub fn new() -> Self {
        Self {
            keys: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a new account and return its public key
    pub fn create_account(&self, name: &str) -> String {
        let (sk, vk) = signer::generate_keypair();
        let public_key = signer::public_key_to_hex(&vk);
        self.keys.lock().unwrap().insert(name.to_string(), sk);
        tracing::info!(
            "🔑 Created account '{}' with public key: {}",
            name,
            &public_key[..16]
        );
        public_key
    }

    /// Sign a transaction for the given account
    pub fn sign(&self, account: &str, message: &[u8]) -> Option<String> {
        let keys = self.keys.lock().unwrap();
        keys.get(account).map(|sk| signer::sign(sk, message))
    }

    /// Get public key for an account (if it exists)
    pub fn get_public_key(&self, account: &str) -> Option<String> {
        let keys = self.keys.lock().unwrap();
        keys.get(account).map(|sk| {
            let vk = sk.verifying_key();
            signer::public_key_to_hex(&vk)
        })
    }
}
