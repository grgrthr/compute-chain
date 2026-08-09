use sha2::{Digest, Sha256};

pub struct Memory {
    pub data: Vec<u8>,
}

impl Memory {
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![0; size],
        }
    }

    pub fn read_u64(&self, address: usize) -> u64 {
        let bytes = &self.data[address..address + 8];
        u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])
    }

    pub fn write_u64(&mut self, address: usize, value: u64) {
        let bytes = value.to_le_bytes();
        self.data[address..address + 8].copy_from_slice(&bytes);
    }

    /// حساب هاش لحالة الذاكرة الكاملة
    pub fn hash_state(&self) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(&self.data);
        hasher.finalize().to_vec()
    }

    /// هاش مبسط للذاكرة المستخدمة فقط
    pub fn hash_used(&self, used_addresses: &[(usize, usize)]) -> Vec<u8> {
        let mut hasher = Sha256::new();
        for &(addr, len) in used_addresses {
            if addr + len <= self.data.len() {
                hasher.update(&self.data[addr..addr + len]);
            }
        }
        hasher.finalize().to_vec()
    }
}

// Strategy: Move Dependencies Down for vm (Infrastructure)
// Review and adjust before applying.
