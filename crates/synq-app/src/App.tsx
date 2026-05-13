import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

function App() {
  const [deviceId, setDeviceId] = useState<string>("Loading...");

  interface PeerInfo {
    device_id: { 0: string };
    name: string;
    platform: string;
    address: string;
  }

  const [peers, setPeers] = useState<PeerInfo[]>([]);
  const [isDiscovering, setIsDiscovering] = useState(false);

  useEffect(() => {
    async function fetchDeviceId() {
      try {
        const id = await invoke<string>("get_device_info");
        setDeviceId(id);
      } catch (e) {
        console.error(e);
        setDeviceId("Error");
      }
    }
    fetchDeviceId();
  }, []);

  async function handleEmergencyKill() {
    await invoke("emergency_kill");
    alert("Emergency Kill-switch activated! All input injection halted.");
  }

  async function handleDiscovery() {
    setIsDiscovering(true);
    try {
      const foundPeers = await invoke<PeerInfo[]>("start_discovery");
      setPeers(foundPeers);
    } catch (e) {
      console.error(e);
    } finally {
      setIsDiscovering(false);
    }
  }

  return (
    <main className="container">
      <header>
        <h1>Synq Engine</h1>
        <div className="device-badge">
          <span>Local Device ID:</span>
          <strong>{deviceId}</strong>
        </div>
      </header>

      <section className="card">
        <div className="status-info">
          <div className="pulse-dot"></div>
          <p>Continuity daemon active & running</p>
        </div>
        
        <button 
          className="btn-primary" 
          onClick={handleDiscovery} 
          disabled={isDiscovering}
        >
          {isDiscovering ? (
            <>🔍 Searching Network...</>
          ) : (
            <>🌐 Start Discovery</>
          )}
        </button>
      </section>

      {peers.length > 0 && (
        <section className="peers-section">
          <h3 style={{ color: 'var(--text-secondary)', fontSize: '0.9rem', paddingLeft: '8px' }}>
            DISCOVERED PEERS
          </h3>
          {peers.map((peer) => (
            <div key={peer.device_id[0]} className="peer-card">
              <div className="peer-info">
                <h4>{peer.name}</h4>
                <span>{peer.platform} • {peer.address}</span>
              </div>
              <button className="connect-badge" onClick={() => alert("Connecting...")}>
                Connect
              </button>
            </div>
          ))}
        </section>
      )}

      <footer className="card kill-switch" style={{ marginTop: 'auto', gap: '12px', padding: '24px' }}>
        <p style={{ fontSize: '0.85rem', color: 'var(--text-secondary)', textAlign: 'center' }}>
          Safety override for all input injection
        </p>
        <button className="btn-danger" onClick={handleEmergencyKill}>
          🛑 Emergency Kill-switch
        </button>
      </footer>
    </main>
  );
}

export default App;
