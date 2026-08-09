use futures::{SinkExt, StreamExt};
use serde_json;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

use crate::websocket::message::{MessageType, WSMessage};

pub struct WebSocketClient {
    url: String,
    connected: Arc<Mutex<bool>>,
}

impl WebSocketClient {
    pub fn new(server_url: String) -> Self {
        Self {
            url: server_url,
            connected: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn connect(&self) -> Result<(), Box<dyn std::error::Error>> {
        let (ws_stream, _) = connect_async(&self.url).await?;
        let (mut sender, mut receiver) = ws_stream.split();

        {
            let mut conn = self.connected.lock().await;
            *conn = true;
        }

        println!("✅ Connected to WebSocket server at {}", self.url);

        let connected_clone = self.connected.clone();

        tokio::spawn(async move {
            while let Some(msg) = receiver.next().await {
                match msg {
                    Ok(msg) => {
                        if let Ok(text) = msg.to_text() {
                            if let Ok(ws_msg) = serde_json::from_str::<WSMessage>(text) {
                                println!(
                                    "📥 Received: {:?} from {}",
                                    ws_msg.msg_type, ws_msg.sender
                                );
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error receiving message: {}", e);
                        break;
                    }
                }
            }
            let mut conn = connected_clone.lock().await;
            *conn = false;
        });

        let ping = WSMessage::new(MessageType::Ping, "ping".into(), "client".to_string());
        sender
            .send(Message::Text(serde_json::to_string(&ping)?))
            .await?;

        Ok(())
    }

    pub async fn is_connected(&self) -> bool {
        *self.connected.lock().await
    }
}
