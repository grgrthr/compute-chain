use crate::economic::fees::FeeManager;
use crate::vm::instruction::Instruction;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartContract {
    pub id: String,
    pub owner: String,
    pub code: Vec<Instruction>,
    pub balance: u64,
    pub storage: HashMap<String, u64>,
    pub created_at: u64,
    pub call_count: u64,
    pub total_gas_used: u64,
    pub total_fees_paid: u64,
}

pub struct ContractStorage {
    contracts: Arc<Mutex<HashMap<String, SmartContract>>>,
    fee_manager: Arc<FeeManager>,
    pub total_gas_used: Arc<Mutex<u64>>,
    pub total_fees_collected: Arc<Mutex<u64>>,
}

impl ContractStorage {
    pub fn new() -> Self {
        Self {
            contracts: Arc::new(Mutex::new(HashMap::new())),
            fee_manager: Arc::new(FeeManager::new()),
            total_gas_used: Arc::new(Mutex::new(0)),
            total_fees_collected: Arc::new(Mutex::new(0)),
        }
    }

    /// تحميل العقود من القرص
    pub fn load_from_disk(path: &str) -> Result<Self, String> {
        let file = format!("{}/contracts/contracts.json", path);

        if !std::path::Path::new(&file).exists() {
            return Ok(Self::new());
        }

        let json = std::fs::read_to_string(&file).map_err(|e| e.to_string())?;
        let data: Vec<serde_json::Value> =
            serde_json::from_str(&json).map_err(|e| e.to_string())?;

        let storage = Self::new();
        println!("Contracts loaded: {} contracts (metadata only)", data.len());

        Ok(storage)
    }

    /// نشر عقد جديد
    pub fn deploy(&self, owner: &str, code: Vec<Instruction>) -> String {
        let id = format!("0x{}", hex::encode(&uuid::Uuid::new_v4().as_bytes()[..8]));

        let contract = SmartContract {
            id: id.clone(),
            owner: owner.to_string(),
            code,
            balance: 0,
            storage: HashMap::new(),
            created_at: Self::current_time(),
            call_count: 0,
            total_gas_used: 0,
            total_fees_paid: 0,
        };

        self.contracts.lock().unwrap().insert(id.clone(), contract);
        println!("Contract deployed: {}", id);

        let _ = self.save_to_disk("./chain_data");

        id
    }

    /// استرجاع عقد
    pub fn get(&self, id: &str) -> Option<SmartContract> {
        self.contracts.lock().unwrap().get(id).cloned()
    }

    /// استدعاء عقد مع Gas Metering
    pub fn call(&self, id: &str, args: &[u64], gas_limit: u64) -> Result<CallResult, String> {
        let mut contracts = self.contracts.lock().unwrap();
        let contract = contracts.get_mut(id).ok_or("Contract not found")?;

        let mut cpu = crate::vm::cpu::Cpu::new();
        cpu.calldata = args.to_vec();

        let mut memory = crate::vm::memory::Memory::new(65536);
        let program = crate::vm::program::Program::new(contract.code.clone());
        let mut step_counter: u64 = 0;
        let max_steps = gas_limit.min(10000);

        let mut gas_used: u64 = 0;

        while !cpu.halted && step_counter < max_steps {
            let pc = cpu.pc;
            if pc >= program.instructions.len() {
                cpu.halted = true;
                break;
            }

            let inst = &program.instructions[pc];
            gas_used += Self::gas_for_instruction(inst);

            if gas_used > gas_limit {
                return Err(format!(
                    "Out of gas: used={}, limit={}",
                    gas_used, gas_limit
                ));
            }

            let result = crate::vm::executor::Executor::step(&mut cpu, &mut memory, &program);
            if result.is_none() {
                break;
            }
            step_counter += 1;
        }

        contract.call_count += 1;
        contract.total_gas_used += gas_used;
        contract.total_fees_paid += gas_used;

        let mut total_gas = self.total_gas_used.lock().unwrap();
        *total_gas += gas_used;

        let mut total_fees = self.total_fees_collected.lock().unwrap();
        *total_fees += gas_used;

        Ok(CallResult {
            registers: cpu.registers.to_vec(),
            gas_used,
            gas_limit,
            steps: step_counter,
            remaining_gas: gas_limit.saturating_sub(gas_used),
        })
    }

    /// تكلفة Gas لكل تعليمة
    fn gas_for_instruction(inst: &Instruction) -> u64 {
        match inst {
            Instruction::Mov { .. } => 1,
            Instruction::Add { .. } => 2,
            Instruction::Sub { .. } => 2,
            Instruction::Mul { .. } => 3,
            Instruction::Div { .. } => 5,
            Instruction::Cmp { .. } => 2,
            Instruction::Jump { .. } => 1,
            Instruction::Load { .. } => 3,
            Instruction::Store { .. } => 5,
            Instruction::Call { .. } => 10,
            Instruction::Ret => 2,
            Instruction::Push { .. } => 2,
            Instruction::Pop { .. } => 2,
            Instruction::CallData { .. } => 3,
            Instruction::Log { .. } => 5,
            Instruction::Sha256 { .. } => 20,
            Instruction::Halt => 0,
            _ => 1,
        }
    }

