//! synq-input — Input engine trait and platform-specific HID implementations.
//!
//! This crate defines the `InputEngine` trait that all platform backends must
//! implement, plus the kill-switch safety mechanism.
//!
//! Platform implementations are conditionally compiled:
//! - macOS: `macos.rs` (CGEvent-based, Tier 1)
//! - Windows: `windows_impl.rs` (SendInput-based, Tier 1)

pub mod killswitch;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows_impl;

use synq_core::{InputEvent, SynqResult};

/// The core input injection trait.
///
/// Implementations must be thread-safe (`Send + Sync`) as input events
/// arrive from the networking layer on a separate task.
pub trait InputEngine: Send + Sync {
    /// Inject a single input event into the OS.
    ///
    /// Returns `Err(SynqError::KillSwitch)` if the kill-switch is active.
    fn inject_event(&self, event: &InputEvent) -> SynqResult<()>;

    /// Begin capturing local input (for forwarding to a remote peer).
    ///
    /// While grabbed, local input is intercepted and forwarded via the net layer
    /// instead of being processed locally.
    fn grab_input(&self) -> SynqResult<()>;

    /// Release the local input grab, restoring normal input processing.
    fn release_input(&self) -> SynqResult<()>;

    /// Emergency kill-switch: immediately release all grabs and stop injection.
    ///
    /// This is the safety valve — it must never fail.
    fn emergency_kill(&self);

    /// Check if accessibility / input permissions are granted.
    fn check_permissions(&self) -> SynqResult<bool>;
}

/// Create the platform-appropriate input engine.
pub fn create_input_engine() -> SynqResult<Box<dyn InputEngine>> {
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(macos::MacOSInputEngine::new()?))
    }
    #[cfg(target_os = "windows")]
    {
        Ok(Box::new(windows_impl::WindowsInputEngine::new()?))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err(synq_core::SynqError::Other(
            "Unsupported platform".into(),
        ))
    }
}
