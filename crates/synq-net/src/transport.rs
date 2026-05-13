//! WebRTC DataChannel transport layer.
//!
//! Provides both reliable (clipboard) and unreliable (input) channels
//! over a single peer connection.

use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_init::RTCDataChannelInit;
use webrtc::data_channel::RTCDataChannel;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::ice_transport::ice_server::RTCIceServer;

use synq_core::{SynqError, SynqResult};

/// WebRTC transport wrapper managing peer connection and data channels.
pub struct WebRtcTransport {
    peer_connection: Arc<RTCPeerConnection>,
    reliable_channel: Arc<RTCDataChannel>,
    unreliable_channel: Arc<RTCDataChannel>,
    message_rx: Mutex<mpsc::Receiver<Vec<u8>>>,
}

impl WebRtcTransport {
    /// Initialize a new WebRTC transport.
    pub async fn new(stun_servers: Vec<String>) -> SynqResult<Self> {
        let mut m = MediaEngine::default();
        m.register_default_codecs().map_err(|e| SynqError::Connection(e.to_string()))?;

        let api = APIBuilder::new()
            .with_media_engine(m)
            .build();

        let config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: stun_servers,
                ..Default::default()
            }],
            ..Default::default()
        };

        let pc = Arc::new(api.new_peer_connection(config).await
            .map_err(|e| SynqError::Connection(e.to_string()))?);

        // Create data channels
        let reliable_init = RTCDataChannelInit {
            ordered: Some(true),
            ..Default::default()
        };
        let reliable_dc = pc.create_data_channel("synq-reliable", Some(reliable_init)).await
            .map_err(|e| SynqError::Connection(e.to_string()))?;

        let unreliable_init = RTCDataChannelInit {
            ordered: Some(false),
            max_retransmits: Some(0),
            ..Default::default()
        };
        let unreliable_dc = pc.create_data_channel("synq-unreliable", Some(unreliable_init)).await
            .map_err(|e| SynqError::Connection(e.to_string()))?;

        let (tx, rx) = mpsc::channel(1000);
        let tx = Arc::new(tx);

        // Setup message handlers for both channels
        Self::setup_dc_handlers(&reliable_dc, tx.clone());
        Self::setup_dc_handlers(&unreliable_dc, tx.clone());

        Ok(Self {
            peer_connection: pc,
            reliable_channel: reliable_dc,
            unreliable_channel: unreliable_dc,
            message_rx: Mutex::new(rx),
        })
    }

    fn setup_dc_handlers(dc: &Arc<RTCDataChannel>, tx: Arc<mpsc::Sender<Vec<u8>>>) {
        let dc_label = dc.label().to_string();
        dc.on_message(Box::new(move |msg| {
            let tx = tx.clone();
            let data = msg.data.to_vec();
            let label = dc_label.clone();
            Box::pin(async move {
                if let Err(_) = tx.send(data).await {
                    tracing::error!("Failed to forward message from channel {}", label);
                }
            })
        }));
    }

    /// Create an offer for the signaling exchange.
    pub async fn create_offer(&self) -> SynqResult<String> {
        let offer = self.peer_connection.create_offer(None).await
            .map_err(|e| SynqError::Connection(e.to_string()))?;
        
        self.peer_connection.set_local_description(offer.clone()).await
            .map_err(|e| SynqError::Connection(e.to_string()))?;

        Ok(serde_json::to_string(&offer)?)
    }

    /// Accept a remote SDP answer.
    pub async fn accept_answer(&self, sdp_json: &str) -> SynqResult<()> {
        let answer: RTCSessionDescription = serde_json::from_str(sdp_json)?;
        self.peer_connection.set_remote_description(answer).await
            .map_err(|e| SynqError::Connection(e.to_string()))?;
        Ok(())
    }

    /// Send data on the unreliable channel (for input events).
    pub async fn send_unreliable(&self, data: &[u8]) -> SynqResult<()> {
        let bytes = bytes::Bytes::copy_from_slice(data);
        self.unreliable_channel.send(&bytes).await
            .map_err(|e| SynqError::Send(e.to_string()))?;
        Ok(())
    }

    /// Send data on the reliable channel (for clipboard sync).
    pub async fn send_reliable(&self, data: &[u8]) -> SynqResult<()> {
        let bytes = bytes::Bytes::copy_from_slice(data);
        self.reliable_channel.send(&bytes).await
            .map_err(|e| SynqError::Send(e.to_string()))?;
        Ok(())
    }

    /// Receive data from any channel.
    pub async fn recv(&self) -> SynqResult<Vec<u8>> {
        let mut rx = self.message_rx.lock().await;
        rx.recv().await.ok_or(SynqError::Disconnected)
    }

    /// Check if the connection is open.
    pub fn is_connected(&self) -> bool {
        self.peer_connection.connection_state() == RTCPeerConnectionState::Connected
    }

    /// Close the peer connection.
    pub async fn close(&self) -> SynqResult<()> {
        self.peer_connection.close().await
            .map_err(|e| SynqError::Connection(e.to_string()))?;
        Ok(())
    }
}
