//! synq-net — Networking layer for peer discovery, encrypted transport, and reconnection.

pub mod discovery;
pub mod noise;
pub mod reconnect;
pub mod transport;
pub mod signaling;

use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::Mutex;
use tracing::{info, error};
use tauri::Emitter;

use synq_core::{PeerInfo, SynqResult, SynqError, DeviceId};
pub use reconnect::{ReconnectState, ReconnectConfig};
pub use transport::WebRtcTransport;
pub use noise::{NoiseHandshake, NoiseSession};
pub use signaling::SignalingMessage;

/// The core networking trait.
#[async_trait]
pub trait NetLayer: Send + Sync {
    async fn discover_peers(&self) -> SynqResult<Vec<PeerInfo>>;
    async fn connect(&mut self, peer: &PeerInfo) -> SynqResult<()>;
    async fn send(&self, msg: &[u8], reliable: bool) -> SynqResult<()>;
    async fn recv(&self) -> SynqResult<Vec<u8>>;
    async fn disconnect(&self) -> SynqResult<()>;
    fn is_connected(&self) -> bool;
    fn state(&self) -> ConnectionState;
}

/// Connection lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Handshaking,
    Connected,
    Reconnecting { attempts: u32 },
}

/// Standard implementation of the Synq network layer.
pub struct SynqNetLayer {
    discovery: discovery::MdnsDiscovery,
    discovered_peers: Arc<Mutex<Vec<PeerInfo>>>,
    transport: Option<Arc<WebRtcTransport>>,
    noise: Option<Arc<Mutex<NoiseSession>>>,
    local_private_key: Vec<u8>,
    local_device_id: DeviceId,
    signaling_tx: Arc<Mutex<Option<tokio::sync::mpsc::Sender<(SignalingMessage, String)>>>>,
    pending_signals: Arc<Mutex<std::collections::HashMap<DeviceId, tokio::sync::mpsc::Sender<SignalingMessage>>>>,
}

impl SynqNetLayer {
    pub fn new(local_device_id: DeviceId, local_private_key: Vec<u8>) -> SynqResult<Self> {
        let discovery = discovery::MdnsDiscovery::new()?;
        let discovered_peers: Arc<Mutex<Vec<PeerInfo>>> = Arc::new(Mutex::new(Vec::new()));

        Ok(Self {
            discovery,
            discovered_peers,
            transport: None,
            noise: None,
            local_private_key,
            local_device_id,
            signaling_tx: Arc::new(Mutex::new(None)),
            pending_signals: Arc::new(Mutex::new(std::collections::HashMap::new())),
        })
    }

