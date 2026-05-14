//! synq-app — The main Tauri application shell.

use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{State, Manager, Emitter};
use tracing::{info, error};

use synq_core::{SynqResult, DeviceId};
use synq_net::{SynqNetLayer, NetLayer};
use synq_clipboard::{SynqClipboardEngine, ClipboardEngine};
use synq_input::{InputEngine, create_input_engine, killswitch};
use synq_focus::{create_focus_arbiter, FocusArbiter, FocusConfig};

pub struct AppState {
    pub daemon: Arc<Mutex<SynqDaemon>>,
}

/// The main orchestrator connecting all Synq components
pub struct SynqDaemon {
    pub device_id: DeviceId,
    pub net: Arc<Mutex<SynqNetLayer>>,
    pub input: Arc<dyn InputEngine>,
    pub clipboard: Arc<Mutex<SynqClipboardEngine>>,
    pub focus: Box<dyn FocusArbiter>,
    pub app_handle: tauri::AppHandle,
}

impl SynqDaemon {
    pub fn emit_log(&self, message: &str, level: &str) {
        let _ = self.app_handle.emit("debug-log", serde_json::json!({
            "message": message,
            "level": level,
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        }));
        match level {
            "error" => tracing::error!("{}", message),
            "warn" => tracing::warn!("{}", message),
            _ => tracing::info!("{}", message),
        }
    }

