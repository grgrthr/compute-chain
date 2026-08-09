use crate::marketplace::order::OrderBook;
use crate::marketplace::types::{Order, OrderStatus, OrderType};
use std::collections::HashMap;

pub struct OrderMatcher;

#[derive(Debug, Clone)]
pub struct MatchResult {
    pub buy_order_id: String,
    pub sell_order_id: String,
    pub buyer_id: String,
    pub seller_id: String,
    pub compute_units: u64,
    pub price_per_unit: u64,
    pub total_value: u64,
    pub fee_amount: u64,
    pub difficulty_level: u32,
}

impl OrderMatcher {
    /// المطابقة المتقدمة مع دعم المطابقة الجزئية والرسوم
    pub fn match_orders(book: &mut OrderBook) -> Vec<MatchResult> {
        let mut matches = Vec::new();
        let mut filled_ids: Vec<String> = Vec::new();

        // جمع معرفات أوامر الشراء والبيع المفتوحة
        let buy_indices: Vec<usize> = book
            .buy_orders
            .iter()
            .enumerate()
            .filter(|(_, o)| o.status == OrderStatus::Open && o.compute_units > 0)
            .map(|(i, _)| i)
            .collect();

        let sell_indices: Vec<usize> = book
            .sell_orders
            .iter()
            .enumerate()
            .filter(|(_, o)| o.status == OrderStatus::Open && o.compute_units > 0)
            .map(|(i, _)| i)
            .collect();

        for &bi in &buy_indices {
            if book.buy_orders[bi].compute_units == 0 {
                filled_ids.push(book.buy_orders[bi].id.clone());
                continue;
            }

            for &si in &sell_indices {
                if book.sell_orders[si].compute_units == 0 {
                    continue;
                }

                let can_match = {
                    let buy = &book.buy_orders[bi];
                    let sell = &book.sell_orders[si];
                    sell.price_per_unit <= buy.price_per_unit
                        && sell.difficulty_level == buy.difficulty_level
                        && buy.compute_units > 0
                        && sell.compute_units > 0
                };

                if can_match {
                    let buy_units = book.buy_orders[bi].compute_units;
                    let sell_units = book.sell_orders[si].compute_units;
                    let matched_units = sell_units.min(buy_units);

                    let buy_price = book.buy_orders[bi].price_per_unit;
                    let sell_price = book.sell_orders[si].price_per_unit;
                    let execution_price = (buy_price + sell_price) / 2;
                    let total_value = matched_units * execution_price;
                    let fee = total_value / 100;

                    let buy_id = book.buy_orders[bi].id.clone();
                    let sell_id = book.sell_orders[si].id.clone();
                    let buyer_id = book.buy_orders[bi].miner_id.clone();
                    let seller_id = book.sell_orders[si].miner_id.clone();
                    let diff = book.buy_orders[bi].difficulty_level;

                    matches.push(MatchResult {
                        buy_order_id: buy_id,
                        sell_order_id: sell_id,
                        buyer_id,
                        seller_id,
                        compute_units: matched_units,
                        price_per_unit: execution_price,
                        total_value,
                        fee_amount: fee,
                        difficulty_level: diff,
                    });

                    book.buy_orders[bi].compute_units -= matched_units;
                    book.sell_orders[si].compute_units -= matched_units;

                    book.buy_orders[bi].status = OrderStatus::Executing;
                    book.sell_orders[si].status = OrderStatus::Executing;

                    if book.buy_orders[bi].compute_units == 0 {
                        filled_ids.push(book.buy_orders[bi].id.clone());
                        book.buy_orders[bi].fill();
                    }
                    if book.sell_orders[si].compute_units == 0 {
                        filled_ids.push(book.sell_orders[si].id.clone());
                        book.sell_orders[si].fill();
                    }
                }
            }
        }

        // إزالة الطلبات المنفذة بالكامل
        for id in &filled_ids {
            book.buy_orders
                .retain(|o| o.id != *id || o.compute_units > 0);
            book.sell_orders
                .retain(|o| o.id != *id || o.compute_units > 0);
        }

        if !matches.is_empty() {
            println!(
                "Matched {} orders ({} filled)",
                matches.len(),
                filled_ids.len()
            );
        }

        matches
    }

    /// مطابقة سريعة: FIFO
    pub fn match_fifo(book: &mut OrderBook) -> Vec<MatchResult> {
        let mut matches = Vec::new();
        let mut buy_to_remove = Vec::new();
        let mut sell_to_remove = Vec::new();

        for (bi, buy_order) in book.buy_orders.iter_mut().enumerate() {
            if buy_order.status != OrderStatus::Open || buy_order.compute_units == 0 {
                continue;
            }

            for (si, sell_order) in book.sell_orders.iter_mut().enumerate() {
                if sell_order.status != OrderStatus::Open || sell_order.compute_units == 0 {
                    continue;
                }

                if sell_order.price_per_unit <= buy_order.price_per_unit {
                    let matched_units = sell_order.compute_units.min(buy_order.compute_units);
                    let price = buy_order.price_per_unit;
                    let value = matched_units * price;
                    let fee = value / 100;

                    matches.push(MatchResult {
                        buy_order_id: buy_order.id.clone(),
                        sell_order_id: sell_order.id.clone(),
                        buyer_id: buy_order.miner_id.clone(),
                        seller_id: sell_order.miner_id.clone(),
                        compute_units: matched_units,
                        price_per_unit: price,
                        total_value: value,
                        fee_amount: fee,
                        difficulty_level: buy_order.difficulty_level,
                    });

                    buy_order.compute_units -= matched_units;
                    sell_order.compute_units -= matched_units;

                    if sell_order.compute_units == 0 {
                        sell_order.fill();
                        sell_to_remove.push(si);
                    }
                    if buy_order.compute_units == 0 {
                        buy_order.fill();
                        buy_to_remove.push(bi);
                    }
                    break;
                }
            }
        }

        // إزالة من الخلف للأمام
        for &idx in buy_to_remove.iter().rev() {
            if idx < book.buy_orders.len() {
                book.buy_orders.remove(idx);
            }
        }
        for &idx in sell_to_remove.iter().rev() {
            if idx < book.sell_orders.len() {
                book.sell_orders.remove(idx);
            }
        }

        matches
    }

