//! Windows input engine using Win32 SendInput API (Tier 1).
//!
//! Stub implementation — will be fleshed out when targeting Windows.
//! Compiles only on `target_os = "windows"`.

use synq_core::{InputEvent, SynqError, SynqResult};

use crate::killswitch;
use crate::InputEngine;

/// Windows input engine backed by Win32 `SendInput`.
pub struct WindowsInputEngine;

impl WindowsInputEngine {
    pub fn new() -> SynqResult<Self> {
        Ok(Self)
    }
}

impl InputEngine for WindowsInputEngine {
    fn inject_event(&self, event: &InputEvent) -> SynqResult<()> {
        killswitch::check()?;
        // TODO: Implement SendInput injection for each InputEventKind
        tracing::warn!("Windows input injection not yet implemented");
        Err(SynqError::InputInjection(
            "Windows implementation pending".into(),
        ))
    }

    fn grab_input(&self) -> SynqResult<()> {
        tracing::info!("Input grab requested (Windows) — placeholder");
        Ok(())
    }

    fn release_input(&self) -> SynqResult<()> {
        tracing::info!("Input release requested (Windows) — placeholder");
        Ok(())
    }

    fn emergency_kill(&self) {
        killswitch::activate();
        let _ = self.release_input();
    }

    fn check_permissions(&self) -> SynqResult<bool> {
        // Windows SendInput doesn't require special permissions
        // (unless UAC-elevated apps are targeted).
        Ok(true)
    }
}
