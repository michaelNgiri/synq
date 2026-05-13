import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

function App() {
  const [deviceId, setDeviceId] = useState<string>("Loading...");

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
    await invoke("start_discovery");
    alert("Discovery started. Check backend logs.");
  }

  return (
    <main className="container">
      <h1>Synq Engine</h1>
      <p>Your Device ID: <strong>{deviceId}</strong></p>

      <div className="card">
        <p>The continuity daemon is running in the background.</p>
        <button onClick={handleDiscovery}>Start Discovery</button>
      </div>

      <div className="card kill-switch">
        <button onClick={handleEmergencyKill} style={{ backgroundColor: '#ff4d4d', color: 'white' }}>
          🛑 Emergency Kill-switch
        </button>
      </div>
    </main>
  );
}

export default App;
