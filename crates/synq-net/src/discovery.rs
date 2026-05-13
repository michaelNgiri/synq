//! LAN peer discovery via mDNS.
//!
//! Broadcasts `_synq._tcp.local` and listens for peer announcements.

use std::collections::HashMap;
use mdns_sd::{ServiceDaemon, ServiceInfo, ServiceEvent};
use synq_core::{PeerInfo, SynqResult, SynqError, DeviceId, Platform};
use uuid::Uuid;

/// mDNS service type for Synq peer discovery.
pub const SERVICE_TYPE: &str = "_synq._tcp.local.";

/// Discovery service that finds Synq peers on the local network.
#[derive(Clone)]
pub struct MdnsDiscovery {
    daemon: ServiceDaemon,
}

impl MdnsDiscovery {
    /// Create a new mDNS discovery service.
    pub fn new() -> SynqResult<Self> {
        let daemon = ServiceDaemon::new()
            .map_err(|e| SynqError::Discovery(format!("Failed to start mDNS daemon: {e}")))?;
        
        tracing::info!("Initializing mDNS discovery for {}", SERVICE_TYPE);
        Ok(Self { daemon })
    }

    /// Register this device on the network for peer discovery.
    pub fn register(&self, local: &PeerInfo) -> SynqResult<()> {
        let device_id = local.device_id.0.to_string();
        let name = &local.name;
        
        let mut properties = HashMap::new();
        properties.insert("id".to_string(), device_id.clone());
        properties.insert("name".to_string(), name.clone());
        properties.insert("platform".to_string(), format!("{:?}", local.platform));
        properties.insert("width".to_string(), local.screen.width.to_string());
        properties.insert("height".to_string(), local.screen.height.to_string());

        let service_info = ServiceInfo::new(
            SERVICE_TYPE,
            &device_id, // instance name is device ID
            &format!("{}.local.", device_id),
            "0.0.0.0", // will be auto-filled by daemon
            52820,     // Default Synq port
            Some(properties),
        ).map_err(|e| SynqError::Discovery(format!("Failed to create service info: {e}")))?;

        self.daemon.register(service_info)
            .map_err(|e| SynqError::Discovery(format!("Failed to register service: {e}")))?;

        tracing::info!("Registered mDNS service for {}", name);
        Ok(())
    }

    /// Browse for available Synq peers on the LAN.
    /// 
    /// This returns a receiver that yields ServiceEvents.
    pub fn browse(&self) -> SynqResult<mdns_sd::Receiver<ServiceEvent>> {
        self.daemon.browse(SERVICE_TYPE)
            .map_err(|e| SynqError::Discovery(format!("Failed to start browsing: {e}")))
    }

    /// Stop the discovery service.
    pub fn stop(&self) {
        tracing::info!("mDNS discovery stopped");
        let _ = self.daemon.shutdown();
    }
}

/// Helper to convert ServiceInfo to PeerInfo
pub fn info_to_peer(info: &ServiceInfo) -> Option<PeerInfo> {
    let props = info.get_properties();
    
    let id_str = props.get("id")?.to_string();
    let device_id = DeviceId(Uuid::parse_str(&id_str).ok()?);
    
    let name = props.get("name")?.to_string();
    
    let platform_str = props.get("platform")?.to_string();
    let platform = if platform_str.contains("MacOS") {
        Platform::MacOS
    } else {
        Platform::Windows
    };

    let width = props.get("width")?.to_string().parse().ok()?;
    let height = props.get("height")?.to_string().parse().ok()?;
    
    // Get the first IP address found
    let address = info.get_addresses().iter().next()?.to_string();
    let port = info.get_port();

    Some(PeerInfo {
        device_id,
        name,
        platform,
        screen: synq_core::ScreenGeometry {
            width,
            height,
            x: 0,
            y: 0,
        },
        address: Some(format!("{}:{}", address, port)),
    })
}
