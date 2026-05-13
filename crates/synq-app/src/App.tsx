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

  const [localIp, setLocalIp] = useState<string | null>(null);

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
  }

  const [manualIp, setManualIp] = useState("");
  const [showManual, setShowManual] = useState(false);

  async function handleManualConnect() {
    if (!manualIp) return;
    try {
      // For now, we'll just simulate a connection or add it to the list
      const mockPeer: PeerInfo = {
        device_id: { 0: "manual-link" },
        name: `Remote Device (${manualIp})`,
        platform: "Remote",
        address: manualIp
      };
      setPeers(prev => [...prev, mockPeer]);
      alert(`Manual link established with ${manualIp}. Starting handshake...`);
    } catch (e) {
      console.error(e);
    }
  }

  async function handleRevealIp() {
    try {
      const ip = await invoke<string>("get_local_ip");
      setLocalIp(ip);
    } catch (e) {
      console.error(e);
    }
  }

  return (
    <main className="container">
      <header>
        <h1>Synq Engine</h1>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', alignItems: 'center' }}>
          <div className="device-badge">
            <span>Local Device ID:</span>
            <strong>{deviceId}</strong>
          </div>
          {localIp ? (
            <div className="device-badge" style={{ cursor: 'pointer' }} onClick={() => {
              navigator.clipboard.writeText(localIp);
              alert("IP Copied to clipboard!");
            }}>
              <span>Local IP:</span>
              <strong>{localIp} (Click to copy)</strong>
            </div>
          ) : (
            <button 
              onClick={handleRevealIp}
              style={{ background: 'transparent', border: 'none', color: 'var(--accent-color)', fontSize: '0.8rem', textDecoration: 'underline', cursor: 'pointer' }}
            >
              Show Local IP for Manual Link
            </button>
          )}
        </div>
      </header>

      {!isDiscovering && peers.length === 0 && !showManual ? (
        <section className="card">
          <div className="status-info">
            <div className="pulse-dot"></div>
            <p>Continuity daemon active & running</p>
          </div>
          
          <div style={{ display: 'flex', flexDirection: 'column', gap: '12px', width: '100%' }}>
            <button className="btn-primary" onClick={handleDiscovery}>
              🌐 Start Discovery
            </button>
            <button 
              style={{ background: 'transparent', border: '1px solid var(--border-color)', color: 'var(--text-secondary)' }}
              onClick={() => setShowManual(true)}
            >
              ⌨️ Connect Manually
            </button>
          </div>
        </section>
      ) : (
        <section className="peers-section">
          {showManual && (
            <div className="card" style={{ marginBottom: '20px', animation: 'fadeIn 0.3s ease' }}>
              <h3 style={{ fontSize: '0.9rem', marginBottom: '12px' }}>Enter Device IP</h3>
              <div style={{ display: 'flex', gap: '8px' }}>
                <input 
                  type="text" 
                  placeholder="e.g. 192.168.1.50" 
                  value={manualIp}
                  onChange={(e) => setManualIp(e.target.value)}
                  style={{ flex: 1, padding: '10px', borderRadius: '8px', border: '1px solid var(--border-color)', background: 'rgba(0,0,0,0.2)', color: 'white' }}
                />
                <button className="btn-primary" style={{ padding: '0 20px' }} onClick={handleManualConnect}>
                  Link
                </button>
              </div>
              <button 
                onClick={() => setShowManual(false)}
                style={{ marginTop: '12px', background: 'transparent', border: 'none', color: 'var(--text-secondary)', fontSize: '0.8rem' }}
              >
                ← Back to Discovery
              </button>
            </div>
          )}

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
        <div style={{ display: 'flex', justifyContent: 'center', gap: '12px' }}>
          <span className="badge">Phase 1: Shell</span>
          <span className="badge" style={{ backgroundColor: 'rgba(255, 255, 255, 0.1)', color: 'var(--text-secondary)' }}>v0.1.0</span>
        </div>
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
