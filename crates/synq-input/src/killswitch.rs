//! Emergency kill-switch for the input engine.
//!
//! A global atomic flag that immediately halts all input injection
//! when activated. Checked before every `inject_event()` call.

use std::sync::atomic::{AtomicBool, Ordering};

/// Global kill-switch state. When `true`, all injection is refused.
static KILL_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Activate the kill-switch. All subsequent `inject_event()` calls
/// will return `Err(SynqError::KillSwitch)`.
pub fn activate() {
    if !is_active() {
        tracing::warn!("🛑 Kill-switch ACTIVATED — all input injection halted");
        KILL_ACTIVE.store(true, Ordering::SeqCst);
    }
}

/// Deactivate the kill-switch, allowing injection to resume.
pub fn deactivate() {
    if is_active() {
        tracing::info!("✅ Kill-switch deactivated — input injection resumed");
        KILL_ACTIVE.store(false, Ordering::SeqCst);
    }
}

/// Check if the kill-switch is currently active.
pub fn is_active() -> bool {
    KILL_ACTIVE.load(Ordering::SeqCst)
}

/// Guard that should be checked at the start of every `inject_event()`.
/// Returns `Err(SynqError::KillSwitch)` if the switch is active.
pub fn check() -> synq_core::SynqResult<()> {
    if is_active() {
        Err(synq_core::SynqError::KillSwitch)
    } else {
        Ok(())
    }
}

// We no longer start the listener here using rdev, as multiple rdev listeners
// conflict and cause macOS crashes. The global hotkey is now managed by
// tauri-plugin-global-shortcut in the main application shell.
// Default Hotkey: Alt + Shift + Escape
