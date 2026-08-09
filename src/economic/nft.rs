use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NFT {
    pub id: String,
    pub name: String,
    pub owner: String,
    pub data: String,
    pub created_at: u64,
    pub transfer_count: u64,
}

pub struct NFTEngine {
    pub nfts: Arc<Mutex<HashMap<String, NFT>>>,
    owner_nfts: Arc<Mutex<HashMap<String, Vec<String>>>>,
}

impl NFTEngine {
    pub fn load_from_disk(path: &str) -> Result<Self, String> {
        let file = format!("{}/nfts/nfts.json", path);
        if !std::path::Path::new(&file).exists() {
            return Ok(Self::new());
        }
        let json = std::fs::read_to_string(&file).map_err(|e| e.to_string())?;
        let nfts_vec: Vec<NFT> = serde_json::from_str(&json).map_err(|e| e.to_string())?;
        let nfts_map: HashMap<String, NFT> =
            nfts_vec.into_iter().map(|n| (n.id.clone(), n)).collect();
        let mut owner_map: HashMap<String, Vec<String>> = HashMap::new();
        for nft in nfts_map.values() {
            owner_map
                .entry(nft.owner.clone())
                .or_insert(Vec::new())
                .push(nft.id.clone());
        }
        println!("💾 NFTs loaded: {}", nfts_map.len());
        Ok(Self {
            nfts: Arc::new(Mutex::new(nfts_map)),
            owner_nfts: Arc::new(Mutex::new(owner_map)),
        })
    }

    pub fn new() -> Self {
        Self {
            nfts: Arc::new(Mutex::new(HashMap::new())),
            owner_nfts: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// إنشاء NFT جديد
    pub fn mint(&self, owner: &str, name: &str, data: &str) -> String {
        let id = format!(
            "nft_{}",
            uuid::Uuid::new_v4()
                .to_string()
                .split('-')
                .next()
                .unwrap_or("0")
        );

        let nft = NFT {
            id: id.clone(),
            name: name.to_string(),
            owner: owner.to_string(),
            data: data.to_string(),
            created_at: Self::current_time(),
            transfer_count: 0,
        };

        self.nfts.lock().unwrap().insert(id.clone(), nft);
        self.owner_nfts
            .lock()
            .unwrap()
            .entry(owner.to_string())
            .or_insert(Vec::new())
            .push(id.clone());

        println!("🎨 NFT minted: {} -> {}", id, owner);
        id
    }

    /// سك NFT مع royalty
    pub fn mint_with_royalty(
        &self,
        owner: &str,
        name: &str,
        data: &str,
        royalty_percent: u8,
    ) -> String {
        let id = self.mint(owner, name, data);
        println!("🎨 NFT minted with {}% royalty: {}", royalty_percent, id);
        id
    }

    /// نقل NFT
    pub fn transfer(&self, id: &str, from: &str, to: &str) -> Result<(), String> {
        let mut nfts = self.nfts.lock().unwrap();
        let nft = nfts.get_mut(id).ok_or("NFT not found")?;

        if nft.owner != from {
            return Err("Sender is not the owner".into());
        }

        nft.owner = to.to_string();
        nft.transfer_count += 1;

        let mut owner_nfts = self.owner_nfts.lock().unwrap();
        if let Some(list) = owner_nfts.get_mut(from) {
            list.retain(|x| x != id);
        }
        owner_nfts
            .entry(to.to_string())
            .or_insert(Vec::new())
            .push(id.to_string());

        println!("🎨 NFT transferred: {} -> {}", id, to);
        Ok(())
    }

    /// NFT Marketplace: شراء NFT
    pub fn buy_nft(&self, id: &str, from: &str, to: &str, price: u64) -> Result<(), String> {
        let nft = self.get(id).ok_or("NFT not found")?;
        if nft.owner != from {
            return Err("Seller is not the owner".into());
        }
        println!("💰 NFT sold: {} from {} to {} for {}", id, from, to, price);
        self.transfer(id, from, to)
    }

    /// استرجاع NFT واحد
    pub fn get(&self, id: &str) -> Option<NFT> {
        self.nfts.lock().unwrap().get(id).cloned()
    }

    /// سرد جميع NFTs
    pub fn list_all(&self) -> Vec<NFT> {
        self.nfts.lock().unwrap().values().cloned().collect()
    }

    /// NFTs لمستخدم محدد
    pub fn get_by_owner(&self, owner: &str) -> Vec<NFT> {
        let owner_nfts = self.owner_nfts.lock().unwrap();
        let nfts = self.nfts.lock().unwrap();
        owner_nfts
            .get(owner)
            .map(|ids| ids.iter().filter_map(|id| nfts.get(id).cloned()).collect())
            .unwrap_or_default()
    }

    /// إجمالي عدد NFTs
    pub fn total_count(&self) -> usize {
        self.nfts.lock().unwrap().len()
    }

    /// حفظ على القرص
    pub fn save_to_disk(&self, path: &str) -> Result<(), String> {
        let nfts = self.nfts.lock().unwrap();
        let dir = format!("{}/nfts", path);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

        let data: Vec<serde_json::Value> = nfts
            .values()
            .map(|n| {
                serde_json::json!({
                    "id": n.id, "name": n.name, "owner": n.owner,
                    "data": n.data, "created_at": n.created_at,
                    "transfer_count": n.transfer_count,
                })
            })
            .collect();

        let json = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
        std::fs::write(format!("{}/nfts.json", dir), json).map_err(|e| e.to_string())?;

        println!("💾 NFTs saved: {}", nfts.len());
        Ok(())
    }

    fn current_time() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

// Strategy: Dependency Inversion for economic (Core)
// Review and adjust before applying.
