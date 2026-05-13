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
async fn get_local_ip() -> Result<String, String> {
    local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .map_err(|e| e.to_string())
}

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

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent, MouseButton};

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
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            let show = MenuItem::with_id(app, "show", "Open Dashboard", true, None::<&str>)?;
            let hide = MenuItem::with_id(app, "hide", "Hide to Tray", true, None::<&str>)?;
            let about = MenuItem::with_id(app, "about", "About Synq...", true, None::<&str>)?;
            let kill = MenuItem::with_id(app, "kill", "🛑 Emergency Kill", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Synq", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(app)?;

            let menu = Menu::with_items(app, &[&show, &hide, &separator, &about, &kill, &separator, &quit])?;

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
                    "quit" => {
                        app.exit(0);
                    }
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
                    "kill" => {
                        killswitch::activate();
                    }
                    _ => {}
                })
                .build(app)?;

            // Keep the tray alive by managing it
            app.manage(tray);

            // Start the background discovery monitor now that the runtime is ready
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = app_handle.state::<AppState>();
                let daemon = state.daemon.lock().await;
                daemon.net.start_discovery_monitor(
                    app_handle.clone(), 
                    daemon.device_id, 
                    whoami::devicename()
                );
            });

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
            get_local_ip,
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