    /// تقدير Gas لبرنامج كامل
    pub fn estimate_gas(&self, code: &[Instruction]) -> u64 {
        let mut total: u64 = 0;
        for inst in code {
            total += Self::gas_for_instruction(inst);
        }
        total * 2 // هامش أمان 2x
    }

    /// حساب Fee من Gas
    pub fn gas_to_fee(&self, gas: u64) -> u64 {
        let estimate = self.fee_manager.calculate_fee(gas, 0, 0);
        estimate.total
    }

    /// إحصائيات Gas الشبكة
    pub fn gas_stats(&self) -> GasStats {
        GasStats {
            total_gas_used: *self.total_gas_used.lock().unwrap(),
            total_fees_collected: *self.total_fees_collected.lock().unwrap(),
            contract_count: self.contracts.lock().unwrap().len() as u64,
        }
    }

    /// إيداع في العقد
    pub fn deposit(&self, id: &str, amount: u64) -> Result<(), String> {
        let mut contracts = self.contracts.lock().unwrap();
        let contract = contracts.get_mut(id).ok_or("Contract not found")?;
        contract.balance += amount;
        Ok(())
    }

    /// سحب من العقد (للمالك فقط)
    pub fn withdraw(&self, id: &str, owner: &str, amount: u64) -> Result<(), String> {
        let mut contracts = self.contracts.lock().unwrap();
        let contract = contracts.get_mut(id).ok_or("Contract not found")?;

        if contract.owner != owner {
            return Err("Only owner can withdraw".into());
        }

        if contract.balance < amount {
            return Err("Insufficient contract balance".into());
        }

        contract.balance -= amount;
        Ok(())
    }

    /// تخزين قيمة في العقد
    pub fn store_value(&self, id: &str, key: &str, value: u64) -> Result<(), String> {
        let mut contracts = self.contracts.lock().unwrap();
        let contract = contracts.get_mut(id).ok_or("Contract not found")?;
        contract.storage.insert(key.into(), value);
        Ok(())
    }

    /// قراءة قيمة من العقد
    pub fn get_value(&self, id: &str, key: &str) -> Option<u64> {
        let contracts = self.contracts.lock().unwrap();
        contracts.get(id)?.storage.get(key).copied()
    }

    /// حفظ العقود على القرص
    pub fn save_to_disk(&self, path: &str) -> Result<(), String> {
        let contracts = self.contracts.lock().unwrap();
        let dir = format!("{}/contracts", path);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

        let data: Vec<serde_json::Value> = contracts
            .values()
            .map(|c| {
                serde_json::json!({
                    "id": c.id,
                    "owner": c.owner,
                    "balance": c.balance,
                    "storage": c.storage,
                    "created_at": c.created_at,
                    "call_count": c.call_count,
                    "total_gas_used": c.total_gas_used,
                    "total_fees_paid": c.total_fees_paid,
                    "code_length": c.code.len(),
                })
            })
            .collect();

        let json = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
        std::fs::write(format!("{}/contracts.json", dir), json).map_err(|e| e.to_string())?;

        println!("Contracts saved: {} contracts", contracts.len());
        Ok(())
    }

    fn current_time() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

// ============================================================
// Call Result
// ============================================================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallResult {
    pub registers: Vec<u64>,
    pub gas_used: u64,
    pub gas_limit: u64,
    pub steps: u64,
    pub remaining_gas: u64,
}

// ============================================================
// Gas Stats
// ============================================================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasStats {
    pub total_gas_used: u64,
    pub total_fees_collected: u64,
    pub contract_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gas_estimation() {
        let storage = ContractStorage::new();
        let code = vec![
            Instruction::Mov {
                register: 0,
                value: 10,
            },
            Instruction::Add {
                destination: 1,
                source: 0,
            },
            Instruction::Mul {
                destination: 2,
                source: 1,
            },
        ];
        let gas = storage.estimate_gas(&code);
        assert!(gas > 0);
    }

    #[test]
    fn test_gas_limit_exceeded() {
        let storage = ContractStorage::new();
        let code = vec![Instruction::Sha256 {
            source: 0,
            destination: 1,
        }];
        let id = storage.deploy("test", code);

        let result = storage.call(&id, &[], 10);
        assert!(result.is_err());
    }
}

// Strategy: Extract Interface for contract (Storage)
// Review and adjust before applying.
