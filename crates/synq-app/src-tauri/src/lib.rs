use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{State, Manager};
use tracing::{info, error};
use tracing_subscriber;

// Core Synq Crates
use synq_core::{DeviceId, SynqResult};
use synq_net::{SynqNetLayer, NetLayer};
use synq_input::{create_input_engine, InputEngine, killswitch};
use synq_clipboard::SynqClipboardEngine;
use synq_focus::{create_focus_arbiter, FocusArbiter, FocusConfig};

/// Global state structure managed by Tauri
pub struct AppState {
    pub daemon: Arc<Mutex<SynqDaemon>>,
}

/// The main orchestrator connecting all Synq components
pub struct SynqDaemon {
    pub device_id: DeviceId,
    pub net: SynqNetLayer,
    pub input: Box<dyn InputEngine>,
    pub clipboard: SynqClipboardEngine,
    pub focus: Box<dyn FocusArbiter>,
}

impl SynqDaemon {
    pub fn new() -> SynqResult<Self> {
        info!("Initializing SynqDaemon...");
        
        let device_id = DeviceId::new();
        
        // TODO: Generate or load real noise private key
        let dummy_private_key = vec![0; 32];
        let net = SynqNetLayer::new(dummy_private_key)?;
        
        let input = create_input_engine()?;
        let clipboard = SynqClipboardEngine::new(device_id);
        let focus = create_focus_arbiter(FocusConfig::default());
        
        // Start the global emergency kill-switch listener
        killswitch::start_hotkey_listener();

        info!("SynqDaemon initialized with DeviceID: {}", device_id);
        
        Ok(Self {
            device_id,
            net,
            input,
            clipboard,
            focus,
        })
    }
}

// -------------------------------------------------------------------------
// Tauri Commands (Invokable from frontend)
// -------------------------------------------------------------------------

#[tauri::command]
async fn get_device_info(state: State<'_, AppState>) -> Result<String, String> {
    let daemon = state.daemon.lock().await;
    Ok(daemon.device_id.to_string())
}

#[tauri::command]
async fn start_discovery(state: State<'_, AppState>) -> Result<Vec<synq_core::PeerInfo>, String> {
    use synq_core::{PeerInfo, Platform, ScreenGeometry};
    
    let daemon = state.daemon.lock().await;
    
    // Construct local peer info for registration
    let local_peer = PeerInfo {
        device_id: daemon.device_id,
        name: whoami::devicename(), // Use machine name
        platform: if cfg!(target_os = "macos") { Platform::MacOS } else { Platform::Windows },
        screen: ScreenGeometry {
            width: 1920, // TODO: Get actual screen size
            height: 1080,
            x: 0,
            y: 0,
        },
        address: None, // Will be filled by mDNS
    };

    // 1. Register ourselves so others can find us
    daemon.net.register_local(&local_peer).map_err(|e| e.to_string())?;
    
    // 2. Look for others
    info!("Discovery started for device: {}", local_peer.name);
    let peers = daemon.net.discover_peers().await.map_err(|e| e.to_string())?;
    
    Ok(peers)
}

#[tauri::command]
async fn emergency_kill(_state: State<'_, AppState>) -> Result<(), String> {
    info!("Emergency kill requested from UI");
    killswitch::activate();
    Ok(())
}

// -------------------------------------------------------------------------
// App Entry Point
// -------------------------------------------------------------------------

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();
    
    let daemon = match SynqDaemon::new() {
        Ok(d) => d,
        Err(e) => {
            error!("Failed to initialize SynqDaemon: {:?}", e);
            panic!("Fatal initialization error");
        }
    };
    
    let app_state = AppState {
        daemon: Arc::new(Mutex::new(daemon)),
    };

    tauri::Builder::default()
        .manage(app_state)
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let show_i = MenuItem::with_id(app, "show", "Settings", true, None::<&str>)?;
            let kill_i = MenuItem::with_id(app, "kill", "Emergency Kill", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &kill_i, &quit_i])?;

            let tray_icon = app.default_window_icon().unwrap().clone();

            let tray = TrayIconBuilder::new()
                .icon(tray_icon)
                .menu(&menu)
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click { .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let is_visible = window.is_visible().unwrap_or(false);
                            if is_visible {
                                window.hide().unwrap();
                            } else {
                                window.show().unwrap();
                                window.set_focus().unwrap();
                            }
                        }
                    }
                })
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => {
                        std::process::exit(0);
                    }
                    "show" => {
                        let window = app.get_webview_window("main").unwrap();
                        window.show().unwrap();
                        window.set_focus().unwrap();
                    }
                    "kill" => {
                        killswitch::activate();
                    }
                    _ => {}
                })
                .build(app)?;

            // Keep the tray alive by managing it
            app.manage(tray);

            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                // Instead of closing, hide to tray
                window.hide().unwrap();
                api.prevent_close();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            get_device_info,
            start_discovery,
            emergency_kill
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| match event {
            tauri::RunEvent::ExitRequested { api, .. } => {
                api.prevent_exit();
            }
            _ => {}
        });
}
