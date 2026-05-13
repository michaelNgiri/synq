//! Application configuration for Synq.

use serde::{Deserialize, Serialize};

use crate::types::{DeviceId, Edge};

/// Top-level Synq configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynqConfig {
    /// This device's persistent identity.
    pub device_id: DeviceId,
    /// Human-readable device name (e.g. "MacBook Pro").
    pub device_name: String,
    /// Screen arrangement — which edge connects to which peer.
    pub layout: ScreenLayout,
    /// Hotkey bindings.
    pub hotkeys: HotkeyConfig,
    /// Network settings.
    pub network: NetworkConfig,
}

/// Defines how this device relates to its peers spatially.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenLayout {
    /// Which edge of this screen leads to a peer, and which peer.
    pub connections: Vec<EdgeConnection>,
}

/// Maps a screen edge to a peer device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeConnection {
    pub edge: Edge,
    pub peer_device_id: DeviceId,
}

/// Hotkey configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    /// Emergency kill-switch (default: Ctrl+Shift+Escape).
    pub kill_switch: String,
    /// Toggle Synq on/off (default: Ctrl+Shift+S).
    pub toggle: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            kill_switch: "ctrl+shift+escape".into(),
            toggle: "ctrl+shift+s".into(),
        }
    }
}

/// Network configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Enable mDNS LAN discovery.
    pub mdns_enabled: bool,
    /// Optional signaling server URL for internet relay.
    pub signaling_server: Option<String>,
    /// STUN servers for NAT traversal.
    pub stun_servers: Vec<String>,
    /// Optional TURN server for symmetric NAT.
    pub turn_server: Option<TurnConfig>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            mdns_enabled: true,
            signaling_server: None,
            stun_servers: vec!["stun:stun.l.google.com:19302".into()],
            turn_server: None,
        }
    }
}

/// TURN server credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnConfig {
    pub url: String,
    pub username: String,
    pub password: String,
}
