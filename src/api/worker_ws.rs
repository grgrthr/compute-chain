use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use crate::api::worker_registry;
use tokio::sync::mpsc;
use axum::response::IntoResponse;
use futures_util::StreamExt;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worker_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,
}

lazy_static::lazy_static! {
    pub static ref WORKER_EVENTS: Arc<tokio::sync::broadcast::Sender<String>> = 
        Arc::new(tokio::sync::broadcast::channel::<String>(256).0);
}
use lazy_static::lazy_static;

pub async fn worker_ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_worker(socket))
}

async fn handle_worker(mut socket: WebSocket) {
    let worker_id = format!("worker_{:04x}", 
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() % 0xFFFF);
    
    tracing::info!("🔗 Browser worker connected: {}", worker_id);
    
    // Send welcome
    let welcome = serde_json::json!({
        "type": "welcome",
        "worker_id": worker_id,
        "status": "connected"
    });
    let _ = socket.send(Message::Text(welcome.to_string())).await;
    
    // Broadcast worker connected event
    let event = serde_json::json!({
        "type": "worker_connected",
        "worker_id": worker_id,
        "capabilities": ["image", "hash", "csv", "thumbnail"],
        "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
    });
    let _ = WORKER_EVENTS.send(event.to_string());
    
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    worker_registry::register_worker(&worker_id, vec!["compress".into(),"thumbnail".into(),"csv".into(),"hash".into()], tx);
    // Main loop — receive messages from browser AND job dispatcher
    loop {
        let msg = tokio::select! {
            Some(dispatch_msg) = rx.recv() => {
                let _ = socket.send(Message::Text(dispatch_msg)).await;
                continue;
            }
            socket_msg = socket.recv() => {
                match socket_msg {
                    Some(Ok(msg)) => msg,
                    _ => break,
                }
            }
        };
        if let Message::Text(text) = msg {
            if let Ok(worker_msg) = serde_json::from_str::<WorkerMessage>(&text) {
                match worker_msg.msg_type.as_str() {
                    "register" => {
                        let capabilities = worker_msg.capabilities.unwrap_or_default();
                        tracing::info!("📋 Worker {} registered with {:?}", worker_id, capabilities);
                        let event = serde_json::json!({
                            "type": "worker_registered",
                            "worker_id": worker_id,
                            "capabilities": capabilities
                        });
                        let _ = WORKER_EVENTS.send(event.to_string());
                        
                        let ack = serde_json::json!({
                            "type": "registered",
                            "worker_id": worker_id,
                            "status": "online"
                        });
                        let _ = socket.send(Message::Text(ack.to_string())).await;
                    }
                    "result" => {
                        let job_id = worker_msg.job_id.as_deref().unwrap_or("unknown");
                        tracing::info!("📤 Worker {} completed job {}", worker_id, job_id);
                        
                        let success = worker_msg.result.as_ref()
                            .and_then(|r| r.get("success"))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true);
                        
                        let result_data = worker_msg.result.clone().unwrap_or(serde_json::json!({}));
                        
                        // Process through browser_jobs pipeline
                        match crate::api::handlers::browser_jobs::handle_browser_result(
                            job_id, &worker_id, success, &result_data
                        ) {
                            Ok(record) => {
                                tracing::info!("📊 BrowserJob {} processed: status={}, verification={:?}", 
                                    job_id, record.status, record.verification_status);
                                
                                let event = serde_json::json!({
                                    "type": "job_result",
                                    "worker_id": worker_id,
                                    "job_id": job_id,
                                    "result": result_data,
                                    "status": record.status,
                                    "verification": record.verification_status,
                                    "output_hash": record.output_hash.as_ref().map(|h| &h[..16]),
                                    "merkle_root": record.merkle_root.as_ref().map(|h| &h[..16])
                                });
                                let _ = WORKER_EVENTS.send(event.to_string());
                                
                                // Emit internal event for proof+block+reward finalization
                                let finalize_event = serde_json::json!({
                                    "type": "browser_result_completed",
                                    "job_id": job_id,
                                    "worker_id": worker_id
                                });
                                let _ = WORKER_EVENTS.send(finalize_event.to_string());
                                
                                // Release worker
                                crate::api::worker_registry::set_worker_idle(&worker_id);
                                let idle_event = serde_json::json!({
                                    "type": "worker_idle",
                                    "worker_id": worker_id
                                });
                                let _ = WORKER_EVENTS.send(idle_event.to_string());
                            }
                            Err(e) => {
                                tracing::warn!("❌ BrowserJob {} failed: {}", job_id, e);
                                let event = serde_json::json!({
                                    "type": "job_result",
                                    "worker_id": worker_id,
                                    "job_id": job_id,
                                    "status": "failed",
                                    "error": e
                                });
                                let _ = WORKER_EVENTS.send(event.to_string());
                                crate::api::worker_registry::set_worker_idle(&worker_id);
                                let idle_event = serde_json::json!({
                                    "type": "worker_idle",
                                    "worker_id": worker_id
                                });
                                let _ = WORKER_EVENTS.send(idle_event.to_string());
                            }
                        }
                    }
                    "progress" => {
                        let event = serde_json::json!({
                            "type": "job_progress",
                            "worker_id": worker_id,
                            "job_id": worker_msg.job_id,
                            "progress": worker_msg.progress
                        });
                        let _ = WORKER_EVENTS.send(event.to_string());
                    }
                    "ping" => {
                        let pong = serde_json::json!({"type": "pong"});
                        let _ = socket.send(Message::Text(pong.to_string())).await;
                    }
                    _ => {}
                }
            }
        }
    }
    
    // Worker disconnected
    worker_registry::unregister_worker(&worker_id);
    tracing::info!("🔌 Browser worker disconnected: {}", worker_id);
    let event = serde_json::json!({
        "type": "worker_disconnected",
        "worker_id": worker_id
    });
    let _ = WORKER_EVENTS.send(event.to_string());
}

// ═══ WORKER EVENTS STREAM ═══
pub async fn worker_events_ws(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(move |mut socket| handle_events(socket))
}

async fn handle_events(mut socket: WebSocket) {
    let mut rx = WORKER_EVENTS.subscribe();
    loop {
        match rx.recv().await {
            Ok(msg) => {
                if socket.send(Message::Text(msg)).await.is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}

// ═══ STATIC FILE HANDLERS ═══
pub async fn worker_page_handler() -> axum::response::Html<String> {
    axum::response::Html(include_str!("../../assets/worker.html").to_string())
}

pub async fn tasks_js_handler() -> axum::response::Response {
    axum::response::Response::builder()
        .header("Content-Type", "application/javascript")
        .body(axum::body::Body::from(include_str!("../../assets/tasks.js").to_string()))
        .unwrap()
}

pub async fn demo_page_handler() -> axum::response::Html<String> {
    axum::response::Html(include_str!("../../assets/demo.html").to_string())
}

pub async fn sha256_js_handler() -> axum::response::Response {
    axum::response::Response::builder()
        .header("Content-Type", "application/javascript")
        .body(axum::body::Body::from(include_str!("../../assets/sha256.js").to_string()))
        .unwrap()
}