    pub async fn new(app_handle: tauri::AppHandle) -> SynqResult<Self> {
        info!("Initializing SynqDaemon...");
        
        let device_id = DeviceId::new();
        
        // TODO: Generate or load real noise private key
        let dummy_private_key = vec![0; 32];
        let net = SynqNetLayer::new(device_id, dummy_private_key)?;
        let net_shared = Arc::new(Mutex::new(net));
        
        let input: Arc<dyn InputEngine> = create_input_engine()?.into();
        let focus = create_focus_arbiter(FocusConfig::default());
        
        // Check permissions early
        if !input.check_permissions()? {
            error!("Accessibility permissions not granted. Global sync and kill-switch will not work.");
        }

        let net_for_cb = net_shared.clone();
        
        let clipboard_cb = Arc::new(move |obj: synq_core::ClipboardObject| {
            let net = net_for_cb.clone();
            tokio::spawn(async move {
                if let Ok(data) = serde_json::to_vec(&obj) {
                    let lock = net.lock().await;
                    let _ = lock.send(&data, true).await;
                }
            });
        });

        let mut clipboard_engine = SynqClipboardEngine::new(device_id, Some(clipboard_cb));
        clipboard_engine.start_observing().await?;
        let clipboard = Arc::new(Mutex::new(clipboard_engine));

        // Start background receiver loop
        let net_rx = net_shared.clone();
        let clipboard_rx = clipboard.clone();
        let input_rx = input.clone();
        tokio::spawn(async move {
            loop {
                // Check connection state first
                let connected = {
                    let lock = net_rx.lock().await;
                    lock.is_connected()
                };

                if connected {
                    let data = {
                        let lock = net_rx.lock().await;
                        lock.recv().await
                    };

                    if let Ok(data) = data {
                        // Try parsing as ClipboardObject
                        if let Ok(obj) = serde_json::from_slice::<synq_core::ClipboardObject>(&data) {
                            let mut cb = clipboard_rx.lock().await;
                            let _ = cb.on_remote_update(obj).await;
                            let _ = cb.apply_staged().await;
                        }
                        // Try parsing as InputEvent
                        else if let Ok(event) = serde_json::from_slice::<synq_core::InputEvent>(&data) {
                            let _ = input_rx.inject_event(&event);
                        }
                    }
                } else {
                    // Not connected, sleep to avoid spinning
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
            }
        });

        info!("SynqDaemon initialized with DeviceID: {}", device_id);
        
        let daemon = Self {
            device_id,
            net: net_shared,
            input,
            clipboard,
            focus,
            app_handle,
        };

        daemon.emit_log("Continuity Daemon fully initialized.", "info");
        Ok(daemon)
    }

    /// Start forwarding local input to the network
    pub async fn start_input_sync(&self) -> SynqResult<()> {
        let net = self.net.clone();
        let input = self.input.clone();
        
        info!("Input Sync: Starting local capture...");
        input.start_capture(Box::new(move |event| {
            let net = net.clone();
            tokio::spawn(async move {
                if let Ok(data) = serde_json::to_vec(&event) {
                    let lock = net.lock().await;
                    // Send unreliable for input
                    let _ = lock.send(&data, false).await;
                }
            });
        }))?;
        
        Ok(())
    }
}

// -------------------------------------------------------------------------
// Tauri Commands (Invokable from frontend)
// -------------------------------------------------------------------------

#[tauri::command]
async fn get_local_ip() -> Result<String, String> {
    info!("Fetching local IP...");
    local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .map_err(|e| {
            error!("IP resolution failed: {}", e);
            "Could not resolve local IP".to_string()
        })
}

#[tauri::command]
async fn get_device_info(state: State<'_, AppState>) -> Result<String, String> {
    let daemon = state.daemon.lock().await;
    Ok(daemon.device_id.to_string())
}

#[tauri::command]
async fn start_discovery(state: State<'_, AppState>) -> Result<Vec<synq_core::PeerInfo>, String> {
    use synq_core::{PeerInfo, Platform, ScreenGeometry};
    
    info!("Discovery command triggered...");
    
    let (device_id, name) = {
        let daemon = state.daemon.lock().await;
        (daemon.device_id, whoami::devicename())
    };
    
    let local_peer = PeerInfo {
        device_id,
        name: if name.is_empty() { "Unknown Device".into() } else { name },
        platform: if cfg!(target_os = "macos") { Platform::MacOS } else { Platform::Windows },
        screen: ScreenGeometry { width: 1920, height: 1080, x: 0, y: 0 },
        address: None,
    };

    let daemon = state.daemon.lock().await;
    let net = daemon.net.lock().await;
    if let Err(e) = net.register_local(&local_peer) {
        error!("Failed to register local device: {}", e);
    }
    
    net.discover_peers().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn connect_to_peer(state: State<'_, AppState>, mut peer: synq_core::PeerInfo) -> Result<(), String> {
    let daemon = state.daemon.lock().await;
    daemon.emit_log(&format!("Connection request to: {}", peer.name), "info");
    daemon.emit_log(&format!("Target IP: {:?}", peer.address), "info");

    // If address is provided but no port, default to our signaling port
    if let Some(addr) = &peer.address {
        if !addr.contains(":") {
            peer.address = Some(format!("{}:52821", addr));
        }
    }

    let net = daemon.net.clone();
    drop(daemon);

    let mut net_lock = net.lock().await;
    net_lock.connect(&peer).await.map_err(|e| {
        e.to_string()
    })?;
    
    let daemon = state.daemon.lock().await;
    daemon.emit_log("Handshake successful!", "info");
    let _ = daemon.start_input_sync().await;
    
    Ok(())
}

#[tauri::command]
async fn emergency_kill(_state: State<'_, AppState>) -> Result<(), String> {
    info!("Emergency kill requested from UI");
    killswitch::activate();
    Ok(())
}

#[tauri::command]
async fn check_permissions(state: State<'_, AppState>) -> Result<bool, String> {
    let daemon = state.daemon.lock().await;
    daemon.input.check_permissions().map_err(|e| e.to_string())
}

// -------------------------------------------------------------------------
// App Entry Point
// -------------------------------------------------------------------------

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent, MouseButton};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();
    
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_shortcuts(["alt+shift+escape"]).map(|b| b.with_handler(|_app, shortcut, event| {
            if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                if shortcut.matches(tauri_plugin_global_shortcut::Modifiers::ALT | tauri_plugin_global_shortcut::Modifiers::SHIFT, tauri_plugin_global_shortcut::Code::Escape) {
                    tracing::info!("Global shortcut triggered: Alt+Shift+Escape");
                    synq_input::killswitch::activate();
                }
            }
        })).unwrap_or_else(|e| {
            tracing::error!("Failed to initialize global shortcut plugin: {}", e);
            tauri_plugin_global_shortcut::Builder::new()
        }).build())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            let app_handle = app.handle().clone();
            let daemon_future = SynqDaemon::new(app_handle);
            let daemon = tauri::async_runtime::block_on(daemon_future).expect("Fatal initialization error");
            
            let app_state = AppState {
                daemon: Arc::new(Mutex::new(daemon)),
            };
            app.manage(app_state);

            let show = MenuItem::with_id(app, "show", "Open Dashboard", true, None::<&str>)?;
            let hide = MenuItem::with_id(app, "hide", "Hide to Tray", true, None::<&str>)?;
            let kill = MenuItem::with_id(app, "kill", "🛑 Emergency Kill", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Synq", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(app)?;

            let menu = Menu::with_items(app, &[&show, &hide, &separator, &kill, &separator, &quit])?;

            let tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => { app.exit(0); }
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "hide" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.hide();
                        }
                    }
                    "kill" => { killswitch::activate(); }
                    _ => {}
                })
                .build(app)?;

            app.manage(tray);

            // Disable the global hotkey listener for now to prevent macOS force-quits during typing
            // synq_input::killswitch::start_hotkey_listener();

            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = app_handle.state::<AppState>();
                let daemon = state.daemon.lock().await;
                let net = daemon.net.lock().await;
                net.start_discovery_monitor(app_handle.clone(), daemon.device_id, whoami::devicename());
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_device_info,
            get_local_ip,
            start_discovery,
            connect_to_peer,
            emergency_kill,
            check_permissions
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
