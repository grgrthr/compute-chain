use crate::marketplace::types::{Order, OrderStatus};
use std::collections::VecDeque;

pub struct OrderBook {
    pub buy_orders: VecDeque<Order>,
    pub sell_orders: VecDeque<Order>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            buy_orders: VecDeque::new(),
            sell_orders: VecDeque::new(),
        }
    }

    pub fn add_order(&mut self, order: Order) {
        match order.order_type {
            crate::marketplace::types::OrderType::Buy => {
                self.buy_orders.push_back(order);
                self.sort_buy_orders();
            }
            crate::marketplace::types::OrderType::Sell => {
                self.sell_orders.push_back(order);
                self.sort_sell_orders();
            }
        }
    }

    pub fn remove_order(&mut self, order_id: &str) -> Option<Order> {
        if let Some(pos) = self.buy_orders.iter().position(|o| o.id == order_id) {
            return self.buy_orders.remove(pos);
        }
        if let Some(pos) = self.sell_orders.iter().position(|o| o.id == order_id) {
            return self.sell_orders.remove(pos);
        }
        None
    }

    pub fn get_open_orders(&self) -> Vec<Order> {
        let mut orders = Vec::new();
        orders.extend(
            self.buy_orders
                .iter()
                .filter(|o| o.status == OrderStatus::Open)
                .cloned(),
        );
        orders.extend(
            self.sell_orders
                .iter()
                .filter(|o| o.status == OrderStatus::Open)
                .cloned(),
        );
        orders
    }

    fn sort_buy_orders(&mut self) {
        let mut orders: Vec<Order> = self.buy_orders.drain(..).collect();
        orders.sort_by(|a, b| b.price_per_unit.cmp(&a.price_per_unit));
        self.buy_orders = orders.into_iter().collect();
    }

    fn sort_sell_orders(&mut self) {
        let mut orders: Vec<Order> = self.sell_orders.drain(..).collect();
        orders.sort_by(|a, b| a.price_per_unit.cmp(&b.price_per_unit));
        self.sell_orders = orders.into_iter().collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marketplace::types::OrderType;

    #[test]
    fn test_add_order() {
        let mut book = OrderBook::new();
        let order = Order::new(OrderType::Buy, "miner1".to_string(), 100, 10, 3);
        book.add_order(order);
        assert_eq!(book.buy_orders.len(), 1);
    }
}
