use futures::StreamExt;
use libp2p::{
    mdns,
    core::ConnectedPoint,
    gossipsub::{self, IdentTopic as GossipsubIdentTopic, MessageAuthenticity},
    identify,
    identify::Event as IdentifyEvent,
    identity,
    kad::{self, store::MemoryStore, QueryResult},
    noise, ping,
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, Transport,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::error::Error;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};

// ============================================================
// Types for gossip messages
// ============================================================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkMessage {
    Transaction(TransactionBroadcast),
    Block(BlockBroadcast),
    ChainSync(ChainSyncRequest),
    ChainResponse(ChainSyncResponse),
    PbftPrePrepare(PbftPrePrepareMsg),
    PbftPrepare(PbftVoteMsg),
    PbftCommit(PbftVoteMsg),
    WorkAnnounce(crate::p2p::message::WorkloadAnnouncement),
    WorkResult(crate::p2p::message::WorkloadResult),
    WorkRequest(crate::p2p::message::WorkloadRequest),
    WorkAssignment(crate::p2p::message::WorkloadAssignment),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionBroadcast {
    pub id: String,
    pub from: String,
    pub to: String,
    pub amount: u64,
    pub fee: u64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockBroadcast {
    pub height: u64,
    pub hash: String,
    pub previous_hash: String,
    pub validator_id: String,
    pub transaction_count: u64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainSyncRequest {
    pub requester_peer: String,
    pub current_height: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainSyncResponse {
    pub height: u64,
    pub blocks: Vec<BlockBroadcast>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PbftPrePrepareMsg {
    pub round: u64,
    pub view: u64,
    pub block_height: u64,
    pub block_hash: String,
    pub previous_hash: String,
    pub validator_id: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PbftVoteMsg {
    pub round: u64,
    pub view: u64,
    pub block_hash: String,
    pub validator_id: String,
    pub vote_type: String,
}

// ============================================================
// Actor Pattern: Commands & Events
// ============================================================

#[derive(Debug)]
pub enum P2PCommand {
    Dial {
        addr: String,
    },
    BroadcastBlock {
        height: u64,
        hash: String,
        previous_hash: String,
        validator_id: String,
        transaction_count: u64,
        timestamp: u64,
    },
    BroadcastTransaction(TransactionBroadcast),
    RequestChainSync {
        current_height: u64,
    },
    BroadcastChainResponse(ChainSyncResponse),
    BroadcastPbftPrePrepare(PbftPrePrepareMsg),
    BroadcastPbftVote(PbftVoteMsg),
    GetPeerCount {
        reply: tokio::sync::oneshot::Sender<usize>,
    },
    GetPeers {
        reply: tokio::sync::oneshot::Sender<Vec<String>>,
    },
    BroadcastWorkload {
        announcement: crate::p2p::message::WorkloadAnnouncement,
    },
    BroadcastResult {
        result: crate::p2p::message::WorkloadResult,
    },
    BroadcastAssignment {
        assignment: crate::p2p::message::WorkloadAssignment,
    },
    SendWorkloadRequest {
        request: crate::p2p::message::WorkloadRequest,
    },
}

#[derive(Debug, Clone)]
pub enum P2PEvent {
    PeerConnected {
        peer_id: String,
    },
    PeerDiscovered {
        peer_id: String,
        address: String,
    },
    PeerDisconnected {
        peer_id: String,
    },
    BlockReceived(BlockBroadcast),
    TransactionReceived(TransactionBroadcast),
    ChainSyncRequested {
        requester_peer: String,
        current_height: u64,
    },
    ChainSyncResponseReceived(ChainSyncResponse),
    PbftPrePrepareReceived(PbftPrePrepareMsg),
    PbftVoteReceived(PbftVoteMsg),
    NewListenAddr {
        address: String,
    },
    WorkloadAnnounceReceived(crate::p2p::message::WorkloadAnnouncement),
    WorkloadResultReceived(crate::p2p::message::WorkloadResult),
    WorkloadRequestReceived(crate::p2p::message::WorkloadRequest),
    WorkloadAssignmentReceived(crate::p2p::message::WorkloadAssignment),
}

// ============================================================
// Topics for gossipsub
// ============================================================
const TOPIC_BLOCKS: &str = "compute-chain/blocks";
const TOPIC_TRANSACTIONS: &str = "compute-chain/transactions";
const TOPIC_SYNC: &str = "compute-chain/sync";
const TOPIC_PBFT: &str = "compute-chain/pbft";
const TOPIC_WORKLOADS: &str = "compute-chain/workloads";
const TOPIC_RESULTS: &str = "compute-chain/results";

// ============================================================
// P2P Handle
// ============================================================
pub struct P2PHandle {
    pub command_tx: mpsc::Sender<P2PCommand>,
    pub local_peer_id: PeerId,
    pub listen_addr: Multiaddr,
}

impl Clone for P2PHandle {
    fn clone(&self) -> Self {
        Self {
            command_tx: self.command_tx.clone(),
            local_peer_id: self.local_peer_id,
            listen_addr: self.listen_addr.clone(),
        }
    }
}

impl P2PHandle {
    pub async fn dial_peer(&self, addr: &str) -> Result<(), String> {
        self.command_tx
            .send(P2PCommand::Dial {
                addr: addr.to_string(),
            })
            .await
            .map_err(|e| format!("Failed to send dial command: {}", e))?;
        Ok(())
    }

    pub async fn broadcast_block(
        &self,
        height: u64,
        hash: String,
        previous_hash: String,
        validator_id: String,
        transaction_count: u64,
        timestamp: u64,
    ) -> Result<(), String> {
        self.command_tx
            .send(P2PCommand::BroadcastBlock {
                height,
                hash,
                previous_hash,
                validator_id,
                transaction_count,
                timestamp,
            })
            .await
            .map_err(|e| format!("Failed to send broadcast block command: {}", e))?;
        Ok(())
    }

    pub async fn broadcast_transaction(&self, tx: TransactionBroadcast) -> Result<(), String> {
        self.command_tx
            .send(P2PCommand::BroadcastTransaction(tx))
            .await
            .map_err(|e| format!("Failed to send broadcast transaction command: {}", e))?;
        Ok(())
    }

    pub async fn request_chain_sync(&self, current_height: u64) -> Result<(), String> {
        self.command_tx
            .send(P2PCommand::RequestChainSync { current_height })
            .await
            .map_err(|e| format!("Failed to send chain sync request: {}", e))?;
        Ok(())
    }

    pub async fn broadcast_chain_response(
        &self,
        response: ChainSyncResponse,
    ) -> Result<(), String> {
        self.command_tx
            .send(P2PCommand::BroadcastChainResponse(response))
            .await
            .map_err(|e| format!("Failed to send chain response: {}", e))?;
        Ok(())
    }

    pub async fn broadcast_pbft_pre_prepare(&self, msg: PbftPrePrepareMsg) -> Result<(), String> {
        self.command_tx
            .send(P2PCommand::BroadcastPbftPrePrepare(msg))
            .await
            .map_err(|e| format!("Failed to send PBFT PrePrepare: {}", e))?;
        Ok(())
    }

    pub async fn broadcast_pbft_vote(&self, msg: PbftVoteMsg) -> Result<(), String> {
        self.command_tx
            .send(P2PCommand::BroadcastPbftVote(msg))
            .await
            .map_err(|e| format!("Failed to send PBFT vote: {}", e))?;
        Ok(())
    }

    pub async fn get_peer_count(&self) -> Result<usize, String> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.command_tx
            .send(P2PCommand::GetPeerCount { reply: tx })
            .await
            .map_err(|e| format!("Failed to send get peer count command: {}", e))?;
        rx.await
            .map_err(|e| format!("Failed to receive peer count: {}", e))
    }

    pub async fn get_connected_peers(&self) -> Result<Vec<String>, String> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.command_tx
            .send(P2PCommand::GetPeers { reply: tx })
            .await
            .map_err(|e| format!("Failed to send get peers command: {}", e))?;
        rx.await
            .map_err(|e| format!("Failed to receive peers: {}", e))
    }
}

// ============================================================
// P2PNode
// ============================================================
pub struct P2PNode {
    pub swarm: Swarm<Behaviour>,
    pub local_peer_id: PeerId,
    pub listen_addr: Multiaddr,
    pub connected_peers: HashSet<PeerId>,
    pub received_blocks: Vec<BlockBroadcast>,
    pub received_transactions: Vec<TransactionBroadcast>,
}

#[derive(NetworkBehaviour)]
pub struct Behaviour {
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
    pub gossipsub: gossipsub::Behaviour,
    pub kademlia: kad::Behaviour<MemoryStore>,
    pub mdns: mdns::tokio::Behaviour,
}

impl P2PNode {
    pub async fn new(p2p_port: u16) -> Result<Self, Box<dyn Error>> {
        let key = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(key.public());
        let local_key = key.clone();

        let tcp = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true));
        let noise = noise::Config::new(&key)?;
        let transport = tcp
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(noise)
            .multiplex(yamux::Config::default())
            .boxed();

        let identify = identify::Behaviour::new(identify::Config::new(
            "/compute-chain/1.0.0".into(),
            key.public(),
        ));
        let ping = ping::Behaviour::new(ping::Config::new().with_interval(Duration::from_secs(5)));

        let gossipsub_config = gossipsub::Config::default();
        let mut gossipsub = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(local_key.clone()),
            gossipsub_config,
        )?;
        gossipsub.subscribe(&GossipsubIdentTopic::new(TOPIC_BLOCKS))?;
        gossipsub.subscribe(&GossipsubIdentTopic::new(TOPIC_TRANSACTIONS))?;
        gossipsub.subscribe(&GossipsubIdentTopic::new(TOPIC_SYNC))?;
        gossipsub.subscribe(&GossipsubIdentTopic::new(TOPIC_PBFT))?;
        gossipsub.subscribe(&GossipsubIdentTopic::new(TOPIC_WORKLOADS))?;
        gossipsub.subscribe(&GossipsubIdentTopic::new(TOPIC_RESULTS))?;
        tracing::info!(
            "📢 Gossipsub subscribed to topics: {}, {}, {}, {}",
            TOPIC_BLOCKS,
            TOPIC_TRANSACTIONS,
            TOPIC_SYNC,
            TOPIC_PBFT
        );

        let kademlia = kad::Behaviour::new(peer_id, MemoryStore::new(peer_id));
        let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), peer_id)?;
        let behaviour = Behaviour {
            identify,
            ping,
            gossipsub,
            kademlia,
            mdns,
        };
        let swarm_config = libp2p::swarm::Config::with_tokio_executor();
        let mut swarm = Swarm::new(transport, behaviour, peer_id, swarm_config);

        let addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", p2p_port).parse()?;
        swarm.listen_on(addr.clone())?;

        tracing::info!("Peer ID: {}", peer_id);
        tracing::info!("Listening on {}", addr);

        Ok(Self {
            swarm,
            local_peer_id: peer_id,
            listen_addr: addr,
            connected_peers: HashSet::new(),
            received_blocks: Vec::new(),
            received_transactions: Vec::new(),
        })
    }

    pub async fn run(
        mut self,
        mut command_rx: mpsc::Receiver<P2PCommand>,
        event_tx: broadcast::Sender<P2PEvent>,
        _auto_dial_peers: Vec<String>,
    ) {
        if let Err(e) = self.swarm.behaviour_mut().kademlia.bootstrap() {
            tracing::warn!("🔍 Kademlia bootstrap failed: {} (single node mode)", e);
        } else {
            tracing::info!("🔍 Kademlia bootstrap started");
        }

        loop {
            tokio::select! {
                Some(cmd) = command_rx.recv() => {
                    match cmd {
                        P2PCommand::Dial { addr } => {
                            if let Ok(m) = addr.parse::<Multiaddr>() {
                                tracing::info!("Dialing peer: {}", m);
                                if let Err(e) = self.swarm.dial(m) { tracing::error!("Dial failed: {}", e); }
                            }
                        }
                        P2PCommand::BroadcastBlock { height, hash, previous_hash, validator_id, transaction_count, timestamp } => {
                            let msg = NetworkMessage::Block(BlockBroadcast { height, hash, previous_hash, validator_id, transaction_count, timestamp });
                            if let Ok(json) = serde_json::to_string(&msg) {
                                if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(GossipsubIdentTopic::new(TOPIC_BLOCKS), json.as_bytes()) {
                                    tracing::error!("Block broadcast failed: {}", e);
                                } else { tracing::info!("📤 Block broadcasted: height={}", height); }
                            }
                        }
                        P2PCommand::BroadcastTransaction(tx) => {
                            let msg = NetworkMessage::Transaction(tx.clone());
                            if let Ok(json) = serde_json::to_string(&msg) {
                                if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(GossipsubIdentTopic::new(TOPIC_TRANSACTIONS), json.as_bytes()) {
                                    tracing::error!("TX broadcast failed: {}", e);
                                } else { tracing::info!("📤 TX broadcasted: id={}", tx.id); }
                            }
                        }
                        P2PCommand::RequestChainSync { current_height } => {
                            let msg = NetworkMessage::ChainSync(ChainSyncRequest { requester_peer: self.local_peer_id.to_string(), current_height });
                            if let Ok(json) = serde_json::to_string(&msg) {
                                if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(GossipsubIdentTopic::new(TOPIC_SYNC), json.as_bytes()) {
                                    tracing::error!("Chain sync request failed: {}", e);
                                } else { tracing::info!("📤 Chain sync request sent: height={}", current_height); }
                            }
                        }
                        P2PCommand::BroadcastChainResponse(response) => {
                            let msg = NetworkMessage::ChainResponse(response);
                            if let Ok(json) = serde_json::to_string(&msg) {
                                if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(GossipsubIdentTopic::new(TOPIC_SYNC), json.as_bytes()) {
                                    tracing::error!("Chain response broadcast failed: {}", e);
                                } else { tracing::info!("📤 Chain sync response broadcasted"); }
                            }
                        }
                        P2PCommand::BroadcastPbftPrePrepare(msg) => {
                            let round = msg.round;
                            let view = msg.view;
                            let net_msg = NetworkMessage::PbftPrePrepare(msg);
                            if let Ok(json) = serde_json::to_string(&net_msg) {
                                if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(GossipsubIdentTopic::new(TOPIC_PBFT), json.as_bytes()) {
                                    tracing::error!("PBFT PrePrepare broadcast failed: {}", e);
                                } else { tracing::info!("📤 PBFT PrePrepare broadcasted: round={}, view={}", round, view); }
                            }
                        }
                        P2PCommand::BroadcastPbftVote(msg) => {
                            let round = msg.round;
                            let view = msg.view;
                            let vote_type = msg.vote_type.clone();
                            let net_msg = if msg.vote_type == "prepare" {
                                NetworkMessage::PbftPrepare(msg)
                            } else {
                                NetworkMessage::PbftCommit(msg)
                            };
                            if let Ok(json) = serde_json::to_string(&net_msg) {
                                if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(GossipsubIdentTopic::new(TOPIC_PBFT), json.as_bytes()) {
                                    tracing::error!("PBFT vote broadcast failed: {}", e);
                                } else { tracing::info!("📤 PBFT {} broadcasted: round={}, view={}", vote_type, round, view); }
                            }
                        }
                        P2PCommand::GetPeerCount { reply } => { let _ = reply.send(self.connected_peers.len()); }
                        P2PCommand::BroadcastWorkload { announcement } => {
                            let msg = NetworkMessage::WorkAnnounce(announcement);
                            if let Ok(json) = serde_json::to_string(&msg) {
                                let _ = self.swarm.behaviour_mut().gossipsub.publish(GossipsubIdentTopic::new(TOPIC_WORKLOADS), json.as_bytes());
                            }
                        }
                        P2PCommand::SendWorkloadRequest { request } => {
                            let msg = NetworkMessage::WorkRequest(request);
                            if let Ok(json) = serde_json::to_string(&msg) {
                                let _ = self.swarm.behaviour_mut().gossipsub.publish(GossipsubIdentTopic::new(TOPIC_WORKLOADS), json.as_bytes());
                            }
                        }
                        P2PCommand::BroadcastAssignment { assignment } => {
                            let msg = NetworkMessage::WorkAssignment(assignment);
                            if let Ok(json) = serde_json::to_string(&msg) {
                                let _ = self.swarm.behaviour_mut().gossipsub.publish(GossipsubIdentTopic::new(TOPIC_WORKLOADS), json.as_bytes());
                            }
                        }
                        P2PCommand::BroadcastResult { result } => {
                            let msg = NetworkMessage::WorkResult(result);
                            if let Ok(json) = serde_json::to_string(&msg) {
                                let _ = self.swarm.behaviour_mut().gossipsub.publish(GossipsubIdentTopic::new(TOPIC_RESULTS), json.as_bytes());
                            }
                        }
                        P2PCommand::GetPeers { reply } => {
                            let peers: Vec<String> = self.connected_peers.iter().map(|p| p.to_string()).collect();
                            let _ = reply.send(peers);
                        }
                    }
                }

                event = self.swarm.select_next_some() => {
                    match event {
                        SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                            tracing::error!("❌ OutgoingConnectionError: peer={:?} error={:?}", peer_id, error);
                        }
                        SwarmEvent::IncomingConnectionError { error, .. } => {
                            tracing::error!("❌ IncomingConnectionError: error={:?}", error);
                        }
                        SwarmEvent::Behaviour(BehaviourEvent::Ping(event)) => {
                            tracing::info!("🏓 Ping event: {:?}", event);
                        }
                        SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                            self.connected_peers.insert(peer_id);
                            tracing::info!("🔗 Connected: {}", peer_id);
                            match &endpoint {
                                ConnectedPoint::Dialer { address, .. } => {
                                    self.swarm.behaviour_mut().kademlia.add_address(&peer_id, address.clone());
                                }
                                ConnectedPoint::Listener { send_back_addr, .. } => {
                                    self.swarm.behaviour_mut().kademlia.add_address(&peer_id, send_back_addr.clone());
                                }
                            }
                            let _ = event_tx.send(P2PEvent::PeerConnected { peer_id: peer_id.to_string() });
                            if let Err(e) = self.swarm.behaviour_mut().kademlia.bootstrap() {
                                tracing::warn!("🔍 Kademlia bootstrap retry failed: {}", e);
                            } else { tracing::info!("🔍 Kademlia bootstrap retry started"); }
                        }
                        SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                            self.connected_peers.remove(&peer_id);
                            self.swarm.behaviour_mut().kademlia.remove_peer(&peer_id);
                            tracing::warn!("🔌 Disconnected: {} cause={:?}", peer_id, cause);
                            let _ = event_tx.send(P2PEvent::PeerDisconnected { peer_id: peer_id.to_string() });
                        }
                        SwarmEvent::NewListenAddr { address, .. } => {
                            let _ = event_tx.send(P2PEvent::NewListenAddr { address: address.to_string() });
                        }
                        SwarmEvent::Behaviour(BehaviourEvent::Identify(IdentifyEvent::Received { peer_id, info, .. })) => {
                            tracing::info!("🪪 Identify received from {} ({} listen addresses)", peer_id, info.listen_addrs.len());
                            for addr in &info.listen_addrs {
                                self.swarm.behaviour_mut().kademlia.add_address(&peer_id, addr.clone());
                                let _ = event_tx.send(P2PEvent::PeerDiscovered { peer_id: peer_id.to_string(), address: addr.to_string() });
                            }
                        }
                        SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(gossipsub::Event::Message { message, .. })) => {
                            if let Ok(text) = String::from_utf8(message.data) {
                                if let Ok(net_msg) = serde_json::from_str::<NetworkMessage>(&text) {
                                    match net_msg {
                                        NetworkMessage::Block(block) => {
                                            tracing::info!("📥 Received block via gossip: height={}", block.height);
                                            self.received_blocks.push(block.clone());
                                            let _ = event_tx.send(P2PEvent::BlockReceived(block));
                                        }
                                        NetworkMessage::Transaction(tx) => {
                                            tracing::info!("📥 Received TX via gossip: id={}", tx.id);
                                            self.received_transactions.push(tx.clone());
                                            let _ = event_tx.send(P2PEvent::TransactionReceived(tx));
                                        }
                                        NetworkMessage::ChainSync(request) => {
                                            let _ = event_tx.send(P2PEvent::ChainSyncRequested { requester_peer: request.requester_peer, current_height: request.current_height });
                                        }
                                        NetworkMessage::ChainResponse(response) => {
                                            let _ = event_tx.send(P2PEvent::ChainSyncResponseReceived(response));
                                        }
                                        NetworkMessage::PbftPrePrepare(msg) => {
                                            tracing::info!("📥 PBFT PrePrepare received: round={}, view={}, height={}", msg.round, msg.view, msg.block_height);
                                            let _ = event_tx.send(P2PEvent::PbftPrePrepareReceived(msg));
                                        }
                                        NetworkMessage::PbftPrepare(msg) => {
                                            tracing::info!("📥 PBFT Prepare received: from={}, round={}, view={}", msg.validator_id, msg.round, msg.view);
                                            let _ = event_tx.send(P2PEvent::PbftVoteReceived(msg));
                                        }
                                        NetworkMessage::PbftCommit(msg) => {
                                            tracing::info!("📥 PBFT Commit received: from={}, round={}, view={}", msg.validator_id, msg.round, msg.view);
                                            let _ = event_tx.send(P2PEvent::PbftVoteReceived(msg));
                                        }
                                        NetworkMessage::WorkAnnounce(msg) => {
                                            let _ = event_tx.send(P2PEvent::WorkloadAnnounceReceived(msg));
                                        }
                                        NetworkMessage::WorkResult(msg) => {
                                            let _ = event_tx.send(P2PEvent::WorkloadResultReceived(msg));
                                        }
                                        NetworkMessage::WorkRequest(msg) => {
                                            let _ = event_tx.send(P2PEvent::WorkloadRequestReceived(msg));
                                        }
                                        NetworkMessage::WorkAssignment(msg) => {
                                            let _ = event_tx.send(P2PEvent::WorkloadAssignmentReceived(msg));
                                        }
                                    }
                                }
                            }
                        }
                        SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                            for (peer_id, addr) in list {
                                self.swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
                            }
                        }
                        SwarmEvent::Behaviour(BehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed { result, .. })) => {
                            if let QueryResult::Bootstrap(Ok(ok)) = result {
                                tracing::info!("🔍 Kademlia bootstrap OK: {} peers remaining", ok.num_remaining);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

pub async fn spawn_p2p_actor(
    p2p_port: u16,
    auto_dial_peers: Vec<String>,
) -> Result<
    (
        P2PHandle,
        broadcast::Receiver<P2PEvent>,
        tokio::task::JoinHandle<()>,
    ),
    Box<dyn Error>,
> {
    let node = P2PNode::new(p2p_port).await?;
    let local_peer_id = node.local_peer_id;
    let listen_addr = node.listen_addr.clone();
    let (command_tx, command_rx) = mpsc::channel::<P2PCommand>(256);
    let (event_tx, event_rx) = broadcast::channel::<P2PEvent>(256);
    let handle = P2PHandle {
        command_tx,
        local_peer_id,
        listen_addr,
    };
    let join_handle =
        tokio::spawn(async move { node.run(command_rx, event_tx, auto_dial_peers).await });
    Ok((handle, event_rx, join_handle))
}

// Strategy: Move Dependencies Down for p2p (Infrastructure)
// Review and adjust before applying.
