use std::time::Duration;
use synq_core::{PeerInfo, DeviceId, Platform, ScreenGeometry};
use synq_net::discovery::{MdnsDiscovery, info_to_peer};
use mdns_sd::ServiceEvent;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    
    println!("📡 Starting Synq Network Discovery Verification");
    
    let discovery = MdnsDiscovery::new()?;
    
    let local_peer = PeerInfo {
        device_id: DeviceId::new(),
        name: format!("Verify-Net-{}", whoami::username()),
        platform: Platform::MacOS,
        screen: ScreenGeometry { width: 1920, height: 1080, x: 0, y: 0 },
        address: None,
    };
    
    println!("🏠 Registering local device: {}", local_peer.name);
    discovery.register(&local_peer)?;
    
    println!("🔍 Browsing for peers (waiting 10 seconds)...");
    let receiver = discovery.browse()?;
    
    let (tx, mut rx) = mpsc::channel(100);

    // Spawn a dedicated thread to handle blocking mDNS receives
    std::thread::spawn(move || {
        while let Ok(event) = receiver.recv() {
            if tx.blocking_send(event).is_err() {
                break;
            }
        }
    });

    let timeout = tokio::time::sleep(Duration::from_secs(10));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            event = rx.recv() => {
                if let Some(event) = event {
                    match event {
                        ServiceEvent::ServiceResolved(info) => {
                            if let Some(peer) = info_to_peer(&info) {
                                if peer.device_id != local_peer.device_id {
                                    println!("✨ DISCOVERED PEER: {} ({}) at {:?}", peer.name, peer.device_id, peer.address);
                                } else {
                                    println!("👤 Discovered self: {}", peer.name);
                                }
                            }
                        }
                        ServiceEvent::SearchStarted(ty) => {
                            println!("🚀 Search started for {}", ty);
                        }
                        _ => {}
                    }
                }
            }
            _ = &mut timeout => {
                println!("⏰ Discovery timeout reached.");
                break;
            }
        }
    }
    
    discovery.stop();
    Ok(())
}
