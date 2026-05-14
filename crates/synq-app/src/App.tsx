import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

function App() {
  const [deviceId, setDeviceId] = useState<string>("Loading...");

  interface PeerInfo {
    device_id: string; // Backend expects string for Uuid tuple
    name: string;
    platform: 'MacOS' | 'Windows';
    screen: { width: number, height: number, x: number, y: number };
    address: string;
    status?: 'connecting' | 'connected' | 'error';
  }

  interface LogEntry {
    message: string;
    level: string;
    timestamp: number;
  }

  const [peers, setPeers] = useState<PeerInfo[]>([]);
  const [debugLogs, setDebugLogs] = useState<LogEntry[]>([]);
  const [isDiscovering, setIsDiscovering] = useState(false);

  const [localIp, setLocalIp] = useState<string | null>(null);
  const [hasPermissions, setHasPermissions] = useState<boolean | null>(null);
  const [osName, setOsName] = useState<string>("");

  useEffect(() => {
    async function init() {
      try {
        const id = await invoke<string>("get_device_info");
        setDeviceId(id);
        
        const perms = await invoke<boolean>("check_permissions");
        setHasPermissions(perms);
        
        if (navigator.userAgent.includes("Mac")) {
          setOsName("macOS");
        } else if (navigator.userAgent.includes("Win")) {
          setOsName("Windows");
        } else {
          setOsName("Linux");
        }
      } catch (e) {
        console.error(e);
      }
    }
    init();

    // Listen for discovery events from backend
    const unlistenDiscovered = listen<PeerInfo>("peer-discovered", (event) => {
      setPeers((prev) => {
        if (prev.find((p) => p.device_id === event.payload.device_id)) return prev;
        return [...prev, event.payload];
      });
    });

    // Listen for debug logs from backend
    const unlistenLogs = listen<LogEntry>("debug-log", (event) => {
      setDebugLogs((prev) => [event.payload, ...prev].slice(0, 50));
    });

    const unlistenRemoved = listen<string>("peer-removed", (event) => {
      setPeers((prev) => prev.filter((p) => p.device_id !== event.payload));
    });

    return () => {
      unlistenDiscovered.then((f) => f());
      unlistenLogs.then((f) => f());
      unlistenRemoved.then((f) => f());
    };
  }, []);

  async function handleEmergencyKill() {
    await invoke("emergency_kill");
    alert("Emergency Kill-switch activated! All input injection halted.");
  }

  async function handleDiscovery() {
    setIsDiscovering(true);
    setDebugLogs(prev => [{ message: "Discovery started...", level: "info", timestamp: Date.now() }, ...prev]);
    try {
      // Trigger registration and get current list
      const foundPeers = await invoke<PeerInfo[]>("start_discovery");
      setPeers(foundPeers);
    } catch (e) {
      console.error(e);
      setDebugLogs(prev => [{ message: `Discovery error: ${e}`, level: "error", timestamp: Date.now() }, ...prev]);
    }
  }

  const [manualIp, setManualIp] = useState("");
  const [showManual, setShowManual] = useState(false);

  const handleConnect = async (peer: PeerInfo) => {
    if (hasPermissions === false) {
      const msg = osName === "macOS" 
        ? "Please grant SYNQ 'Accessibility' permissions in System Settings > Privacy & Security to allow input sharing." 
        : "System permissions are missing. Input sharing may not work.";
      alert(`⚠️ Permissions Required\n\n${msg}`);
      return;
    }

    try {
      console.log("Initiating connection to:", peer);
      setDebugLogs(prev => [{ message: `Click: Connect to ${peer.name}`, level: "info", timestamp: Date.now() }, ...prev]);
      
      // Transition UI to connecting state
      setPeers((prev) => 
        prev.map(p => p.device_id === peer.device_id ? { ...p, status: 'connecting' } : p)
      );
      
      await invoke("connect_to_peer", { peer });
      
      // Update status on success
      setPeers((prev) => 
        prev.map(p => p.device_id === peer.device_id ? { ...p, status: 'connected' } : p)
      );
    } catch (e) {
      console.error("Connection failed:", e);
      setDebugLogs(prev => [{ message: `Connection failed: ${e}`, level: "error", timestamp: Date.now() }, ...prev]);
      alert(`Failed to connect to ${peer.name}: ${e}`);
      
      // Reset status on failure
      setPeers((prev) => 
        prev.map(p => p.device_id === peer.device_id ? { ...p, status: undefined } : p)
      );
    }
  };

  async function handleManualConnect() {
    if (!manualIp) return;
    setDebugLogs(prev => [{ message: `Manual link attempted: ${manualIp}`, level: "info", timestamp: Date.now() }, ...prev]);
    try {
      // Use 52821 for signaling
      const fullAddress = manualIp.includes(":") ? manualIp : `${manualIp}:52821`;
      
      const manualPeer: PeerInfo = {
        device_id: "00000000-0000-0000-0000-000000000000", 
        name: `Manual Link`,
        platform: "Windows",
        screen: { width: 1920, height: 1080, x: 0, y: 0 },
        address: fullAddress
      };
      
      setPeers(prev => {
        if (prev.find(p => p.address === fullAddress)) return prev;
        return [...prev, manualPeer];
      });
      
      setShowManual(false);
      setManualIp("");
      
      await handleConnect(manualPeer);
    } catch (e) {
      console.error(e);
      setDebugLogs(prev => [{ message: `Manual link failed: ${e}`, level: "error", timestamp: Date.now() }, ...prev]);
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
      {hasPermissions === false && (
        <div style={{ background: 'rgba(255, 60, 60, 0.15)', borderBottom: '1px solid rgba(255, 60, 60, 0.3)', padding: '12px 20px', display: 'flex', flexDirection: 'column', gap: '8px' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px', color: '#ff6b6b', fontWeight: 'bold' }}>
            <span>⚠️</span> {osName === "macOS" ? "Accessibility Permissions Required" : "System Permissions Required"}
          </div>
          <p style={{ fontSize: '0.85rem', color: '#ffbaba', margin: 0, lineHeight: 1.4 }}>
            {osName === "macOS" ? (
              <>SYNQ needs permission to capture and inject your mouse & keyboard. Go to <strong>System Settings &gt; Privacy &amp; Security &gt; Accessibility</strong> and enable SYNQ.</>
            ) : (
              <>SYNQ needs elevated permissions to capture and inject inputs on this system.</>
            )}
          </p>
          <button 
            onClick={async () => {
              const perms = await invoke<boolean>("check_permissions");
              setHasPermissions(perms);
              if (perms) alert("Permissions verified!");
            }}
            style={{ alignSelf: 'flex-start', background: 'transparent', border: '1px solid #ff6b6b', color: '#ff6b6b', padding: '4px 12px', borderRadius: '4px', fontSize: '0.75rem', cursor: 'pointer', marginTop: '4px' }}
          >
            Check Again
          </button>
        </div>
      )}

      <header style={{ marginTop: hasPermissions === false ? '0' : '30px' }}>
        <h1 style={{ letterSpacing: '2px' }}>SYNQ</h1>
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

      {/* Debug Console Section */}
      <section className="card debug-console" style={{ maxHeight: '150px', overflowY: 'auto', background: '#000', padding: '10px', fontSize: '0.75rem', fontFamily: 'monospace', marginBottom: '20px', border: '1px solid #333' }}>
        <div style={{ color: '#0f0', marginBottom: '5px', borderBottom: '1px solid #222', paddingBottom: '2px', display: 'flex', justifyContent: 'space-between' }}>
          <span>SYSTEM LOG</span>
          <span style={{ cursor: 'pointer', color: '#666' }} onClick={() => setDebugLogs([])}>Clear</span>
        </div>
        {debugLogs.length === 0 && <div style={{ color: '#444' }}>Waiting for activity...</div>}
        {debugLogs.map((log, i) => (
          <div key={i} style={{ color: log.level === 'error' ? '#f55' : log.level === 'warn' ? '#ff5' : '#aaa', marginBottom: '2px' }}>
            <span style={{ color: '#555' }}>[{new Date(log.timestamp).toLocaleTimeString([], { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' })}]</span> {log.message}
          </div>
        ))}
      </section>

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
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: '20px', padding: '0 8px' }}>
                <h3 style={{ color: 'var(--text-secondary)', fontSize: '0.9rem' }}>
                  DISCOVERED PEERS
                </h3>
                <button 
                  style={{ background: 'transparent', border: 'none', color: 'var(--accent-color)', fontSize: '0.7rem', cursor: 'pointer' }}
                  onClick={() => setPeers([])}
                >
                  Clear List
                </button>
              </div>
              {peers.map((peer) => (
                <div key={peer.device_id} className="peer-card">
                  <div className="peer-info">
                    <h4>{peer.name}</h4>
                    <span>{peer.platform} • {peer.address || "Local Network"}</span>
                  </div>
                  <button 
                    className={`connect-badge ${peer.status === 'connecting' ? 'connecting' : ''} ${peer.status === 'connected' ? 'connected' : ''}`} 
                    onClick={() => handleConnect(peer)}
                    disabled={peer.status === 'connecting' || peer.status === 'connected'}
                  >
                    {peer.status === 'connecting' ? '...' : peer.status === 'connected' ? 'Connected' : 'Connect'}
                  </button>
                </div>
              ))}
            </>
          )}

          <button 
            style={{ marginTop: '20px', background: 'transparent', border: '1px solid var(--border-color)', color: 'var(--text-secondary)' }}
            onClick={() => {
              setIsDiscovering(false);
              setPeers([]);
              setShowManual(false);
            }}
          >
            ← Back to Home
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
        <span style={{ fontSize: '0.7rem', color: 'var(--text-secondary)', textAlign: 'center', marginTop: '4px', opacity: 0.7 }}>
          Global Hotkey: <strong>Alt + Shift + Esc</strong>
        </span>
      </footer>
    </main>
  );
}

export default App;