    /// حساب ملخص السوق
    pub fn market_summary(book: &OrderBook) -> MarketSummary {
        let buy_count = book
            .buy_orders
            .iter()
            .filter(|o| o.status == OrderStatus::Open)
            .count();
        let sell_count = book
            .sell_orders
            .iter()
            .filter(|o| o.status == OrderStatus::Open)
            .count();

        let buy_volume: u64 = book
            .buy_orders
            .iter()
            .filter(|o| o.status == OrderStatus::Open)
            .map(|o| o.compute_units * o.price_per_unit)
            .sum();

        let sell_volume: u64 = book
            .sell_orders
            .iter()
            .filter(|o| o.status == OrderStatus::Open)
            .map(|o| o.compute_units * o.price_per_unit)
            .sum();

        let best_bid = book
            .buy_orders
            .iter()
            .filter(|o| o.status == OrderStatus::Open)
            .map(|o| o.price_per_unit)
            .max();

        let best_ask = book
            .sell_orders
            .iter()
            .filter(|o| o.status == OrderStatus::Open)
            .map(|o| o.price_per_unit)
            .min();

        let spread = match (best_bid, best_ask) {
            (Some(bid), Some(ask)) if bid >= ask => Some(bid - ask),
            _ => None,
        };

        MarketSummary {
            open_buy_orders: buy_count as u64,
            open_sell_orders: sell_count as u64,
            buy_volume,
            sell_volume,
            best_bid,
            best_ask,
            spread,
        }
    }

    pub fn calculate_value(compute_units: u64, price_per_unit: u64) -> u64 {
        compute_units * price_per_unit
    }

    pub fn estimate_execution_time(compute_units: u64, difficulty: u32) -> u64 {
        let base_time = 100;
        base_time * compute_units * difficulty as u64 / 10
    }
}

#[derive(Debug, Clone)]
pub struct MarketSummary {
    pub open_buy_orders: u64,
    pub open_sell_orders: u64,
    pub buy_volume: u64,
    pub sell_volume: u64,
    pub best_bid: Option<u64>,
    pub best_ask: Option<u64>,
    pub spread: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marketplace::order::OrderBook;
    use crate::marketplace::types::OrderType;

    #[test]
    fn test_match_orders_full() {
        let mut book = OrderBook::new();
        let buy = Order::new(OrderType::Buy, "buyer1".into(), 100, 15, 3);
        let sell = Order::new(OrderType::Sell, "seller1".into(), 100, 10, 3);
        book.add_order(buy);
        book.add_order(sell);

        let matches = OrderMatcher::match_orders(&mut book);
        assert!(!matches.is_empty());
        assert_eq!(matches[0].compute_units, 100);
        assert_eq!(matches[0].price_per_unit, 12);
    }

    #[test]
    fn test_partial_match() {
        let mut book = OrderBook::new();
        let buy = Order::new(OrderType::Buy, "buyer1".into(), 50, 20, 3);
        let sell = Order::new(OrderType::Sell, "seller1".into(), 100, 15, 3);
        book.add_order(buy);
        book.add_order(sell);

        let matches = OrderMatcher::match_orders(&mut book);
        assert!(!matches.is_empty());
        assert_eq!(matches[0].compute_units, 50);
    }

    #[test]
    fn test_no_match_different_difficulty() {
        let mut book = OrderBook::new();
        let buy = Order::new(OrderType::Buy, "buyer1".into(), 100, 15, 5);
        let sell = Order::new(OrderType::Sell, "seller1".into(), 100, 10, 3);
        book.add_order(buy);
        book.add_order(sell);

        let matches = OrderMatcher::match_orders(&mut book);
        assert!(matches.is_empty());
    }

    #[test]
    fn test_calculate_value() {
        assert_eq!(OrderMatcher::calculate_value(100, 10), 1000);
    }

    #[test]
    fn test_market_summary() {
        let mut book = OrderBook::new();
        book.add_order(Order::new(OrderType::Buy, "b1".into(), 100, 20, 3));
        book.add_order(Order::new(OrderType::Sell, "s1".into(), 100, 15, 3));

        let summary = OrderMatcher::market_summary(&book);
        assert_eq!(summary.open_buy_orders, 1);
        assert_eq!(summary.open_sell_orders, 1);
        assert_eq!(summary.best_bid, Some(20));
        assert_eq!(summary.best_ask, Some(15));
        assert_eq!(summary.spread, Some(5));
    }

    #[test]
    fn test_fifo_match() {
        let mut book = OrderBook::new();
        book.add_order(Order::new(OrderType::Buy, "b1".into(), 50, 15, 3));
        book.add_order(Order::new(OrderType::Sell, "s1".into(), 50, 10, 3));

        let matches = OrderMatcher::match_fifo(&mut book);
        assert!(!matches.is_empty());
    }
}
