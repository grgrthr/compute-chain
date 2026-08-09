use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OrderType {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OrderStatus {
    Open,
    Filled,
    Cancelled,
    Executing,
    PartialFilled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub order_type: OrderType,
    pub miner_id: String,
    pub compute_units: u64,
    pub price_per_unit: u64,
    pub difficulty_level: u32,
    pub status: OrderStatus,
    pub created_at: u64,
    pub filled_at: Option<u64>,
    pub original_units: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketStats {
    pub total_orders: u64,
    pub open_orders: u64,
    pub filled_orders: u64,
    pub cancelled_orders: u64,
    pub total_compute_units: u64,
    pub avg_price: f64,
    pub total_volume: u64,
    pub total_fees: u64,
}

impl Order {
    pub fn new(
        order_type: OrderType,
        miner_id: String,
        compute_units: u64,
        price_per_unit: u64,
        difficulty_level: u32,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            order_type,
            miner_id,
            compute_units,
            price_per_unit,
            difficulty_level,
            status: OrderStatus::Open,
            created_at: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            filled_at: None,
            original_units: compute_units,
        }
    }

    pub fn fill(&mut self) {
        self.status = OrderStatus::Filled;
        self.filled_at = Some(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        );
    }

    pub fn cancel(&mut self) {
        self.status = OrderStatus::Cancelled;
    }

    pub fn fill_percentage(&self) -> f64 {
        if self.original_units == 0 {
            return 100.0;
        }
        let filled = self.original_units - self.compute_units;
        (filled as f64 / self.original_units as f64) * 100.0
    }
}

impl MarketStats {
    pub fn new() -> Self {
        Self {
            total_orders: 0,
            open_orders: 0,
            filled_orders: 0,
            cancelled_orders: 0,
            total_compute_units: 0,
            avg_price: 0.0,
            total_volume: 0,
            total_fees: 0,
        }
    }

    /// تحديث من الطلبات المفتوحة
    pub fn update_open(&mut self, orders: &[Order]) {
        self.open_orders = orders
            .iter()
            .filter(|o| o.status == OrderStatus::Open)
            .count() as u64;
        self.total_orders = self.filled_orders + self.cancelled_orders + self.open_orders;
    }

    /// تسجيل تنفيذ صفقة
    pub fn record_fill(&mut self, units: u64, price: u64, fee: u64) {
        self.filled_orders += 1;
        self.total_compute_units += units;
        self.total_volume += units * price;
        self.total_fees += fee;
        if self.total_compute_units > 0 {
            self.avg_price = self.total_volume as f64 / self.total_compute_units as f64;
        }
    }

    /// تسجيل إلغاء
    pub fn record_cancel(&mut self) {
        self.cancelled_orders += 1;
    }
}
