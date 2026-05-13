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

use synq_core::{PeerInfo, SynqResult, SynqError};
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
    reconnect: ReconnectState,
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
            reconnect: ReconnectState::new(ReconnectConfig::default()),
            local_private_key,
        })
    }

    /// Start the background task to continuously monitor for peers.
    pub fn start_discovery_monitor(&self, app_handle: tauri::AppHandle) {
        let peers_clone = self.discovered_peers.clone();
        let discovery_clone = self.discovery.clone();
        let app_clone = app_handle.clone();

        info!("Starting background discovery monitor...");

        tokio::spawn(async move {
            match discovery_clone.browse() {
                Ok(receiver) => {
                    info!("mDNS browser active. Waiting for events...");
                    while let Ok(event) = receiver.recv() {
                        match event {
                            mdns_sd::ServiceEvent::ServiceResolved(info) => {
                                info!("Service resolved: {:?}", info.get_fullname());
                                if let Some(peer) = discovery::info_to_peer(&info) {
                                    let mut peers = peers_clone.lock().await;
                                    if !peers.iter().any(|p| p.device_id == peer.device_id) {
                                        info!("NEW PEER DETECTED: {} ({}) at {}", peer.name, peer.device_id, peer.address.as_deref().unwrap_or("unknown"));
                                        peers.push(peer.clone());
                                        let _ = app_clone.emit("peer-discovered", peer);
                                    } else {
                                        info!("Known peer updated: {}", peer.name);
                                    }
                                } else {
                                    warn!("Failed to parse peer info from mDNS record");
                                }
                            }
                            mdns_sd::ServiceEvent::ServiceFound(service_type, instance_name) => {
                                info!("Service found (not yet resolved): {} on {}", instance_name, service_type);
                            }
                            mdns_sd::ServiceEvent::ServiceRemoved(_service_type, instance_name) => {
                                let mut peers = peers_clone.lock().await;
                                peers.retain(|p| p.device_id.0.to_string() != instance_name);
                                info!("Peer removed: {}", instance_name);
                                let _ = app_clone.emit("peer-removed", instance_name);
                            }
                            mdns_sd::ServiceEvent::SearchStarted(service_type) => {
                                info!("mDNS search started for {}", service_type);
                            }
                            _ => {
                                // Log other events for debugging
                                info!("mDNS discovery event: {:?}", event);
                            }
                        }
                    }
                    warn!("mDNS receiver channel closed");
                }
                Err(e) => {
                    error!("CRITICAL: Failed to start mDNS browser: {:?}", e);
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
