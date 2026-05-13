//! synq-net — Networking layer for peer discovery, encrypted transport, and reconnection.

pub mod discovery;
pub mod noise;
pub mod reconnect;
pub mod transport;

use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::Mutex;
use tracing::{info, warn, error};
use tauri::Emitter;

use synq_core::{PeerInfo, SynqResult, SynqError, DeviceId};
pub use reconnect::{ReconnectState, ReconnectConfig};
pub use transport::WebRtcTransport;
pub use noise::{NoiseHandshake, NoiseSession};

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
    // reconnect: ReconnectState, // Removed as it is currently unused
    local_private_key: Vec<u8>,
}

impl SynqNetLayer {
    pub fn new(local_private_key: Vec<u8>) -> SynqResult<Self> {
        let discovery = discovery::MdnsDiscovery::new()?;
        let discovered_peers: Arc<Mutex<Vec<PeerInfo>>> = Arc::new(Mutex::new(Vec::new()));

        Ok(Self {
            discovery,
            discovered_peers,
            transport: None,
            noise: None,
            // reconnect: ReconnectState::new(ReconnectConfig::default()),
            local_private_key,
        })
    }

    /// Start the background task to continuously monitor for peers.
    pub fn start_discovery_monitor(&self, app_handle: tauri::AppHandle, device_id: DeviceId, name: String) {
        let peers_clone = self.discovered_peers.clone();
        let discovery_clone = self.discovery.clone();
        let app_clone = app_handle.clone();
        let daemon_peers = self.discovered_peers.clone();
        let daemon_app = app_handle.clone();

        info!("Starting background discovery monitor (mDNS + UDP Broadcast)...");

        // --- Task 1: mDNS Discovery (Standard) ---
        tokio::spawn(async move {
            if let Ok(receiver) = discovery_clone.browse() {
                while let Ok(event) = receiver.recv() {
                    match event {
                        mdns_sd::ServiceEvent::ServiceResolved(info) => {
                            if let Some(peer) = discovery::info_to_peer(&info) {
                                let mut peers = peers_clone.lock().await;
                                if !peers.iter().any(|p| p.device_id == peer.device_id) {
                                    info!("mDNS Peer Discovered: {} ({})", peer.name, peer.device_id);
                                    peers.push(peer.clone());
                                    let _ = app_clone.emit("peer-discovered", peer);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        });

        // --- Task 2: UDP Broadcast Shout (Reliability Fallback) ---
        tokio::spawn(async move {
            use tokio::net::UdpSocket;
            let socket = UdpSocket::bind("0.0.0.0:52821").await.ok();
            if let Some(socket) = socket {
                socket.set_broadcast(true).unwrap();
                let mut buf = [0u8; 1024];
                
                loop {
                    tokio::select! {
                        // 1. Listen for beacons from others
                        result = socket.recv_from(&mut buf) => {
                            if let Ok((len, _addr)) = result {
                                if let Ok(peer) = serde_json::from_slice::<PeerInfo>(&buf[..len]) {
                                    let mut peers = daemon_peers.lock().await;
                                    if !peers.iter().any(|p| p.device_id == peer.device_id) {
                                        info!("UDP Shout Received: {} ({})", peer.name, peer.device_id);
                                        peers.push(peer.clone());
                                        let _ = daemon_app.emit("peer-discovered", peer);
                                    }
                                }
                            }
                        }
                        // 2. Periodically shout our own identity
                        _ = tokio::time::sleep(tokio::time::Duration::from_secs(3)) => {
                            let ip = local_ip_address::local_ip().ok();
                            if let Some(ip) = ip {
                                let local_peer = PeerInfo {
                                    device_id,
                                    name: name.clone(),
                                    platform: if cfg!(target_os = "macos") { synq_core::Platform::MacOS } else { synq_core::Platform::Windows },
                                    screen: synq_core::ScreenGeometry { width: 0, height: 0, x: 0, y: 0 },
                                    address: Some(format!("{}:52820", ip)),
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

    /// Perform the Noise XX handshake over WebRTC.
    async fn perform_handshake(&mut self, transport: Arc<WebRtcTransport>, is_initiator: bool) -> SynqResult<NoiseSession> {
        let mut handshake = if is_initiator {
            NoiseHandshake::new_initiator(&self.local_private_key)?
        } else {
            NoiseHandshake::new_responder(&self.local_private_key)?
        };

        // Handshake Loop (3 messages for XX)
        // This is a simplified version; real WebRTC signaling would be out-of-band.
        // For now, we assume the data channel is already open and use it for the handshake.
        
        if is_initiator {
            // Message 1 -> e
            let msg1 = handshake.write_message(&[])?;
            transport.send_reliable(&msg1).await?;
            
            // Message 2 <- e, ee, s, es
            let msg2 = transport.recv().await?;
            handshake.read_message(&msg2)?;
            
            // Message 3 -> s, se
            let msg3 = handshake.write_message(&[])?;
            transport.send_reliable(&msg3).await?;
        } else {
            // Message 1 <- e
            let msg1 = transport.recv().await?;
            handshake.read_message(&msg1)?;
            
            // Message 2 -> e, ee, s, es
            let msg2 = handshake.write_message(&[])?;
            transport.send_reliable(&msg2).await?;
            
            // Message 3 <- s, se
            let msg3 = transport.recv().await?;
            handshake.read_message(&msg3)?;
        }

        handshake.into_transport()
    }
    /// Register this device on the network.
    pub fn register_local(&self, local: &PeerInfo) -> SynqResult<()> {
        self.discovery.register(local)
    }
}

#[async_trait]
impl NetLayer for SynqNetLayer {
    async fn discover_peers(&self) -> SynqResult<Vec<PeerInfo>> {
        // Return the current snapshot of discovered peers
        Ok(self.discovered_peers.lock().await.clone())
    }

    async fn connect(&mut self, peer: &PeerInfo) -> SynqResult<()> {
        info!("Connecting to peer: {} at {:?}", peer.name, peer.address);
        
        let transport = Arc::new(WebRtcTransport::new(vec!["stun:stun.l.google.com:19302".into()]).await?);
        
        // TODO: Out-of-band signaling (SDP exchange) would happen here.
        // For Phase 1, we assume direct discovery/connection.
        
        let noise_session = self.perform_handshake(transport.clone(), true).await?;
        
        self.transport = Some(transport);
        self.noise = Some(Arc::new(Mutex::new(noise_session)));
        
        Ok(())
    }

    async fn send(&self, msg: &[u8], reliable: bool) -> SynqResult<()>{
        let transport = self.transport.as_ref().ok_or(SynqError::Disconnected)?;
        let noise = self.noise.as_ref().ok_or(SynqError::Handshake("No encrypted session".into()))?;
        
        let mut noise = noise.lock().await;
        let encrypted = noise.encrypt(msg)?;
        
        if reliable {
            transport.send_reliable(&encrypted).await
        } else {
            transport.send_unreliable(&encrypted).await
        }
    }

    async fn recv(&self) -> SynqResult<Vec<u8>> {
        let transport = self.transport.as_ref().ok_or(SynqError::Disconnected)?;
        let noise = self.noise.as_ref().ok_or(SynqError::Handshake("No encrypted session".into()))?;
        
        let ciphertext = transport.recv().await?;
        let mut noise = noise.lock().await;
        noise.decrypt(&ciphertext)
    }

    async fn disconnect(&self) -> SynqResult<()> {
        if let Some(transport) = &self.transport {
            transport.close().await?;
        }
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.transport.as_ref().map(|t| t.is_connected()).unwrap_or(false)
    }

    fn state(&self) -> ConnectionState {
        if self.is_connected() {
            ConnectionState::Connected
        } else {
            ConnectionState::Disconnected
        }
    }
}
