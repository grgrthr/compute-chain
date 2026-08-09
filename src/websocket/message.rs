use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WSMessage {
    pub msg_type: MessageType,
    pub payload: String,
    pub sender: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageType {
    NewBlock,
    NewTransaction,
    NewOrder,
    PeerConnected,
    ExecuteWorkload,
    WorkloadResult,
    Ping,
    Pong,
}

impl WSMessage {
    pub fn new(msg_type: MessageType, payload: String, sender: String) -> Self {
        Self {
            msg_type,
            payload,
            sender,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}
