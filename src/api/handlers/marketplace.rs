use crate::api::handlers::AppState;
use crate::api::models::*;
use crate::marketplace::types::OrderType;
use axum::{extract::State, Json};
use std::sync::Arc;

pub async fn create_order_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateOrderRequest>,
) -> Json<OrderResponse> {
    let order_type = match request.order_type.as_str() {
        "buy" => OrderType::Buy,
        _ => OrderType::Sell,
    };
    let marketplace = state.marketplace.lock().unwrap();
    let order = marketplace.create_order(
        order_type,
        request.miner_id,
        request.compute_units,
        request.price_per_unit,
        request.difficulty_level,
    );
    Json(OrderResponse {
        id: order.id,
        order_type: request.order_type,
        miner_id: order.miner_id,
        compute_units: order.compute_units,
        price_per_unit: order.price_per_unit,
        difficulty_level: order.difficulty_level,
        status: format!("{:?}", order.status),
        created_at: order.created_at,
    })
}

pub async fn get_open_orders_handler(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<OrderResponse>> {
    let orders = state.marketplace.lock().unwrap().get_open_orders();
    Json(
        orders
            .into_iter()
            .map(|o| OrderResponse {
                id: o.id,
                order_type: match o.order_type {
                    OrderType::Buy => "buy".into(),
                    OrderType::Sell => "sell".into(),
                },
                miner_id: o.miner_id,
                compute_units: o.compute_units,
                price_per_unit: o.price_per_unit,
                difficulty_level: o.difficulty_level,
                status: format!("{:?}", o.status),
                created_at: o.created_at,
            })
            .collect(),
    )
}

pub async fn get_market_stats_handler(
    State(state): State<Arc<AppState>>,
) -> Json<MarketStatsResponse> {
    let stats = state.marketplace.lock().unwrap().get_stats();
    Json(MarketStatsResponse {
        total_orders: stats.total_orders,
        open_orders: stats.open_orders,
        total_compute_units: stats.total_compute_units,
        avg_price: stats.avg_price,
        total_volume: stats.total_volume,
    })
}

// Strategy: Insert Mediator for api (Interface)
// Review and adjust before applying.
