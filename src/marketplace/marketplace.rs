use crate::marketplace::matching::{MarketSummary, MatchResult, OrderMatcher};
use crate::marketplace::order::OrderBook;
use crate::marketplace::types::{MarketStats, Order, OrderStatus, OrderType};
use std::sync::{Arc, Mutex};

pub struct Marketplace {
    order_book: Arc<Mutex<OrderBook>>,
    stats: Arc<Mutex<MarketStats>>,
    match_history: Arc<Mutex<Vec<MatchResult>>>,
}

impl Marketplace {
    pub fn new() -> Self {
        Self {
            order_book: Arc::new(Mutex::new(OrderBook::new())),
            stats: Arc::new(Mutex::new(MarketStats::new())),
            match_history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// إنشاء أمر جديد
    pub fn create_order(
        &self,
        order_type: OrderType,
        miner_id: String,
        compute_units: u64,
        price_per_unit: u64,
        difficulty_level: u32,
    ) -> Order {
        let order = Order::new(
            order_type,
            miner_id,
            compute_units,
            price_per_unit,
            difficulty_level,
        );

        {
            let mut book = self.order_book.lock().unwrap();
            book.add_order(order.clone());
        }

        // تحديث stats بالطلب الجديد قبل المطابقة
        self.update_stats();

        println!(
            "Order created: {} {} units @ {} (diff={})",
            match order.order_type {
                OrderType::Buy => "BUY",
                OrderType::Sell => "SELL",
            },
            order.compute_units,
            order.price_per_unit,
            order.difficulty_level
        );

        // محاولة مطابقة فورية
        self.process_matches();
        order
    }

    /// إلغاء أمر
    pub fn cancel_order(&self, order_id: &str) -> Option<Order> {
        let cancelled = {
            let mut book = self.order_book.lock().unwrap();
            book.remove_order(order_id)
        };

        if let Some(mut order) = cancelled {
            order.cancel();
            {
                let mut stats = self.stats.lock().unwrap();
                stats.record_cancel();
            }
            self.update_stats();
            println!("Order cancelled: {}", order.id);
            Some(order)
        } else {
            None
        }
    }

    /// معالجة المطابقات
    /// رسوم المنصة 1%
    pub fn get_commission(&self, amount: u64) -> u64 {
        amount / 100
    }
    pub fn process_matches(&self) -> Vec<MatchResult> {
        let matches = {
            let mut book = self.order_book.lock().unwrap();
            OrderMatcher::match_orders(&mut book)
        };

        if !matches.is_empty() {
            let mut history = self.match_history.lock().unwrap();
            history.extend(matches.clone());
            self.record_matches(&matches);
            println!("Processed {} matches", matches.len());
        }

        self.update_stats();
        matches
    }

    /// الحصول على الطلبات المفتوحة
    pub fn get_open_orders(&self) -> Vec<Order> {
        let book = self.order_book.lock().unwrap();
        book.get_open_orders()
    }

    /// الحصول على جميع الطلبات
    pub fn get_all_orders(&self) -> Vec<Order> {
        let book = self.order_book.lock().unwrap();
        let mut orders = Vec::new();
        orders.extend(book.buy_orders.iter().cloned());
        orders.extend(book.sell_orders.iter().cloned());
        orders
    }

    /// سجل المطابقات
    pub fn get_match_history(&self) -> Vec<MatchResult> {
        self.match_history.lock().unwrap().clone()
    }

    /// ملخص السوق
    pub fn get_market_summary(&self) -> MarketSummary {
        let book = self.order_book.lock().unwrap();
        OrderMatcher::market_summary(&book)
    }

    /// أفضل سعر شراء
    pub fn best_bid(&self) -> Option<u64> {
        self.get_market_summary().best_bid
    }

    /// أفضل سعر بيع
    pub fn best_ask(&self) -> Option<u64> {
        self.get_market_summary().best_ask
    }

    /// إحصائيات السوق
    pub fn get_stats(&self) -> MarketStats {
        self.stats.lock().unwrap().clone()
    }

    /// تحديث الإحصائيات من الطلبات المفتوحة
    fn update_stats(&self) {
        let orders = self.get_open_orders();
        let mut stats = self.stats.lock().unwrap();
        stats.update_open(&orders);
    }

    /// تسجيل تنفيذ صفقات في الإحصائيات
    fn record_matches(&self, matches: &[MatchResult]) {
        let mut stats = self.stats.lock().unwrap();
        for m in matches {
            stats.record_fill(m.compute_units, m.price_per_unit, m.fee_amount);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_match() {
        let marketplace = Marketplace::new();

        let buy = marketplace.create_order(OrderType::Buy, "buyer1".into(), 100, 15, 3);
        assert!(!buy.id.is_empty());

        let sell = marketplace.create_order(OrderType::Sell, "seller1".into(), 100, 10, 3);
        assert!(!sell.id.is_empty());

        let history = marketplace.get_match_history();
        assert!(!history.is_empty());

        let stats = marketplace.get_stats();
        assert!(stats.filled_orders > 0);
        assert!(stats.total_volume > 0);
    }

    #[test]
    fn test_cancel_order() {
        let marketplace = Marketplace::new();
        let order = marketplace.create_order(OrderType::Buy, "buyer1".into(), 100, 15, 3);
        let cancelled = marketplace.cancel_order(&order.id);
        assert!(cancelled.is_some());
        assert_eq!(cancelled.unwrap().status, OrderStatus::Cancelled);
    }

    #[test]
    fn test_market_summary() {
        let marketplace = Marketplace::new();
        marketplace.create_order(OrderType::Buy, "b1".into(), 100, 20, 3);
        marketplace.create_order(OrderType::Sell, "s1".into(), 50, 25, 3);

        let summary = marketplace.get_market_summary();
        assert!(summary.best_bid.is_some());
        assert!(summary.best_ask.is_some());
        assert_eq!(summary.best_bid, Some(20));
        assert_eq!(summary.best_ask, Some(25));
    }

    #[test]
    fn test_stats_accumulate() {
        let marketplace = Marketplace::new();

        // صفقة 1
        marketplace.create_order(OrderType::Buy, "b1".into(), 100, 15, 3);
        marketplace.create_order(OrderType::Sell, "s1".into(), 100, 10, 3);

        // صفقة 2
        marketplace.create_order(OrderType::Buy, "b2".into(), 50, 20, 3);
        marketplace.create_order(OrderType::Sell, "s2".into(), 50, 12, 3);

        let stats = marketplace.get_stats();
        println!(
            "Stats: filled={}, volume={}, fees={}",
            stats.filled_orders, stats.total_volume, stats.total_fees
        );
        assert!(
            stats.filled_orders >= 2,
            "Should have at least 2 filled orders"
        );
        assert!(stats.total_volume > 0, "Volume should be > 0");
    }
}
