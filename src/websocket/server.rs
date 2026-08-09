use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_tungstenite::{accept_async, tungstenite::protocol::Message};

#[derive(Clone)]
pub struct WebSocketServer {
    clients: Arc<Mutex<HashMap<String, tokio::sync::mpsc::UnboundedSender<String>>>>,
    port: u16,
}

impl WebSocketServer {
    pub fn new(port: u16) -> Self {
        Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
            port,
        }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port)).await?;
        println!("🔌 WebSocket Server on ws://0.0.0.0:{}", self.port);

        while let Ok((stream, _addr)) = listener.accept().await {
            let clients = self.clients.clone();
            tokio::spawn(async move {
                Self::handle_connection(stream, clients).await;
            });
        }
        Ok(())
    }

    async fn handle_connection(
        stream: TcpStream,
        clients: Arc<Mutex<HashMap<String, tokio::sync::mpsc::UnboundedSender<String>>>>,
    ) {
        let ws_stream = match accept_async(stream).await {
            Ok(ws) => ws,
            Err(_) => return,
        };

        let (mut sender, mut receiver) = ws_stream.split();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let client_id = uuid::Uuid::new_v4().to_string();

        clients.lock().await.insert(client_id.clone(), tx);
        println!("✅ WS Client: {}", &client_id[..8]);

        let welcome = serde_json::json!({"type":"connected","id":&client_id[..8]}).to_string();
        let _ = sender.send(Message::Text(welcome)).await;

        // استقبال
        let recv_clients = clients.clone();
        let recv_id = client_id.clone();
        tokio::spawn(async move {
            while let Some(Ok(msg)) = receiver.next().await {
                if let Ok(text) = msg.to_text() {
                    println!(
                        "📩 WS from {}: {}",
                        &recv_id[..8],
                        &text[..50.min(text.len())]
                    );
                }
            }
            recv_clients.lock().await.remove(&recv_id);
        });

        // إرسال
        while let Some(msg) = rx.recv().await {
            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
        clients.lock().await.remove(&client_id);
    }

    pub async fn broadcast(&self, message: &str) {
        let clients = self.clients.lock().await;
        for (_, tx) in clients.iter() {
            let _ = tx.send(message.to_string());
        }
    }
}
