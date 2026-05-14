//! synq-clipboard — CRDT-based clipboard synchronization.

pub mod crdt;
pub mod observer;
pub mod schema;

use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::Mutex;
use tracing::{info, warn, error};

use synq_core::{ClipboardObject, SynqResult, DeviceId, SynqError};
pub use observer::ClipboardObserver;
pub use crdt::CrdtStore;

/// The clipboard synchronization engine trait.
#[async_trait]
pub trait ClipboardEngine: Send + Sync {
    async fn start_observing(&mut self) -> SynqResult<()>;
    async fn stop_observing(&mut self) -> SynqResult<()>;
    async fn on_remote_update(&mut self, obj: ClipboardObject) -> SynqResult<()>;
    async fn apply_staged(&mut self) -> SynqResult<()>;
    fn get_current(&self) -> Option<&ClipboardObject>;
    fn get_staged(&self) -> Option<&ClipboardObject>;
}

/// Standard implementation of the Synq clipboard engine.
pub struct SynqClipboardEngine {
    observer: Arc<Mutex<ClipboardObserver>>,
    store: Arc<Mutex<CrdtStore>>,
    staged: Option<ClipboardObject>,
    current: Option<ClipboardObject>,
    is_running: bool,
    device_id: DeviceId,
    broadcast_cb: Option<Arc<dyn Fn(synq_core::ClipboardObject) + Send + Sync>>,
}

impl SynqClipboardEngine {
    pub fn new(
        device_id: DeviceId, 
        broadcast_cb: Option<Arc<dyn Fn(synq_core::ClipboardObject) + Send + Sync>>
    ) -> Self {
        Self {
            observer: Arc::new(Mutex::new(ClipboardObserver::new(device_id))),
            store: Arc::new(Mutex::new(CrdtStore::new())),
            staged: None,
            current: None,
            is_running: false,
            device_id,
            broadcast_cb,
        }
    }

    /// Internal poll loop
    async fn poll_loop(
        observer: Arc<Mutex<ClipboardObserver>>,
        store: Arc<Mutex<CrdtStore>>,
        broadcast_cb: Option<Arc<dyn Fn(ClipboardObject) + Send + Sync>>,
    ) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(250));
        loop {
            interval.tick().await;
            
            let mut obs = observer.lock().await;
            match obs.poll() {
                Ok(Some(obj)) => {
                    info!("Local clipboard change detected: {}", obj.mime_type);
                    let mut s = store.lock().await;
                    let _ = s.insert(obj.clone());
                    if let Some(cb) = &broadcast_cb {
                        cb(obj);
                    }
                }
                Ok(None) => {}
                Err(e) => error!("Clipboard poll error: {:?}", e),
            }
        }
    }
}

#[async_trait]
impl ClipboardEngine for SynqClipboardEngine {
    async fn start_observing(&mut self) -> SynqResult<()> {
        if self.is_running { return Ok(()); }
        
        let observer = self.observer.clone();
        let store = self.store.clone();
        let _device_id = self.device_id;
        
        let broadcast_cb = self.broadcast_cb.clone();
        
        tokio::spawn(async move {
            Self::poll_loop(observer, store, broadcast_cb).await;
        });
        
        self.is_running = true;
        info!("Clipboard observation started");
        Ok(())
    }

    async fn stop_observing(&mut self) -> SynqResult<()> {
        self.is_running = false;
        // In a real impl, we'd use a cancellation token
        Ok(())
    }

    async fn on_remote_update(&mut self, obj: ClipboardObject) -> SynqResult<()> {
        info!("Remote clipboard update received: {} from {}", obj.mime_type, obj.origin_device);
        
        // Stage the update instead of applying immediately (Paste-Gating)
        self.staged = Some(obj.clone());
        
        let mut store = self.store.lock().await;
        store.insert(obj)?;
        
        Ok(())
    }

    async fn apply_staged(&mut self) -> SynqResult<()> {
        let obj = self.staged.take().ok_or(SynqError::Clipboard("No staged clipboard entry".into()))?;
        
        info!("Applying staged clipboard entry to system");
        
        let mut cb = arboard::Clipboard::new().map_err(|e| SynqError::Clipboard(e.to_string()))?;
        
        match &obj.content {
            synq_core::ClipboardContent::Text(text) => {
                cb.set_text(text.clone()).map_err(|e| SynqError::Clipboard(e.to_string()))?;
            }
            synq_core::ClipboardContent::Image(_data) => {
                // arboard requires ImageData struct. This is simplified for Phase 1.
                warn!("Image clipboard application not fully implemented in Phase 1 MVP");
            }
            _ => {
                warn!("Clipboard type not supported for application yet");
            }
        }

        // Update the observer's last hash to avoid feedback loop
        let mut obs = self.observer.lock().await;
        obs.set_last_hash(obj.payload_hash);
        
        self.current = Some(obj);
        Ok(())
    }

    fn get_current(&self) -> Option<&ClipboardObject> {
        self.current.as_ref()
    }

    fn get_staged(&self) -> Option<&ClipboardObject> {
        self.staged.as_ref()
    }
}
