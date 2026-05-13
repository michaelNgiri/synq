use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{State, Manager};
use tracing::{info, error};
use tracing_subscriber;

// Core Synq Crates
use synq_core::{DeviceId, SynqResult};
use synq_net::SynqNetLayer;
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
async fn start_discovery(state: State<'_, AppState>) -> Result<(), String> {
    let _daemon = state.daemon.lock().await;
    // In a real implementation, we would call daemon.net.discover_peers()
    // and push the results to the frontend via events.
    info!("Discovery started via frontend command");
    Ok(())
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

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
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
                
            // On macOS, hide the dock icon to make it a true menu bar app
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                // Prevent the app from exiting when the window is closed
                // Instead, just hide the window so it stays in the tray
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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
