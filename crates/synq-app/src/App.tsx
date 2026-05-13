import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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
    async function init() {
      try {
        const id = await invoke<string>("get_device_info");
        setDeviceId(id);
      } catch (e) {
        console.error(e);
      }
    }
    init();

    // Listen for discovery events from backend
    const unlistenDiscovered = listen<PeerInfo>("peer-discovered", (event) => {
      setPeers((prev) => {
        if (prev.find((p) => p.device_id[0] === event.payload.device_id[0])) return prev;
        return [...prev, event.payload];
      });
    });

    const unlistenRemoved = listen<string>("peer-removed", (event) => {
      setPeers((prev) => prev.filter((p) => p.device_id[0] !== event.payload));
    });

    return () => {
      unlistenDiscovered.then((f) => f());
      unlistenRemoved.then((f) => f());
    };
  }, []);

  async function handleEmergencyKill() {
    await invoke("emergency_kill");
    alert("Emergency Kill-switch activated! All input injection halted.");
  }

  async function handleDiscovery() {
    setIsDiscovering(true);
    try {
      // Trigger registration and get current list
      const foundPeers = await invoke<PeerInfo[]>("start_discovery");
      setPeers(foundPeers);
    } catch (e) {
      console.error(e);
    }
    // Note: We keep isDiscovering true to show the searching state
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

      {!isDiscovering && peers.length === 0 ? (
        <section className="card">
          <div className="status-info">
            <div className="pulse-dot"></div>
            <p>Continuity daemon active & running</p>
          </div>
          
          <button className="btn-primary" onClick={handleDiscovery}>
            🌐 Start Discovery
          </button>
        </section>
      ) : (
        <section className="peers-section">
          {isDiscovering && (
            <div className="searching-container">
              <div className="searching-rings">
                <div className="ring"></div>
                <div className="ring"></div>
                <div className="ring"></div>
                <span style={{ fontSize: '1.5rem' }}>📡</span>
              </div>
              <p style={{ color: 'var(--text-secondary)', fontSize: '0.9rem' }}>
                Searching for nearby devices...
              </p>
            </div>
          )}

          {peers.length > 0 && (
            <>
              <h3 style={{ color: 'var(--text-secondary)', fontSize: '0.9rem', paddingLeft: '8px', marginTop: '20px' }}>
                DISCOVERED PEERS
              </h3>
              {peers.map((peer) => (
                <div key={peer.device_id[0]} className="peer-card">
                  <div className="peer-info">
                    <h4>{peer.name}</h4>
                    <span>{peer.platform} • {peer.address || "Local Network"}</span>
                  </div>
                  <button className="connect-badge" onClick={() => alert("Connecting...")}>
                    Connect
                  </button>
                </div>
              ))}
            </>
          )}

          <button 
            style={{ marginTop: '20px', background: 'transparent', border: '1px solid var(--border-color)', color: 'var(--text-secondary)' }}
            onClick={() => setIsDiscovering(false)}
          >
            Stop Searching
          </button>
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