    /// Start the background task to continuously monitor for peers.
    pub fn start_discovery_monitor(&self, app_handle: tauri::AppHandle, device_id: DeviceId, name: String) {
        let peers_clone = self.discovered_peers.clone();
        let discovery_clone = self.discovery.clone();
        let app_clone = app_handle.clone();
        let daemon_peers = self.discovered_peers.clone();
        let daemon_app = app_handle.clone();
        let pending_signals_clone = self.pending_signals.clone();
        
        let (sig_tx, mut sig_rx) = tokio::sync::mpsc::channel::<(SignalingMessage, String)>(100);
        let signaling_tx_lock = self.signaling_tx.clone();
        let sig_tx_init = signaling_tx_lock.clone();

        // Update signaling_tx in state
        tauri::async_runtime::spawn(async move {
            let mut lock = sig_tx_init.lock().await;
            *lock = Some(sig_tx);
        });

        info!("Starting background discovery monitor (mDNS + UDP Broadcast)...");

        // --- Task 1: mDNS Discovery (Standard) ---
        tokio::spawn(async move {
            if let Ok(receiver) = discovery_clone.browse() {
                while let Ok(event) = receiver.recv() {
                    match event {
                        mdns_sd::ServiceEvent::ServiceResolved(info) => {
                            if let Some(peer) = discovery::info_to_peer(&info) {
                                if peer.device_id != device_id {
                                    let mut peers = peers_clone.lock().await;
                                    if !peers.iter().any(|p| p.device_id == peer.device_id) {
                                        info!("mDNS Peer Discovered: {} ({})", peer.name, peer.device_id);
                                        peers.push(peer.clone());
                                        let _ = app_clone.emit("peer-discovered", peer);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        });

        // --- Task 2: UDP Broadcast & Signaling ---
        let signaling_tx_task2 = signaling_tx_lock.clone();
        tokio::spawn(async move {
            use tokio::net::UdpSocket;
            let socket = UdpSocket::bind("0.0.0.0:52821").await.ok();
            if let Some(socket) = socket {
                let socket = Arc::new(socket);
                if socket.set_broadcast(true).is_err() {
                    error!("Failed to set UDP broadcast mode. Subnet discovery may be limited.");
                }
                let mut buf = [0u8; 4096];
                
                loop {
                    let socket_recv = socket.clone();
                    let pending_signals_inner = pending_signals_clone.clone();
                    let sig_tx_inner_main = signaling_tx_task2.clone();
                    
                    tokio::select! {
                        result = socket_recv.recv_from(&mut buf) => {
                            if let Ok((len, addr)) = result {
                                let data = &buf[..len];
                                if let Ok(msg) = serde_json::from_slice::<SignalingMessage>(data) {
                                    match msg {
                                        SignalingMessage::Offer { sdp, from } => {
                                            if from != device_id {
                                                info!("Incoming connection offer from {} ({})", from, addr);
                                                let sig_tx_inner = sig_tx_inner_main.clone();
                                                let local_id_inner = device_id;
                                                let target_ip = addr.ip().to_string();
                                                tokio::spawn(async move {
                                                    match WebRtcTransport::new(vec!["stun:stun.l.google.com:19302".into()]).await {
                                                        Ok(transport) => {
                                                            let transport = Arc::new(transport);
                                                            let sig_tx_inner_clone = sig_tx_inner.clone();
                                                            let target_ip_clone = target_ip.clone();
                                                            transport.on_ice_candidate(move |candidate| {
                                                                let sig_tx = sig_tx_inner_clone.clone();
                                                                let tip = target_ip_clone.clone();
                                                                tokio::spawn(async move {
                                                                    let lock = sig_tx.lock().await;
                                                                    if let Some(tx) = lock.as_ref() {
                                                                        let _ = tx.send((SignalingMessage::IceCandidate { candidate, from: local_id_inner }, tip)).await;
                                                                    }
                                                                });
                                                            });

                                                            if let Ok(answer_sdp) = transport.create_answer(&sdp).await {
                                                                let lock = sig_tx_inner.lock().await;
                                                                if let Some(tx) = lock.as_ref() {
                                                                    let _ = tx.send((SignalingMessage::Answer { sdp: answer_sdp, from: local_id_inner }, target_ip)).await;
                                                                }
                                                            }
                                                        }
                                                        Err(e) => error!("Failed to create responder transport: {}", e),
                                                    }
                                                });
                                            }
                                        }
                                        SignalingMessage::IceCandidate { candidate, from } => {
                                            if from != device_id {
                                                let pending = pending_signals_inner.lock().await;
                                                if let Some(tx) = pending.get(&from) {
                                                    let _ = tx.try_send(SignalingMessage::IceCandidate { candidate, from });
                                                }
                                            }
                                        }
                                        SignalingMessage::Answer { from, .. } => {
                                            if from != device_id {
                                                let pending = pending_signals_inner.lock().await;
                                                if let Some(tx) = pending.get(&from) {
                                                    let _ = tx.try_send(msg);
                                                }
                                            }
                                        }
                                    }
                                } 
                                else if let Ok(peer) = serde_json::from_slice::<PeerInfo>(data) {
                                    if peer.device_id != device_id {
                                        let mut peers = daemon_peers.lock().await;
                                        if !peers.iter().any(|p| p.device_id == peer.device_id) {
                                            info!("UDP Shout Received: {} ({})", peer.name, peer.device_id);
                                            peers.push(peer.clone());
                                            let _ = daemon_app.emit("peer-discovered", peer);
                                        }
                                    }
                                }
                            }
                        }
                        Some((msg, target_ip)) = sig_rx.recv() => {
                            if let Ok(data) = serde_json::to_vec(&msg) {
                                let target_addr = if target_ip.contains(":") { target_ip } else { format!("{}:52821", target_ip) };
                                let _ = socket.send_to(&data, &target_addr).await;
                            }
                        }
                        _ = tokio::time::sleep(tokio::time::Duration::from_secs(3)) => {
                            let ip = local_ip_address::local_ip().ok();
                            if let Some(ip) = ip {
                                let local_peer = PeerInfo {
                                    device_id,
                                    name: name.clone(),
                                    platform: if cfg!(target_os = "macos") { synq_core::Platform::MacOS } else { synq_core::Platform::Windows },
                                    screen: synq_core::ScreenGeometry { width: 0, height: 0, x: 0, y: 0 },
                                    address: Some(format!("{}:52821", ip)),
                                };
                                if let Ok(data) = serde_json::to_vec(&local_peer) {
                                    let _ = socket.send_to(&data, "255.255.255.255:52821").await;
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    pub fn register_local(&self, peer: &PeerInfo) -> SynqResult<()> {
        info!("Registering local peer: {} ({})", peer.name, peer.device_id);
        Ok(())
    }

    pub async fn perform_handshake(&self, transport: Arc<WebRtcTransport>, initiator: bool) -> SynqResult<NoiseSession> {
        let mut handshake = if initiator {
            NoiseHandshake::new_initiator(&self.local_private_key)?
        } else {
            NoiseHandshake::new_responder(&self.local_private_key)?
        };

        if initiator {
            let msg = handshake.write_message(&[])?;
            transport.send_reliable(&msg).await?;
            let incoming = transport.recv().await?;
            let _ = handshake.read_message(&incoming)?;
        } else {
            let incoming = transport.recv().await?;
            let _ = handshake.read_message(&incoming)?;
            let msg = handshake.write_message(&[])?;
            transport.send_reliable(&msg).await?;
        }

        Ok(handshake.into_transport()?)
    }
}

#[async_trait]
impl NetLayer for SynqNetLayer {
    async fn discover_peers(&self) -> SynqResult<Vec<PeerInfo>> {
        let peers = self.discovered_peers.lock().await;
        Ok(peers.clone())
    }

    async fn connect(&mut self, peer: &PeerInfo) -> SynqResult<()> {
        info!("Connecting to peer: {} at {:?}", peer.name, peer.address);
        
        let target_ip = peer.address.clone().ok_or(SynqError::Connection("No address for peer".into()))?
            .split(':').next().unwrap().to_string();

        let transport = Arc::new(WebRtcTransport::new(vec!["stun:stun.l.google.com:19302".into()]).await?);
        let sig_tx_lock = self.signaling_tx.clone();
        let local_id = self.local_device_id;
        let target_ip_clone = target_ip.clone();
        
        transport.on_ice_candidate(move |candidate| {
            let sig_tx = sig_tx_lock.clone();
            let tip = target_ip_clone.clone();
            tokio::spawn(async move {
                let lock = sig_tx.lock().await;
                if let Some(tx) = lock.as_ref() {
                    let _ = tx.send((SignalingMessage::IceCandidate { candidate, from: local_id }, tip)).await;
                }
            });
        });

        let (p_tx, mut p_rx) = tokio::sync::mpsc::channel(10);
        {
            let mut pending = self.pending_signals.lock().await;
            pending.insert(peer.device_id, p_tx);
        }

        let offer_sdp = transport.create_offer().await?;
        {
            let sig_tx = self.signaling_tx.lock().await;
            if let Some(tx) = sig_tx.as_ref() {
                tx.send((SignalingMessage::Offer { sdp: offer_sdp, from: local_id }, target_ip.clone())).await
                    .map_err(|e| SynqError::Connection(e.to_string()))?;
            }
        }

        info!("Waiting for answer from {}...", peer.name);
        let transport_clone = transport.clone();
        loop {
            tokio::select! {
                msg = p_rx.recv() => {
                    match msg {
                        Some(SignalingMessage::Answer { sdp, .. }) => {
                            transport_clone.accept_answer(&sdp).await?;
                            info!("WebRTC Answer accepted from {}", peer.name);
                        }
                        Some(SignalingMessage::IceCandidate { candidate, .. }) => {
                            let _ = transport_clone.add_ice_candidate(&candidate).await;
                        }
                        _ => {}
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(10)) => {
                    if !transport_clone.is_connected() {
                        return Err(SynqError::Connection("Handshake timeout".into()));
                    } else { break; }
                }
            }
            if transport_clone.is_connected() { break; }
        }

        {
            let mut pending = self.pending_signals.lock().await;
            pending.remove(&peer.device_id);
        }

        let noise_session = self.perform_handshake(transport.clone(), true).await?;
        self.transport = Some(transport);
        self.noise = Some(Arc::new(Mutex::new(noise_session)));
        
        info!("Successfully connected and encrypted with {}!", peer.name);
        Ok(())
    }

    async fn send(&self, msg: &[u8], reliable: bool) -> SynqResult<()> {
        if let (Some(transport), Some(noise)) = (&self.transport, &self.noise) {
            let mut session = noise.lock().await;
            let encrypted = session.encrypt(msg)?;
            if reliable {
                transport.send_reliable(&encrypted).await
            } else {
                transport.send_unreliable(&encrypted).await
            }
        } else {
            Err(SynqError::Connection("Not connected".into()))
        }
    }

    async fn recv(&self) -> SynqResult<Vec<u8>> {
        if let (Some(transport), Some(noise)) = (&self.transport, &self.noise) {
            let data = transport.recv().await?;
            let mut session = noise.lock().await;
            session.decrypt(&data)
        } else {
            Err(SynqError::Connection("Not connected".into()))
        }
    }

    async fn disconnect(&self) -> SynqResult<()> {
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.transport.as_ref().map(|t| t.is_connected()).unwrap_or(false)
    }

    fn state(&self) -> ConnectionState {
        if self.is_connected() { ConnectionState::Connected } else { ConnectionState::Disconnected }
    }
}
