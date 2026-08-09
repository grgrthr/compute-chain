use crate::api::handlers::AppState;
use axum::{extract::State, Json};
use std::collections::HashMap;
use std::sync::Arc;

pub async fn send_transaction_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let from = request["from"].as_str().unwrap_or("");
    let to = request["to"].as_str().unwrap_or("");
    let amount = request["amount"].as_u64().unwrap_or(0);

    if from.is_empty() || to.is_empty() {
        return Json(serde_json::json!({ "status": "error", "message": "Missing from/to" }));
    }

    tracing::info!("💸 TX: {} -> {} amount={}", from, to, amount);

    // Get current nonce for sender
    let nonce = state.token_engine.get_nonce(from);

    // Create transaction with nonce
    let mut tx =
        crate::consensus::types::Transaction::new(from.to_string(), to.to_string(), amount, 1);
    tx.nonce = nonce;
    let msg = tx.signing_message();

    // Sign the transaction
    let (public_key, signature) = {
        let wallet = state.wallet.lock().unwrap();
        let pk = wallet.get_public_key(from).unwrap_or_default();
        let sig = wallet.sign(from, &msg).unwrap_or_default();
        (pk, sig)
    };

    tx.public_key = public_key.clone();
    tx.signature = signature.clone();

    // Verify signature (MANDATORY)
    if public_key.is_empty() || signature.is_empty() {
        return Json(
            serde_json::json!({ "status": "error", "message": "Transaction must be signed. No valid keys found for sender." }),
        );
    }
    if !tx.verify_signature() {
        return Json(
            serde_json::json!({ "status": "error", "message": "Invalid signature - transaction rejected" }),
        );
    }

    // Add to mempool with nonce check
    {
        let current_nonce = state.token_engine.get_nonce(from);
        let mut mempool = state.mempool.inner.write().unwrap();
        match mempool.add(tx.clone(), current_nonce) {
            Ok(()) => {
                tracing::info!("📝 TX added to mempool: {} (nonce: {})", tx.id, tx.nonce);
                state.token_engine.increment_nonce(from);
                let _ = mempool.save_to_disk("./chain_data");
            }
            Err(e) => {
                return Json(serde_json::json!({ "status": "error", "message": e }));
            }
        }
    }

    // Broadcast to peers
    let tx_broadcast = crate::p2p::TransactionBroadcast {
        id: tx.id.clone(),
        from: from.to_string(),
        to: to.to_string(),
        amount,
        fee: 1,
        timestamp: tx.timestamp,
    };
    let _ = state.p2p_handle.broadcast_transaction(tx_broadcast).await;

    Json(serde_json::json!({
        "status": "success",
        "tx_hash": tx.id,
        "from": from,
        "to": to,
        "amount": amount,
        "nonce": nonce,
        "signed": true,
        "in_mempool": true
    }))
}

pub async fn get_balance_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(p): axum::extract::Query<HashMap<String, String>>,
) -> Json<serde_json::Value> {
    let address = p.get("address").cloned().unwrap_or_default();
    if address.is_empty() {
        return Json(
            serde_json::json!({"error": "Missing 'address' parameter. Usage: GET /tx/balance?address=genesis"}),
        );
    }
    let nonce = state.token_engine.get_nonce(&address);
    Json(
        serde_json::json!({ "address": address, "balance": state.token_engine.get_balance(&address), "total_supply": state.token_engine.get_total_supply(), "nonce": nonce }),
    )
}

// Strategy: Insert Mediator for api (Interface)
// Review and adjust before applying.
