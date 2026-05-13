//! System clipboard observer — polls for changes and detects new content.
//!
//! Uses `arboard` for cross-platform clipboard access and BLAKE3
//! hashing for efficient change detection.

use synq_core::{ClipboardObject, DeviceId, SynqResult};

/// Clipboard observer that polls the system clipboard for changes.
pub struct ClipboardObserver {
    /// BLAKE3 hash of the last known clipboard content.
    last_hash: Option<[u8; 32]>,
    /// This device's ID (for tagging origin).
    device_id: DeviceId,
    /// Poll interval in milliseconds.
    pub poll_interval_ms: u64,
}

impl ClipboardObserver {
    /// Create a new observer for the given device.
    pub fn new(device_id: DeviceId) -> Self {
        Self {
            last_hash: None,
            device_id,
            poll_interval_ms: 250,
        }
    }

    /// Poll the system clipboard and return a `ClipboardObject` if the
    /// content has changed since the last poll.
    pub fn poll(&mut self) -> SynqResult<Option<ClipboardObject>> {
        // Try reading text from the clipboard
        let mut clipboard = arboard::Clipboard::new().map_err(|e| {
            synq_core::SynqError::Clipboard(format!("Failed to access clipboard: {e}"))
        })?;

        if let Ok(text) = clipboard.get_text() {
            let hash = blake3::hash(text.as_bytes());
            let hash_bytes = *hash.as_bytes();

            // Check if content changed
            if self.last_hash.as_ref() != Some(&hash_bytes) {
                self.last_hash = Some(hash_bytes);
                let obj = ClipboardObject::new_text(text, self.device_id);
                tracing::debug!("Clipboard change detected: {}", obj.mime_type);
                return Ok(Some(obj));
            }
        }

        Ok(None)
    }

    /// Set the hash to a known value (to prevent feedback loops when
    /// we write to the clipboard ourselves).
    pub fn set_last_hash(&mut self, hash: [u8; 32]) {
        self.last_hash = Some(hash);
    }
}
