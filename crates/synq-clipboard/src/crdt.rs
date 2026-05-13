//! CRDT store for clipboard entries using Automerge.
//!
//! Provides conflict-free merge of clipboard state across peers.

use synq_core::{ClipboardObject, SynqResult};

/// CRDT-backed clipboard store.
pub struct CrdtStore {
    /// Local clipboard history (most recent first).
    entries: Vec<ClipboardObject>,
    /// Maximum number of entries to retain.
    max_entries: usize,
}

impl CrdtStore {
    /// Create a new CRDT store with default capacity.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            max_entries: 50,
        }
    }

    /// Add a new clipboard entry to the store.
    pub fn insert(&mut self, obj: ClipboardObject) -> SynqResult<()> {
        // Dedup by hash
        if self.entries.iter().any(|e| e.content_matches(&obj)) {
            tracing::debug!("Duplicate clipboard entry — skipping");
            return Ok(());
        }

        self.entries.insert(0, obj);

        // Trim to max
        if self.entries.len() > self.max_entries {
            self.entries.truncate(self.max_entries);
        }

        Ok(())
    }

    /// Get the most recent clipboard entry.
    pub fn latest(&self) -> Option<&ClipboardObject> {
        self.entries.first()
    }

    /// Get all entries (most recent first).
    pub fn entries(&self) -> &[ClipboardObject] {
        &self.entries
    }

    /// Generate a sync message for sending to a peer.
    ///
    /// TODO: Replace with actual Automerge sync protocol.
    pub fn generate_sync_message(&self) -> SynqResult<Vec<u8>> {
        let data = serde_json::to_vec(&self.entries)?;
        Ok(data)
    }

    /// Apply a sync message received from a peer.
    ///
    /// TODO: Replace with actual Automerge sync protocol.
    pub fn apply_sync_message(&mut self, _msg: &[u8]) -> SynqResult<()> {
        tracing::info!("CRDT sync message applied — placeholder");
        Ok(())
    }
}

impl Default for CrdtStore {
    fn default() -> Self {
        Self::new()
    }
}
